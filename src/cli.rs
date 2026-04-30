use clap::{Args, Parser, Subcommand};

use crate::model::Category;

/// CLI家計簿アプリ
#[derive(Parser)]
#[command(name = "kakeibo", about = "CLI家計簿アプリ")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 取引を追加する
    Add(AddArgs),
    /// 取引の一覧を表示する
    List(ListArgs),
    /// 取引を編集する
    Edit(EditArgs),
    /// 取引を削除する
    Delete(DeleteArgs),
    /// 月次集計を表示する
    Summary(SummaryArgs),
    /// 予算を管理する
    Budget(BudgetArgs),
    /// 消費進捗率を表示する
    Progress(ProgressArgs),
}

/// `add` サブコマンドの引数
#[derive(Args)]
pub struct AddArgs {
    /// 名称
    #[arg(short = 'n', long)]
    pub name: String,

    /// 金額（円、正の整数）
    #[arg(short = 'a', long)]
    pub amount: i64,

    /// カテゴリ識別子（例: food, fixed, income）
    #[arg(short = 'c', long)]
    pub category: Category,

    /// 日付（YYYY-MM-DD）。省略時は実行日
    #[arg(short = 'd', long)]
    pub date: Option<String>,

    /// メモ
    #[arg(short = 'm', long)]
    pub memo: Option<String>,
}

/// `edit` サブコマンドの引数
#[derive(Args)]
pub struct EditArgs {
    /// 更新対象の取引 ID
    pub id: i64,

    /// 名称
    #[arg(short = 'n', long)]
    pub name: Option<String>,

    /// 金額（円、正の整数）
    #[arg(short = 'a', long)]
    pub amount: Option<i64>,

    /// カテゴリ識別子（例: food, fixed, income）
    #[arg(short = 'c', long)]
    pub category: Option<Category>,

    /// 日付（YYYY-MM-DD）
    #[arg(short = 'd', long)]
    pub date: Option<String>,

    /// メモ
    #[arg(short = 'm', long)]
    pub memo: Option<String>,
}

/// `delete` サブコマンドの引数
#[derive(Args)]
pub struct DeleteArgs {
    /// 削除対象の取引 ID
    pub id: i64,
}

/// `budget` サブコマンドの引数
#[derive(Args)]
pub struct BudgetArgs {
    #[command(subcommand)]
    pub command: BudgetCommands,
}

#[derive(Subcommand)]
pub enum BudgetCommands {
    /// 予算上限を設定する
    Set(BudgetSetArgs),
    /// 予算設定を表示する
    Show,
}

/// `budget set` サブコマンドの引数
#[derive(Args)]
pub struct BudgetSetArgs {
    /// 月全体の上限金額（--category との併用不可）
    #[arg(long, conflicts_with = "category")]
    pub total: Option<i64>,

    /// カテゴリ識別子（例: food, fixed）
    #[arg(long, conflicts_with = "total", requires = "amount")]
    pub category: Option<Category>,

    /// カテゴリ別予算の上限金額（--category 使用時に指定）
    pub amount: Option<i64>,
}

/// `summary` サブコマンドの引数
#[derive(Args)]
pub struct SummaryArgs {
    /// 対象月（YYYY-MM）。省略時は当月
    #[arg(long)]
    pub month: Option<String>,

    /// カテゴリ別集計モード
    #[arg(long)]
    pub by_category: bool,
}

/// `list` サブコマンドの引数
#[derive(Args)]
pub struct ListArgs {
    /// 対象月（YYYY-MM）。省略時は当月
    #[arg(long)]
    pub month: Option<String>,

    /// カテゴリ絞り込み
    #[arg(short = 'c', long)]
    pub category: Option<Category>,
}

/// `progress` サブコマンドの引数
#[derive(Args)]
pub struct ProgressArgs {
    /// 月全体のみ表示する（--by-category との併用不可）
    #[arg(long, conflicts_with = "by_category")]
    pub total: bool,

    /// カテゴリ別のみ表示する（--total との併用不可）
    #[arg(long, conflicts_with = "total")]
    pub by_category: bool,

    /// 昨月実績対比モード
    #[arg(long)]
    pub last_month: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Category;

    // 正常系: add の必須引数が正しくパースされること
    #[test]
    fn add_args_parses_required_fields() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "kakeibo",
            "add",
            "--name",
            "UberEats",
            "--amount",
            "1500",
            "--category",
            "food",
        ])?;
        let Commands::Add(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.name, "UberEats");
        assert_eq!(args.amount, 1500);
        assert_eq!(args.category, Category::Food);
        assert!(args.date.is_none());
        assert!(args.memo.is_none());
        Ok(())
    }

    // 正常系: add のオプション引数が正しくパースされること
    #[test]
    fn add_args_parses_optional_fields() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "kakeibo",
            "add",
            "--name",
            "UberEats",
            "--amount",
            "1500",
            "--category",
            "food",
            "--date",
            "2025-04-15",
            "--memo",
            "夕食",
        ])?;
        let Commands::Add(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.date.as_deref(), Some("2025-04-15"));
        assert_eq!(args.memo.as_deref(), Some("夕食"));
        Ok(())
    }

    // 正常系: list がオプションなしでパースされること
    #[test]
    fn list_args_parses_without_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "list"])?;
        let Commands::List(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(args.month.is_none());
        assert!(args.category.is_none());
        Ok(())
    }

    // 正常系: list の全オプションが正しくパースされること
    #[test]
    fn list_args_parses_all_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "kakeibo",
            "list",
            "--month",
            "2025-04",
            "--category",
            "food",
        ])?;
        let Commands::List(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.month.as_deref(), Some("2025-04"));
        assert_eq!(args.category, Some(Category::Food));
        Ok(())
    }

    // 異常系: 不正なカテゴリ文字列はパースエラーになること
    #[test]
    fn add_args_rejects_invalid_category() {
        let result = Cli::try_parse_from([
            "kakeibo",
            "add",
            "--name",
            "test",
            "--amount",
            "1000",
            "--category",
            "invalid",
        ]);
        assert!(result.is_err());
    }

    // 正常系: budget set --total がパースされること
    #[test]
    fn budget_set_total_parses_correctly() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "budget", "set", "--total", "150000"])?;
        let Commands::Budget(budget_args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        let BudgetCommands::Set(args) = budget_args.command else {
            anyhow::bail!("予期しないサブコマンドです");
        };
        assert_eq!(args.total, Some(150000));
        assert!(args.category.is_none());
        assert!(args.amount.is_none());
        Ok(())
    }

    // 正常系: budget set --category がパースされること
    #[test]
    fn budget_set_category_parses_correctly() -> anyhow::Result<()> {
        let cli =
            Cli::try_parse_from(["kakeibo", "budget", "set", "--category", "food", "40000"])?;
        let Commands::Budget(budget_args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        let BudgetCommands::Set(args) = budget_args.command else {
            anyhow::bail!("予期しないサブコマンドです");
        };
        assert!(args.total.is_none());
        assert_eq!(args.category, Some(Category::Food));
        assert_eq!(args.amount, Some(40000));
        Ok(())
    }

    // 正常系: budget show がパースされること
    #[test]
    fn budget_show_parses_correctly() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "budget", "show"])?;
        let Commands::Budget(budget_args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(matches!(budget_args.command, BudgetCommands::Show));
        Ok(())
    }

    // 異常系: --total と --category の同時指定はエラーになること
    #[test]
    fn budget_set_total_and_category_are_exclusive() {
        let result = Cli::try_parse_from([
            "kakeibo",
            "budget",
            "set",
            "--total",
            "150000",
            "--category",
            "food",
            "40000",
        ]);
        assert!(result.is_err());
    }

    // 異常系: --category に金額なしはエラーになること
    #[test]
    fn budget_set_category_without_amount_is_error() {
        let result =
            Cli::try_parse_from(["kakeibo", "budget", "set", "--category", "food"]);
        assert!(result.is_err());
    }

    // 正常系: summary がオプションなしでパースされること
    #[test]
    fn summary_args_parses_without_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "summary"])?;
        let Commands::Summary(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(args.month.is_none());
        assert!(!args.by_category);
        Ok(())
    }

    // 正常系: summary --month が正しくパースされること
    #[test]
    fn summary_args_parses_with_month() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "summary", "--month", "2025-04"])?;
        let Commands::Summary(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.month.as_deref(), Some("2025-04"));
        assert!(!args.by_category);
        Ok(())
    }

    // 正常系: summary --by-category が正しくパースされること
    #[test]
    fn summary_args_parses_with_by_category() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "summary", "--by-category"])?;
        let Commands::Summary(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(args.month.is_none());
        assert!(args.by_category);
        Ok(())
    }

    // 正常系: summary の全オプションが正しくパースされること
    #[test]
    fn summary_args_parses_all_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "kakeibo",
            "summary",
            "--by-category",
            "--month",
            "2025-04",
        ])?;
        let Commands::Summary(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.month.as_deref(), Some("2025-04"));
        assert!(args.by_category);
        Ok(())
    }

    // 正常系: edit の ID とオプションが正しくパースされること
    #[test]
    fn edit_args_parses_id_and_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "kakeibo", "edit", "3", "--amount", "2000", "--memo", "新しいメモ",
        ])?;
        let Commands::Edit(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.id, 3);
        assert_eq!(args.amount, Some(2000));
        assert_eq!(args.memo.as_deref(), Some("新しいメモ"));
        assert!(args.name.is_none());
        assert!(args.category.is_none());
        assert!(args.date.is_none());
        Ok(())
    }

    // 正常系: edit がオプションなしで ID だけパースされること
    #[test]
    fn edit_args_parses_id_only() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "edit", "7"])?;
        let Commands::Edit(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.id, 7);
        assert!(args.name.is_none());
        assert!(args.amount.is_none());
        Ok(())
    }

    // 正常系: delete の ID が正しくパースされること
    #[test]
    fn delete_args_parses_id() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "delete", "5"])?;
        let Commands::Delete(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert_eq!(args.id, 5);
        Ok(())
    }

    // 正常系: progress がオプションなしでパースされること
    #[test]
    fn progress_args_parses_without_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "progress"])?;
        let Commands::Progress(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(!args.total);
        assert!(!args.by_category);
        assert!(!args.last_month);
        Ok(())
    }

    // 正常系: progress --total が正しくパースされること
    #[test]
    fn progress_args_parses_total_flag() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "progress", "--total"])?;
        let Commands::Progress(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(args.total);
        assert!(!args.by_category);
        assert!(!args.last_month);
        Ok(())
    }

    // 正常系: progress --by-category が正しくパースされること
    #[test]
    fn progress_args_parses_by_category_flag() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "progress", "--by-category"])?;
        let Commands::Progress(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(!args.total);
        assert!(args.by_category);
        assert!(!args.last_month);
        Ok(())
    }

    // 正常系: progress --last-month が正しくパースされること
    #[test]
    fn progress_args_parses_last_month_flag() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "progress", "--last-month"])?;
        let Commands::Progress(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(!args.total);
        assert!(!args.by_category);
        assert!(args.last_month);
        Ok(())
    }

    // 正常系: progress --last-month --total が正しくパースされること
    #[test]
    fn progress_args_parses_last_month_with_total() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["kakeibo", "progress", "--last-month", "--total"])?;
        let Commands::Progress(args) = cli.command else {
            anyhow::bail!("予期しないコマンドです");
        };
        assert!(args.total);
        assert!(!args.by_category);
        assert!(args.last_month);
        Ok(())
    }

    // 異常系: --total と --by-category の同時指定はエラーになること
    #[test]
    fn progress_args_total_and_by_category_are_exclusive() {
        let result =
            Cli::try_parse_from(["kakeibo", "progress", "--total", "--by-category"]);
        assert!(result.is_err());
    }
}
