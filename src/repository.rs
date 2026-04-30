use anyhow::{anyhow, Context};
use rusqlite::{Connection, Row};

use crate::model::{Budget, Category, Transaction};

/// 取引追加時に渡す入力データ。
pub struct NewTransaction {
    /// 名称
    pub name: String,
    /// 金額（円、正の整数）
    pub amount: i64,
    /// 日付（YYYY-MM-DD）
    pub date: String,
    /// カテゴリ
    pub category: Category,
    /// メモ（任意）
    pub memo: Option<String>,
}

/// 取引編集時の更新データ。`Some` のフィールドのみ上書きする。
pub struct TransactionUpdate {
    /// 名称
    pub name: Option<String>,
    /// 金額（円、正の整数）
    pub amount: Option<i64>,
    /// 日付（YYYY-MM-DD）
    pub date: Option<String>,
    /// カテゴリ
    pub category: Option<Category>,
    /// メモ
    pub memo: Option<String>,
}

/// 取引一覧の絞り込み条件。`None` は絞り込みなし（全件）。
pub struct TransactionFilter {
    /// 対象月（YYYY-MM）
    pub month: Option<String>,
    /// カテゴリ
    pub category: Option<Category>,
}

fn row_to_transaction(row: &Row<'_>) -> anyhow::Result<Transaction> {
    let category_str: String = row.get(4)?;
    let category: Category = category_str
        .parse()
        .context("DBのカテゴリ値を解析できませんでした")?;
    Ok(Transaction {
        id: row.get(0)?,
        name: row.get(1)?,
        amount: row.get(2)?,
        date: row.get(3)?,
        category,
        memo: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn find_by_id(conn: &Connection, id: i64) -> anyhow::Result<Transaction> {
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, date, category, memo, created_at
         FROM transactions WHERE id = ?1",
    )?;
    stmt.query_and_then(rusqlite::params![id], row_to_transaction)?
        .next()
        .transpose()?
        .ok_or_else(|| anyhow!("ID={id} の取引が見つかりません"))
}

/// 取引を追加し、追加後のレコードを返す。
pub fn add(conn: &Connection, new_tx: &NewTransaction) -> anyhow::Result<Transaction> {
    let category_str = new_tx.category.to_string();
    conn.execute(
        "INSERT INTO transactions (name, amount, date, category, memo, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', 'localtime'))",
        rusqlite::params![
            new_tx.name,
            new_tx.amount,
            new_tx.date,
            category_str,
            new_tx.memo,
        ],
    )
    .context("取引の追加に失敗しました")?;
    find_by_id(conn, conn.last_insert_rowid())
}

/// 取引一覧を日付の降順で返す。
pub fn list(conn: &Connection, filter: &TransactionFilter) -> anyhow::Result<Vec<Transaction>> {
    let category_str = filter.category.map(|c| c.to_string());
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, date, category, memo, created_at
         FROM transactions
         WHERE (?1 IS NULL OR strftime('%Y-%m', date) = ?1)
           AND (?2 IS NULL OR category = ?2)
         ORDER BY date DESC, id DESC",
    )?;
    stmt.query_and_then(
        rusqlite::params![filter.month, category_str],
        row_to_transaction,
    )?
    .collect()
}

/// 指定 ID の取引を更新し、更新後のレコードを返す。
pub fn edit(conn: &Connection, id: i64, update: &TransactionUpdate) -> anyhow::Result<Transaction> {
    let current = find_by_id(conn, id)?;

    let name = update.name.as_deref().unwrap_or(&current.name);
    let amount = update.amount.unwrap_or(current.amount);
    let date = update.date.as_deref().unwrap_or(&current.date);
    let category = update.category.unwrap_or(current.category);
    let category_str = category.to_string();
    let memo = update.memo.as_deref().or(current.memo.as_deref());

    conn.execute(
        "UPDATE transactions
         SET name = ?1, amount = ?2, date = ?3, category = ?4, memo = ?5
         WHERE id = ?6",
        rusqlite::params![name, amount, date, category_str, memo, id],
    )
    .context("取引の更新に失敗しました")?;

    find_by_id(conn, id)
}

/// 指定 ID の取引を削除する。
pub fn delete(conn: &Connection, id: i64) -> anyhow::Result<()> {
    let affected = conn
        .execute(
            "DELETE FROM transactions WHERE id = ?1",
            rusqlite::params![id],
        )
        .context("取引の削除に失敗しました")?;
    if affected == 0 {
        return Err(anyhow!("ID={id} の取引が見つかりません"));
    }
    Ok(())
}

/// 予算設定時に渡す入力データ。`month` は常に NULL（全月共通）として登録する。
pub struct NewBudget {
    /// None = 月全体の予算、Some = カテゴリ別予算
    pub category: Option<Category>,
    /// 上限金額（円、正の整数）
    pub amount: i64,
}

fn row_to_budget(row: &Row<'_>) -> anyhow::Result<Budget> {
    let category_str: Option<String> = row.get(2)?;
    let category = category_str
        .map(|s| s.parse::<Category>().context("DBのカテゴリ値を解析できませんでした"))
        .transpose()?;
    Ok(Budget {
        id: row.get(0)?,
        month: row.get(1)?,
        category,
        amount: row.get(3)?,
    })
}

fn find_budget_by_id(conn: &Connection, id: i64) -> anyhow::Result<Budget> {
    let mut stmt = conn.prepare(
        "SELECT id, month, category, amount FROM budgets WHERE id = ?1",
    )?;
    stmt.query_and_then(rusqlite::params![id], row_to_budget)?
        .next()
        .transpose()?
        .ok_or_else(|| anyhow!("ID={id} の予算が見つかりません"))
}

/// 予算を設定する。同一条件の既存レコードがある場合は上書きする。
pub fn set_budget(conn: &Connection, new_budget: &NewBudget) -> anyhow::Result<Budget> {
    let category_str = new_budget.category.map(|c| c.to_string());
    if new_budget.category.is_none() {
        conn.execute(
            "DELETE FROM budgets WHERE month IS NULL AND category IS NULL",
            [],
        )
        .context("既存の月全体予算の削除に失敗しました")?;
    } else {
        conn.execute(
            "DELETE FROM budgets WHERE month IS NULL AND category = ?1",
            rusqlite::params![category_str],
        )
        .context("既存のカテゴリ予算の削除に失敗しました")?;
    }
    conn.execute(
        "INSERT INTO budgets (month, category, amount) VALUES (NULL, ?1, ?2)",
        rusqlite::params![category_str, new_budget.amount],
    )
    .context("予算の設定に失敗しました")?;
    find_budget_by_id(conn, conn.last_insert_rowid())
}

/// 予算設定の一覧を返す。
pub fn list_budgets(conn: &Connection) -> anyhow::Result<Vec<Budget>> {
    let mut stmt = conn.prepare(
        "SELECT id, month, category, amount FROM budgets ORDER BY id ASC",
    )?;
    stmt.query_and_then([], row_to_budget)?.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup_db() -> anyhow::Result<Connection> {
        let conn = Connection::open_in_memory()?;
        db::migrate(&conn)?;
        Ok(conn)
    }

    fn default_new_transaction() -> NewTransaction {
        NewTransaction {
            name: "テスト購入".to_string(),
            amount: 1500,
            date: "2025-04-15".to_string(),
            category: Category::Food,
            memo: None,
        }
    }

    // 正常系: 全フィールドが正しく保存・返却されること
    #[test]
    fn add_transaction_stores_all_fields() -> anyhow::Result<()> {
        let conn = setup_db()?;
        let new_tx = NewTransaction {
            name: "UberEats".to_string(),
            amount: 1500,
            date: "2025-04-15".to_string(),
            category: Category::Food,
            memo: Some("夕食".to_string()),
        };

        let tx = add(&conn, &new_tx)?;

        assert_eq!(tx.name, "UberEats");
        assert_eq!(tx.amount, 1500);
        assert_eq!(tx.date, "2025-04-15");
        assert_eq!(tx.category, Category::Food);
        assert_eq!(tx.memo.as_deref(), Some("夕食"));
        Ok(())
    }

    // 正常系: メモなしで追加した場合 None が返ること
    #[test]
    fn add_transaction_without_memo_stores_none() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let tx = add(&conn, &default_new_transaction())?;

        assert!(tx.memo.is_none());
        Ok(())
    }

    // 正常系: 月フィルタで指定月の取引のみ返ること
    #[test]
    fn list_transactions_filters_by_month() -> anyhow::Result<()> {
        let conn = setup_db()?;
        add(
            &conn,
            &NewTransaction {
                name: "4月購入".to_string(),
                date: "2025-04-10".to_string(),
                ..default_new_transaction()
            },
        )?;
        add(
            &conn,
            &NewTransaction {
                name: "5月購入".to_string(),
                date: "2025-05-01".to_string(),
                ..default_new_transaction()
            },
        )?;

        let result = list(
            &conn,
            &TransactionFilter {
                month: Some("2025-04".to_string()),
                category: None,
            },
        )?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "4月購入");
        Ok(())
    }

    // 正常系: カテゴリフィルタで指定カテゴリのみ返ること
    #[test]
    fn list_transactions_filters_by_category() -> anyhow::Result<()> {
        let conn = setup_db()?;
        add(
            &conn,
            &NewTransaction {
                name: "食費".to_string(),
                category: Category::Food,
                ..default_new_transaction()
            },
        )?;
        add(
            &conn,
            &NewTransaction {
                name: "交通費".to_string(),
                category: Category::Transport,
                ..default_new_transaction()
            },
        )?;

        let result = list(
            &conn,
            &TransactionFilter {
                month: None,
                category: Some(Category::Food),
            },
        )?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "食費");
        Ok(())
    }

    // 正常系: フィルタなしで全件返ること
    #[test]
    fn list_transactions_with_no_filter_returns_all() -> anyhow::Result<()> {
        let conn = setup_db()?;
        add(&conn, &default_new_transaction())?;
        add(&conn, &default_new_transaction())?;

        let result = list(
            &conn,
            &TransactionFilter {
                month: None,
                category: None,
            },
        )?;

        assert_eq!(result.len(), 2);
        Ok(())
    }

    // 正常系: 指定フィールドのみ更新され、他は変わらないこと
    #[test]
    fn edit_transaction_updates_only_specified_fields() -> anyhow::Result<()> {
        let conn = setup_db()?;
        let tx = add(
            &conn,
            &NewTransaction {
                name: "更新前".to_string(),
                amount: 1000,
                ..default_new_transaction()
            },
        )?;

        let updated = edit(
            &conn,
            tx.id,
            &TransactionUpdate {
                name: None,
                amount: Some(2000),
                date: None,
                category: None,
                memo: Some("新しいメモ".to_string()),
            },
        )?;

        // 更新したフィールドが変わっていること
        assert_eq!(updated.amount, 2000);
        assert_eq!(updated.memo.as_deref(), Some("新しいメモ"));
        // 更新していないフィールドが変わっていないこと
        assert_eq!(updated.name, "更新前");
        assert_eq!(updated.date, "2025-04-15");
        assert_eq!(updated.category, Category::Food);
        Ok(())
    }

    // 異常系: 存在しない ID を編集するとエラーになること
    #[test]
    fn edit_transaction_with_nonexistent_id_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = edit(
            &conn,
            999,
            &TransactionUpdate {
                name: Some("更新".to_string()),
                amount: None,
                date: None,
                category: None,
                memo: None,
            },
        );

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: 削除後に取引が存在しないこと
    #[test]
    fn delete_transaction_removes_record() -> anyhow::Result<()> {
        let conn = setup_db()?;
        let tx = add(&conn, &default_new_transaction())?;

        delete(&conn, tx.id)?;

        let remaining = list(
            &conn,
            &TransactionFilter {
                month: None,
                category: None,
            },
        )?;
        assert!(remaining.is_empty());
        Ok(())
    }

    // 異常系: 存在しない ID を削除するとエラーになること
    #[test]
    fn delete_transaction_with_nonexistent_id_returns_error() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let result = delete(&conn, 999);

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: 月全体の予算が正しく保存されること
    #[test]
    fn set_budget_stores_total_budget() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let budget = set_budget(&conn, &NewBudget { category: None, amount: 150000 })?;

        assert!(budget.category.is_none());
        assert!(budget.month.is_none());
        assert_eq!(budget.amount, 150000);
        Ok(())
    }

    // 正常系: カテゴリ別予算が正しく保存されること
    #[test]
    fn set_budget_stores_category_budget() -> anyhow::Result<()> {
        let conn = setup_db()?;

        let budget = set_budget(
            &conn,
            &NewBudget { category: Some(Category::Food), amount: 40000 },
        )?;

        assert_eq!(budget.category, Some(Category::Food));
        assert_eq!(budget.amount, 40000);
        Ok(())
    }

    // 正常系: 同一条件で再設定すると上書きされること
    #[test]
    fn set_budget_overwrites_existing_record() -> anyhow::Result<()> {
        let conn = setup_db()?;
        set_budget(&conn, &NewBudget { category: None, amount: 100000 })?;

        set_budget(&conn, &NewBudget { category: None, amount: 150000 })?;

        let budgets = list_budgets(&conn)?;
        // 上書きにより1件のみ残ること
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].amount, 150000);
        Ok(())
    }

    // 正常系: 複数の予算設定が全件返ること
    #[test]
    fn list_budgets_returns_all_records() -> anyhow::Result<()> {
        let conn = setup_db()?;
        set_budget(&conn, &NewBudget { category: None, amount: 150000 })?;
        set_budget(&conn, &NewBudget { category: Some(Category::Food), amount: 40000 })?;
        set_budget(&conn, &NewBudget { category: Some(Category::Transport), amount: 10000 })?;

        let budgets = list_budgets(&conn)?;

        assert_eq!(budgets.len(), 3);
        Ok(())
    }
}
