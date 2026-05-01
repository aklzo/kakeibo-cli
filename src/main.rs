mod cli;
mod db;
mod model;
mod repository;

use std::collections::HashMap;

use anyhow::Context;
use clap::Parser;
use rusqlite::Connection;

use cli::{AddArgs, BudgetArgs, BudgetCommands, BudgetSetArgs, Cli, Commands, DeleteArgs, EditArgs, ListArgs, ProgressArgs, SummaryArgs};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let conn = db::open()?;
    match cli.command {
        Commands::Add(args) => run_add(&conn, args),
        Commands::List(args) => run_list(&conn, args),
        Commands::Edit(args) => run_edit(&conn, args),
        Commands::Delete(args) => run_delete(&conn, args),
        Commands::Summary(args) => run_summary(&conn, args),
        Commands::Budget(args) => run_budget(&conn, args),
        Commands::Progress(args) => run_progress(&conn, args),
    }
}

fn run_add(conn: &Connection, args: AddArgs) -> anyhow::Result<()> {
    if args.amount <= 0 {
        anyhow::bail!("金額は正の整数で入力してください");
    }
    let date = match args.date {
        Some(d) => {
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
                .context("日付は YYYY-MM-DD 形式で入力してください")?;
            d
        }
        None => chrono::Local::now().format("%Y-%m-%d").to_string(),
    };
    let new_tx = repository::NewTransaction {
        name: args.name,
        amount: args.amount,
        date,
        category: args.category,
        memo: args.memo,
    };
    let tx = repository::add(conn, &new_tx)?;
    println!("取引を追加しました (ID: {})", tx.id);
    Ok(())
}

fn run_list(conn: &Connection, args: ListArgs) -> anyhow::Result<()> {
    if let Some(ref m) = args.month {
        validate_month_format(m)?;
    }
    let month = args
        .month
        .or_else(|| Some(chrono::Local::now().format("%Y-%m").to_string()));
    let filter = repository::TransactionFilter {
        month,
        category: args.category,
    };
    let transactions = repository::list(conn, &filter)?;
    if transactions.is_empty() {
        println!("取引がありません");
        return Ok(());
    }
    println!(
        "{:>4}  {:<10}  {:>12}  {:<8}  名称  メモ",
        "ID", "日付", "金額", "カテゴリ"
    );
    println!("{}", "─".repeat(60));
    for tx in &transactions {
        println!(
            "{:>4}  {:<10}  {:>12}  {:<8}  {}  {}",
            tx.id,
            tx.date,
            format_amount(tx.amount),
            tx.category.display_name(),
            tx.name,
            tx.memo.as_deref().unwrap_or(""),
        );
    }
    Ok(())
}

fn run_edit(conn: &Connection, args: EditArgs) -> anyhow::Result<()> {
    if args.name.is_none()
        && args.amount.is_none()
        && args.category.is_none()
        && args.date.is_none()
        && args.memo.is_none()
    {
        anyhow::bail!("更新するフィールドを少なくとも一つ指定してください");
    }
    if let Some(amount) = args.amount
        && amount <= 0
    {
        anyhow::bail!("金額は正の整数で入力してください");
    }
    if let Some(ref d) = args.date {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .context("日付は YYYY-MM-DD 形式で入力してください")?;
    }
    let update = repository::TransactionUpdate {
        name: args.name,
        amount: args.amount,
        date: args.date,
        category: args.category,
        memo: args.memo,
    };
    let tx = repository::edit(conn, args.id, &update)?;
    println!("取引を更新しました (ID: {})", tx.id);
    Ok(())
}

fn run_delete(conn: &Connection, args: DeleteArgs) -> anyhow::Result<()> {
    repository::delete(conn, args.id)?;
    println!("取引を削除しました (ID: {})", args.id);
    Ok(())
}

fn run_budget(conn: &Connection, args: BudgetArgs) -> anyhow::Result<()> {
    match args.command {
        BudgetCommands::Set(set_args) => run_budget_set(conn, set_args),
        BudgetCommands::Show => run_budget_show(conn),
    }
}

fn run_budget_set(conn: &Connection, args: BudgetSetArgs) -> anyhow::Result<()> {
    let new_budget = if let Some(total) = args.total {
        if total <= 0 {
            anyhow::bail!("金額は正の整数で入力してください");
        }
        repository::NewBudget { category: None, amount: total }
    } else if let Some(category) = args.category {
        // clap の requires = "amount" により amount は必ず Some だが、念のため
        let amount = args
            .amount
            .ok_or_else(|| anyhow::anyhow!("--category 指定時は金額を入力してください"))?;
        if amount <= 0 {
            anyhow::bail!("金額は正の整数で入力してください");
        }
        repository::NewBudget { category: Some(category), amount }
    } else {
        anyhow::bail!(
            "月全体（--total）またはカテゴリ（--category）のいずれかを指定してください"
        );
    };
    repository::set_budget(conn, &new_budget)?;
    println!("予算を設定しました");
    Ok(())
}

fn run_budget_show(conn: &Connection) -> anyhow::Result<()> {
    let budgets = repository::list_budgets(conn)?;
    if budgets.is_empty() {
        println!("予算が設定されていません");
        return Ok(());
    }
    println!("== 予算設定 ==");
    println!();
    for budget in &budgets {
        let label = match budget.category {
            None => "月全体".to_string(),
            Some(c) => c.display_name().to_string(),
        };
        println!(
            "{}{}",
            pad_display(&label, 14),
            right_align(&format_amount(budget.amount), 12),
        );
    }
    Ok(())
}

fn run_progress(conn: &Connection, args: ProgressArgs) -> anyhow::Result<()> {
    let current_month = chrono::Local::now().format("%Y-%m").to_string();
    run_progress_for_month(conn, &args, &current_month)
}

fn run_progress_for_month(
    conn: &Connection,
    args: &ProgressArgs,
    current_month: &str,
) -> anyhow::Result<()> {
    let show_total = !args.by_category;
    let show_by_category = !args.total;

    let current_txs = repository::list(
        conn,
        &repository::TransactionFilter {
            month: Some(current_month.to_string()),
            category: None,
        },
    )?;
    let current_totals = build_category_totals(&current_txs);
    let current_expense: i64 = current_totals
        .iter()
        .filter(|(c, _)| !c.is_income())
        .map(|(_, &v)| v)
        .sum();

    if args.last_month {
        print_last_month_progress(
            conn,
            current_month,
            &current_totals,
            current_expense,
            show_total,
            show_by_category,
        )
    } else {
        print_budget_progress(
            conn,
            current_month,
            &current_totals,
            current_expense,
            show_total,
            show_by_category,
        )
    }
}

fn print_budget_progress(
    conn: &Connection,
    current_month: &str,
    current_totals: &HashMap<model::Category, i64>,
    current_expense: i64,
    show_total: bool,
    show_by_category: bool,
) -> anyhow::Result<()> {
    let budgets = repository::list_budgets(conn)?;
    let total_budget = budgets.iter().find(|b| b.category.is_none());
    let category_budgets: HashMap<model::Category, i64> = budgets
        .iter()
        .filter_map(|b| b.category.map(|c| (c, b.amount)))
        .collect();

    if show_total && total_budget.is_none() {
        anyhow::bail!(
            "月全体の予算が設定されていません。`budget set --total <金額>` で設定してください"
        );
    }
    if show_by_category && category_budgets.is_empty() {
        anyhow::bail!(
            "カテゴリ別予算が設定されていません。`budget set --category <カテゴリ> <金額>` で設定してください"
        );
    }

    println!(
        "== {} 消費進捗率（予算対比）==",
        format_month_display(current_month)
    );

    if show_total
        && let Some(tb) = total_budget
    {
        let pct = calc_percentage(current_expense, tb.amount);
        println!();
        println!("[ 月全体 ]");
        println!(
            "{}{}",
            pad_display("予算上限", 10),
            right_align(&format_amount(tb.amount), 12),
        );
        println!(
            "{}{}",
            pad_display("現在支出", 10),
            right_align(&format_amount(current_expense), 12),
        );
        println!(
            "{}{}  [{}]",
            pad_display("進捗率", 10),
            right_align(&format!("{:.1}%", pct), 8),
            progress_bar(pct),
        );
    }

    if show_by_category {
        println!();
        println!("[ カテゴリ別 ]");
        for category in model::EXPENSE_CATEGORIES {
            let base = match category_budgets.get(category) {
                Some(&b) => b,
                None => continue,
            };
            let current = current_totals.get(category).copied().unwrap_or(0);
            let pct = calc_percentage(current, base);
            println!(
                "{}  {} / {}  {}  [{}]",
                pad_display(category.display_name(), 12),
                right_align(&format_amount_raw(current), 8),
                right_align(&format_amount(base), 10),
                right_align(&format!("{:.1}%", pct), 7),
                progress_bar(pct),
            );
        }
    }

    Ok(())
}

fn print_last_month_progress(
    conn: &Connection,
    current_month: &str,
    current_totals: &HashMap<model::Category, i64>,
    current_expense: i64,
    show_total: bool,
    show_by_category: bool,
) -> anyhow::Result<()> {
    let last = prev_month(current_month)?;
    let last_txs = repository::list(
        conn,
        &repository::TransactionFilter {
            month: Some(last.clone()),
            category: None,
        },
    )?;

    let last_totals = build_category_totals(&last_txs);
    let last_expense: i64 = last_totals
        .iter()
        .filter(|(c, _)| !c.is_income())
        .map(|(_, &v)| v)
        .sum();

    println!(
        "== {} 消費進捗率（昨月実績対比）==",
        format_month_display(current_month)
    );

    if show_total {
        println!();
        println!("[ 月全体 ]");
        if last_expense > 0 {
            let pct = calc_percentage(current_expense, last_expense);
            println!(
                "{}{}",
                pad_display("昨月実績", 10),
                right_align(&format_amount(last_expense), 12),
            );
            println!(
                "{}{}",
                pad_display("現在支出", 10),
                right_align(&format_amount(current_expense), 12),
            );
            println!(
                "{}{}  [{}]",
                pad_display("進捗率", 10),
                right_align(&format!("{:.1}%", pct), 8),
                progress_bar(pct),
            );
        } else {
            println!("{}（支出データなし）", pad_display("昨月実績", 10));
            println!(
                "{}{}",
                pad_display("現在支出", 10),
                right_align(&format_amount(current_expense), 12),
            );
            println!("{}N/A  [----------]", pad_display("進捗率", 10));
        }
    }

    if show_by_category {
        println!();
        println!("[ カテゴリ別 ]");
        for category in model::EXPENSE_CATEGORIES {
            let base = last_totals.get(category).copied().unwrap_or(0);
            let current = current_totals.get(category).copied().unwrap_or(0);
            if base == 0 && current == 0 {
                continue;
            }
            if base == 0 {
                println!(
                    "{}  {} / （データなし）  N/A  [----------]",
                    pad_display(category.display_name(), 12),
                    right_align(&format_amount_raw(current), 8),
                );
            } else {
                let pct = calc_percentage(current, base);
                println!(
                    "{}  {} / {}  {}  [{}]",
                    pad_display(category.display_name(), 12),
                    right_align(&format_amount_raw(current), 8),
                    right_align(&format_amount(base), 10),
                    right_align(&format!("{:.1}%", pct), 7),
                    progress_bar(pct),
                );
            }
        }
    }

    Ok(())
}

/// 進捗率を計算する（0.0〜100.0+）。base が 0 の場合は 0.0 を返す。
fn calc_percentage(current: i64, base: i64) -> f64 {
    if base == 0 {
        return 0.0;
    }
    (current as f64 / base as f64) * 100.0
}

/// 10 分割のプログレスバー文字列を返す（例: 80% → "████████░░"）。
fn progress_bar(percentage: f64) -> String {
    let filled = ((percentage / 100.0).min(1.0) * 10.0).floor() as usize;
    let empty = 10 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// 金額をカンマ区切りで「円」なしにフォーマットする（例: 1500 → "1,500"）。
fn format_amount_raw(amount: i64) -> String {
    let s = format_amount(amount);
    s.trim_end_matches('円').to_string()
}

/// "YYYY-MM" の前月を "YYYY-MM" 形式で返す。
fn prev_month(month: &str) -> anyhow::Result<String> {
    let mut parts = month.splitn(2, '-');
    let year: i32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("月の形式が正しくありません: {}", month))?;
    let m: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("月の形式が正しくありません: {}", month))?;
    if m == 1 {
        Ok(format!("{:04}-12", year - 1))
    } else {
        Ok(format!("{:04}-{:02}", year, m - 1))
    }
}


fn run_summary(conn: &Connection, args: SummaryArgs) -> anyhow::Result<()> {
    if let Some(ref m) = args.month {
        validate_month_format(m)?;
    }
    let month = args
        .month
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m").to_string());
    let transactions = repository::list(
        conn,
        &repository::TransactionFilter {
            month: Some(month.clone()),
            category: None,
        },
    )?;
    let totals = build_category_totals(&transactions);
    if args.by_category {
        print_by_category_summary(&month, &totals);
    } else {
        print_monthly_summary(&month, &totals);
    }
    Ok(())
}

/// 取引リストをカテゴリごとに合計した HashMap を返す。
fn build_category_totals(
    transactions: &[model::Transaction],
) -> HashMap<model::Category, i64> {
    let mut totals: HashMap<model::Category, i64> = HashMap::new();
    for tx in transactions {
        *totals.entry(tx.category).or_insert(0) += tx.amount;
    }
    totals
}

/// 月次集計を出力する。
fn print_monthly_summary(month: &str, totals: &HashMap<model::Category, i64>) {
    println!("== {} 集計 ==", format_month_display(month));

    let income_total = totals
        .get(&model::Category::Income)
        .copied()
        .unwrap_or(0);
    println!();
    println!("収入");
    if income_total > 0 {
        println!(
            "  {}{}",
            pad_display(model::Category::Income.display_name(), 14),
            right_align(&format_amount(income_total), 12),
        );
    }

    let expense_total: i64 = totals
        .iter()
        .filter(|(c, _)| !c.is_income())
        .map(|(_, &v)| v)
        .sum();
    println!();
    println!("支出");
    for category in model::EXPENSE_CATEGORIES {
        if let Some(&amount) = totals.get(category) {
            println!(
                "  {}{}",
                pad_display(category.display_name(), 14),
                right_align(&format_amount(amount), 12),
            );
        }
    }
    println!("  {}", "─".repeat(26));
    println!(
        "  {}{}",
        pad_display("合計", 14),
        right_align(&format_amount(expense_total), 12),
    );

    let net = income_total - expense_total;
    println!();
    println!(
        "{}{}",
        pad_display("収支", 16),
        right_align(&format_signed_amount(net), 12),
    );
}

/// カテゴリ別集計を出力する。
fn print_by_category_summary(month: &str, totals: &HashMap<model::Category, i64>) {
    println!("== {} カテゴリ別集計 ==", format_month_display(month));
    println!();

    // 収入を先頭、支出を以降に表示する
    let all_categories = [&[model::Category::Income] as &[_], model::EXPENSE_CATEGORIES].concat();

    let rows: Vec<(&str, i64)> = all_categories
        .iter()
        .filter_map(|c| totals.get(c).map(|&a| (c.display_name(), a)))
        .collect();

    if rows.is_empty() {
        println!("取引がありません");
        return;
    }

    println!(
        "{}{}",
        pad_display("カテゴリ", 14),
        right_align("金額", 12),
    );
    println!("{}", "─".repeat(26));
    for (name, amount) in &rows {
        println!(
            "{}{}",
            pad_display(name, 14),
            right_align(&format_amount(*amount), 12),
        );
    }
}

/// "YYYY-MM" を "YYYY年MM月" に変換する。
fn format_month_display(month: &str) -> String {
    let mut parts = month.splitn(2, '-');
    match (parts.next(), parts.next()) {
        (Some(y), Some(m)) => format!("{y}年{m}月"),
        _ => month.to_string(),
    }
}

/// `--month` 引数が YYYY-MM 形式かどうかを検証する。
fn validate_month_format(month: &str) -> anyhow::Result<()> {
    chrono::NaiveDate::parse_from_str(&format!("{}-01", month), "%Y-%m-%d")
        .context("月は YYYY-MM 形式で入力してください")?;
    Ok(())
}

/// 金額に符号を付けてフォーマットする（例: 80000 → "+80,000円"）。
fn format_signed_amount(amount: i64) -> String {
    if amount >= 0 {
        format!("+{}", format_amount(amount))
    } else {
        format!("-{}", format_amount(amount.abs()))
    }
}

/// 文字列を指定した表示幅になるよう右側にスペースを埋める。非 ASCII 文字を幅 2 として計算する。
fn pad_display(s: &str, width: usize) -> String {
    let w: usize = s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - w))
    }
}

/// 文字列を指定幅で右寄せする（幅が足りない場合は左にスペースを埋める）。
fn right_align(s: &str, width: usize) -> String {
    let w: usize = s.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
    if w >= width {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(width - w), s)
    }
}

/// 金額をカンマ区切りの円表記にフォーマットする（例: 1500 → "1,500円"）。
fn format_amount(amount: i64) -> String {
    let s = amount.to_string();
    let mut reversed = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            reversed.push(',');
        }
        reversed.push(c);
    }
    format!("{}円", reversed.chars().rev().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::{AddArgs, BudgetArgs, BudgetCommands, BudgetSetArgs, DeleteArgs, EditArgs, ListArgs, ProgressArgs, SummaryArgs},
        db,
        model::{Category, Transaction},
        repository,
    };

    fn setup_db() -> anyhow::Result<Connection> {
        let conn = Connection::open_in_memory()?;
        db::migrate(&conn)?;
        Ok(conn)
    }

    fn default_add_args() -> AddArgs {
        AddArgs {
            name: "テスト購入".to_string(),
            amount: 1000,
            category: Category::Food,
            date: Some("2025-04-15".to_string()),
            memo: None,
        }
    }

    // 正常系: add が取引を DB に挿入すること
    #[test]
    fn run_add_inserts_transaction_to_db() -> anyhow::Result<()> {
        let conn = setup_db()?;

        run_add(&conn, default_add_args())?;

        let txs = repository::list(
            &conn,
            &repository::TransactionFilter {
                month: None,
                category: None,
            },
        )?;
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].name, "テスト購入");
        assert_eq!(txs[0].amount, 1000);
        Ok(())
    }

    // 異常系: 金額が 0 以下の場合はエラーになること
    #[test]
    fn run_add_rejects_non_positive_amount() -> anyhow::Result<()> {
        let conn = setup_db()?;
        let args = AddArgs {
            amount: 0,
            ..default_add_args()
        };

        let result = run_add(&conn, args);

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: 不正な日付フォーマットはエラーになること
    #[test]
    fn run_add_rejects_invalid_date_format() -> anyhow::Result<()> {
        let conn = setup_db()?;
        let args = AddArgs {
            date: Some("2025/04/15".to_string()),
            ..default_add_args()
        };

        let result = run_add(&conn, args);

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: 取引なしで list が Ok を返すこと
    #[test]
    fn run_list_with_empty_table_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_list(
            &conn,
            ListArgs {
                month: Some("2025-04".to_string()),
                category: None,
            },
        );

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: 取引ありで list が Ok を返すこと
    #[test]
    fn run_list_with_transactions_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;

        let result = run_list(
            &conn,
            ListArgs {
                month: Some("2025-04".to_string()),
                category: None,
            },
        );

        assert!(result.is_ok());
        Ok(())
    }

    // 異常系: list に不正な --month 書式を渡すとエラーになること
    #[test]
    fn run_list_rejects_invalid_month_format() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_list(
            &conn,
            ListArgs {
                month: Some("2025/04".to_string()),
                category: None,
            },
        );

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: summary に不正な --month 書式を渡すとエラーになること
    #[test]
    fn run_summary_rejects_invalid_month_format() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_summary(
            &conn,
            SummaryArgs {
                month: Some("invalid".to_string()),
                by_category: false,
            },
        );

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: カンマ区切りのフォーマットが正しいこと
    #[test]
    fn format_amount_inserts_commas_correctly() {
        assert_eq!(format_amount(500), "500円");
        assert_eq!(format_amount(1500), "1,500円");
        assert_eq!(format_amount(200000), "200,000円");
    }

    fn default_edit_args(id: i64) -> EditArgs {
        EditArgs {
            id,
            name: None,
            amount: Some(2000),
            category: None,
            date: None,
            memo: None,
        }
    }

    // 正常系: edit が指定フィールドを DB に反映すること
    #[test]
    fn run_edit_updates_specified_field() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;
        let txs = repository::list(
            &conn,
            &repository::TransactionFilter {
                month: None,
                category: None,
            },
        )?;
        let id = txs[0].id;

        run_edit(&conn, default_edit_args(id))?;

        let updated = repository::list(
            &conn,
            &repository::TransactionFilter {
                month: None,
                category: None,
            },
        )?;
        assert_eq!(updated[0].amount, 2000);
        Ok(())
    }

    // 異常系: 更新フィールドが一つも指定されていない場合はエラーになること
    #[test]
    fn run_edit_with_no_fields_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;
        let args = EditArgs {
            id: 1,
            name: None,
            amount: None,
            category: None,
            date: None,
            memo: None,
        };

        let result = run_edit(&conn, args);

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: 金額が 0 以下の場合はエラーになること
    #[test]
    fn run_edit_rejects_non_positive_amount() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;
        let args = EditArgs {
            amount: Some(-1),
            ..default_edit_args(1)
        };

        let result = run_edit(&conn, args);

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: 不正な日付フォーマットはエラーになること
    #[test]
    fn run_edit_rejects_invalid_date_format() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;
        let args = EditArgs {
            amount: None,
            date: Some("2025/04/15".to_string()),
            ..default_edit_args(1)
        };

        let result = run_edit(&conn, args);

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: 存在しない ID を編集するとエラーになること
    #[test]
    fn run_edit_with_nonexistent_id_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_edit(&conn, default_edit_args(999));

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: delete が取引を DB から削除すること
    #[test]
    fn run_delete_removes_transaction() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;
        let txs = repository::list(
            &conn,
            &repository::TransactionFilter {
                month: None,
                category: None,
            },
        )?;
        let id = txs[0].id;

        run_delete(&conn, DeleteArgs { id })?;

        let remaining = repository::list(
            &conn,
            &repository::TransactionFilter {
                month: None,
                category: None,
            },
        )?;
        assert!(remaining.is_empty());
        Ok(())
    }

    // 異常系: 存在しない ID を削除するとエラーになること
    #[test]
    fn run_delete_with_nonexistent_id_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_delete(&conn, DeleteArgs { id: 999 });

        assert!(result.is_err());
        Ok(())
    }

    fn make_tx(id: i64, amount: i64, category: Category) -> Transaction {
        Transaction {
            id,
            name: "テスト".to_string(),
            amount,
            date: "2025-04-01".to_string(),
            category,
            memo: None,
            created_at: "2025-04-01 00:00:00".to_string(),
        }
    }

    // 正常系: カテゴリ集計が正しく合算されること
    #[test]
    fn build_category_totals_aggregates_correctly() {
        let transactions = vec![
            make_tx(1, 1000, Category::Food),
            make_tx(2, 500, Category::Food),
            make_tx(3, 200000, Category::Income),
            make_tx(4, 60000, Category::Fixed),
        ];

        let totals = build_category_totals(&transactions);

        assert_eq!(totals.get(&Category::Food), Some(&1500));
        assert_eq!(totals.get(&Category::Income), Some(&200000));
        assert_eq!(totals.get(&Category::Fixed), Some(&60000));
        assert_eq!(totals.get(&Category::Transport), None);
    }

    // 正常系: 取引なしで summary が Ok を返すこと
    #[test]
    fn run_summary_with_empty_table_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_summary(
            &conn,
            SummaryArgs {
                month: Some("2025-04".to_string()),
                by_category: false,
            },
        );

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: 取引ありで summary が Ok を返すこと
    #[test]
    fn run_summary_with_transactions_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;
        run_add(
            &conn,
            AddArgs {
                name: "給料".to_string(),
                amount: 200000,
                category: Category::Income,
                date: Some("2025-04-25".to_string()),
                memo: None,
            },
        )?;

        let result = run_summary(
            &conn,
            SummaryArgs {
                month: Some("2025-04".to_string()),
                by_category: false,
            },
        );

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: --by-category で summary が Ok を返すこと
    #[test]
    fn run_summary_by_category_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(&conn, default_add_args())?;

        let result = run_summary(
            &conn,
            SummaryArgs {
                month: Some("2025-04".to_string()),
                by_category: true,
            },
        );

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: format_month_display が正しく変換されること
    #[test]
    fn format_month_display_converts_correctly() {
        assert_eq!(format_month_display("2025-04"), "2025年04月");
        assert_eq!(format_month_display("2025-12"), "2025年12月");
    }

    // 正常系: format_signed_amount が符号付きで出力されること
    #[test]
    fn format_signed_amount_shows_sign() {
        assert_eq!(format_signed_amount(80000), "+80,000円");
        assert_eq!(format_signed_amount(-20000), "-20,000円");
        assert_eq!(format_signed_amount(0), "+0円");
    }

    fn budget_set_args(total: Option<i64>, category: Option<Category>, amount: Option<i64>) -> BudgetArgs {
        BudgetArgs {
            command: BudgetCommands::Set(BudgetSetArgs { total, category, amount }),
        }
    }

    // 正常系: budget set --total が予算を DB に登録すること
    #[test]
    fn run_budget_set_total_inserts_budget() -> anyhow::Result<()> {
        let conn = setup_db()?;

        run_budget(&conn, budget_set_args(Some(150000), None, None))?;

        let budgets = repository::list_budgets(&conn)?;
        assert_eq!(budgets.len(), 1);
        assert!(budgets[0].category.is_none());
        assert_eq!(budgets[0].amount, 150000);
        Ok(())
    }

    // 正常系: budget set --category が予算を DB に登録すること
    #[test]
    fn run_budget_set_category_inserts_budget() -> anyhow::Result<()> {
        let conn = setup_db()?;

        run_budget(&conn, budget_set_args(None, Some(Category::Food), Some(40000)))?;

        let budgets = repository::list_budgets(&conn)?;
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].category, Some(Category::Food));
        assert_eq!(budgets[0].amount, 40000);
        Ok(())
    }

    // 正常系: 同一条件で再設定すると1件だけ残ること
    #[test]
    fn run_budget_set_overwrites_existing() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_budget(&conn, budget_set_args(Some(100000), None, None))?;

        run_budget(&conn, budget_set_args(Some(150000), None, None))?;

        let budgets = repository::list_budgets(&conn)?;
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].amount, 150000);
        Ok(())
    }

    // 異常系: 金額が 0 以下の場合はエラーになること
    #[test]
    fn run_budget_set_rejects_non_positive_amount() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_budget(&conn, budget_set_args(Some(0), None, None));

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: --total も --category も指定しない場合はエラーになること
    #[test]
    fn run_budget_set_without_flags_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_budget(&conn, budget_set_args(None, None, None));

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: 予算なしで budget show が Ok を返すこと
    #[test]
    fn run_budget_show_with_empty_table_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = run_budget(&conn, BudgetArgs { command: BudgetCommands::Show });

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: 予算ありで budget show が Ok を返すこと
    #[test]
    fn run_budget_show_with_budgets_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_budget(&conn, budget_set_args(Some(150000), None, None))?;
        run_budget(&conn, budget_set_args(None, Some(Category::Food), Some(40000)))?;

        let result = run_budget(&conn, BudgetArgs { command: BudgetCommands::Show });

        assert!(result.is_ok());
        Ok(())
    }

    fn default_progress_args() -> ProgressArgs {
        ProgressArgs { total: false, by_category: false, last_month: false }
    }

    // 正常系: progress（デフォルト）が予算設定済みで Ok を返すこと
    #[test]
    fn run_progress_default_mode_with_budgets_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_budget(&conn, budget_set_args(Some(150000), None, None))?;
        run_budget(&conn, budget_set_args(None, Some(Category::Food), Some(40000)))?;
        run_add(
            &conn,
            AddArgs {
                name: "食費".to_string(),
                amount: 5000,
                category: Category::Food,
                date: Some("2025-04-01".to_string()),
                memo: None,
            },
        )?;

        let result = run_progress_for_month(&conn, &default_progress_args(), "2025-04");

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: progress --total が月全体予算設定済みで Ok を返すこと
    #[test]
    fn run_progress_total_only_with_budget_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_budget(&conn, budget_set_args(Some(150000), None, None))?;

        let args = ProgressArgs { total: true, by_category: false, last_month: false };
        let result = run_progress_for_month(&conn, &args, "2025-04");

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: progress --by-category がカテゴリ予算設定済みで Ok を返すこと
    #[test]
    fn run_progress_by_category_with_budget_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_budget(&conn, budget_set_args(None, Some(Category::Food), Some(40000)))?;

        let args = ProgressArgs { total: false, by_category: true, last_month: false };
        let result = run_progress_for_month(&conn, &args, "2025-04");

        assert!(result.is_ok());
        Ok(())
    }

    // 異常系: 月全体予算未設定で progress --total はエラーになること
    #[test]
    fn run_progress_total_without_budget_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let args = ProgressArgs { total: true, by_category: false, last_month: false };
        let result = run_progress_for_month(&conn, &args, "2025-04");

        assert!(result.is_err());
        Ok(())
    }

    // 異常系: カテゴリ予算未設定で progress --by-category はエラーになること
    #[test]
    fn run_progress_by_category_without_budget_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let args = ProgressArgs { total: false, by_category: true, last_month: false };
        let result = run_progress_for_month(&conn, &args, "2025-04");

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: progress --last-month が昨月データありで Ok を返すこと
    #[test]
    fn run_progress_last_month_with_data_returns_ok() -> anyhow::Result<()> {
        let conn = setup_db()?;
        // 昨月（2025-04）のデータを追加
        run_add(
            &conn,
            AddArgs {
                name: "食費".to_string(),
                amount: 30000,
                category: Category::Food,
                date: Some("2025-04-01".to_string()),
                memo: None,
            },
        )?;

        let args = ProgressArgs { total: false, by_category: false, last_month: true };
        let result = run_progress_for_month(&conn, &args, "2025-05");

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: progress --last-month が昨月取引なしの場合 N/A を表示して Ok を返すこと
    #[test]
    fn run_progress_last_month_without_data_shows_na() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let args = ProgressArgs { total: false, by_category: false, last_month: true };
        let result = run_progress_for_month(&conn, &args, "2025-05");

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: progress --last-month が昨月収入のみの場合 N/A を表示して Ok を返すこと
    #[test]
    fn run_progress_last_month_income_only_shows_na() -> anyhow::Result<()> {
        let conn = setup_db()?;
        run_add(
            &conn,
            AddArgs {
                name: "給料".to_string(),
                amount: 200000,
                category: Category::Income,
                date: Some("2025-04-25".to_string()),
                memo: None,
            },
        )?;

        let args = ProgressArgs { total: false, by_category: false, last_month: true };
        let result = run_progress_for_month(&conn, &args, "2025-05");

        assert!(result.is_ok());
        Ok(())
    }

    // 正常系: calc_percentage が正しく計算されること
    #[test]
    fn calc_percentage_computes_correctly() {
        assert_eq!(calc_percentage(120000, 150000), 80.0);
        assert_eq!(calc_percentage(0, 100), 0.0);
        assert_eq!(calc_percentage(150, 100), 150.0);
        assert_eq!(calc_percentage(0, 0), 0.0);
    }

    // 正常系: プログレスバーが正しいセグメント数で表示されること
    #[test]
    fn progress_bar_shows_correct_segments() {
        assert_eq!(progress_bar(0.0), "░░░░░░░░░░");
        assert_eq!(progress_bar(53.3), "█████░░░░░");
        assert_eq!(progress_bar(80.0), "████████░░");
        assert_eq!(progress_bar(88.9), "████████░░");
        assert_eq!(progress_bar(100.0), "██████████");
        assert_eq!(progress_bar(106.7), "██████████");
    }

    // 正常系: prev_month が前月を正しく返すこと
    #[test]
    fn prev_month_returns_correct_month() -> anyhow::Result<()> {
        assert_eq!(prev_month("2025-05")?, "2025-04");
        assert_eq!(prev_month("2025-01")?, "2024-12");
        assert_eq!(prev_month("2025-12")?, "2025-11");
        Ok(())
    }
}
