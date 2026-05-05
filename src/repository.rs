use anyhow::{Context, anyhow};
use libsql::{Connection, Value};

use crate::model::{Budget, Category, Transaction};

/// 取引追加時に渡す入力データ。
pub struct NewTransaction {
    /// ユーザーID（CLI では "local"、API では Google ID Token の sub クレーム）
    pub user_id: String,
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
    /// ユーザーID
    pub user_id: String,
    /// 対象月（YYYY-MM）
    pub month: Option<String>,
    /// カテゴリ
    pub category: Option<Category>,
}

fn row_to_transaction(row: &libsql::Row) -> anyhow::Result<Transaction> {
    // SELECT id, user_id, name, amount, date, category, memo, created_at
    let category_str: String = row.get(5).context("category カラムの取得に失敗しました")?;
    let category: Category = category_str
        .parse()
        .context("DBのカテゴリ値を解析できませんでした")?;
    Ok(Transaction {
        id: row.get(0).context("id カラムの取得に失敗しました")?,
        user_id: row.get(1).context("user_id カラムの取得に失敗しました")?,
        name: row.get(2).context("name カラムの取得に失敗しました")?,
        amount: row.get(3).context("amount カラムの取得に失敗しました")?,
        date: row.get(4).context("date カラムの取得に失敗しました")?,
        category,
        memo: row.get(6).context("memo カラムの取得に失敗しました")?,
        created_at: row
            .get(7)
            .context("created_at カラムの取得に失敗しました")?,
    })
}

async fn find_by_id(conn: &Connection, id: i64, user_id: &str) -> anyhow::Result<Transaction> {
    let mut rows = conn
        .query(
            "SELECT id, user_id, name, amount, date, category, memo, created_at
             FROM transactions WHERE id = ?1 AND user_id = ?2",
            vec![Value::Integer(id), Value::Text(user_id.to_string())],
        )
        .await?;
    match rows.next().await? {
        Some(row) => row_to_transaction(&row),
        None => Err(anyhow!("ID={id} の取引が見つかりません")),
    }
}

/// 取引を追加し、追加後のレコードを返す。
pub async fn add(conn: &Connection, new_tx: &NewTransaction) -> anyhow::Result<Transaction> {
    let category_str = new_tx.category.to_string();
    let memo_val = new_tx
        .memo
        .as_ref()
        .map(|m| Value::Text(m.clone()))
        .unwrap_or(Value::Null);
    conn.execute(
        "INSERT INTO transactions (user_id, name, amount, date, category, memo, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now', 'localtime'))",
        vec![
            Value::Text(new_tx.user_id.clone()),
            Value::Text(new_tx.name.clone()),
            Value::Integer(new_tx.amount),
            Value::Text(new_tx.date.clone()),
            Value::Text(category_str),
            memo_val,
        ],
    )
    .await
    .context("取引の追加に失敗しました")?;
    find_by_id(conn, conn.last_insert_rowid(), &new_tx.user_id).await
}

/// 取引一覧を日付の降順で返す。
pub async fn list(
    conn: &Connection,
    filter: &TransactionFilter,
) -> anyhow::Result<Vec<Transaction>> {
    let category_str = filter.category.map(|c| c.to_string());
    let month_val = filter
        .month
        .as_ref()
        .map(|m| Value::Text(m.clone()))
        .unwrap_or(Value::Null);
    let cat_val = category_str.map(Value::Text).unwrap_or(Value::Null);
    let mut rows = conn
        .query(
            "SELECT id, user_id, name, amount, date, category, memo, created_at
             FROM transactions
             WHERE user_id = ?1
               AND (?2 IS NULL OR strftime('%Y-%m', date) = ?2)
               AND (?3 IS NULL OR category = ?3)
             ORDER BY date DESC, id DESC",
            vec![Value::Text(filter.user_id.clone()), month_val, cat_val],
        )
        .await?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        result.push(row_to_transaction(&row)?);
    }
    Ok(result)
}

/// 指定 ID の取引を更新し、更新後のレコードを返す。
pub async fn edit(
    conn: &Connection,
    id: i64,
    update: &TransactionUpdate,
    user_id: &str,
) -> anyhow::Result<Transaction> {
    let current = find_by_id(conn, id, user_id).await?;

    let name = update.name.as_deref().unwrap_or(&current.name);
    let amount = update.amount.unwrap_or(current.amount);
    let date = update.date.as_deref().unwrap_or(&current.date);
    let category = update.category.unwrap_or(current.category);
    let category_str = category.to_string();
    let memo = update.memo.as_deref().or(current.memo.as_deref());
    let memo_val = memo
        .map(|m| Value::Text(m.to_string()))
        .unwrap_or(Value::Null);

    conn.execute(
        "UPDATE transactions
         SET name = ?1, amount = ?2, date = ?3, category = ?4, memo = ?5
         WHERE id = ?6 AND user_id = ?7",
        vec![
            Value::Text(name.to_string()),
            Value::Integer(amount),
            Value::Text(date.to_string()),
            Value::Text(category_str),
            memo_val,
            Value::Integer(id),
            Value::Text(user_id.to_string()),
        ],
    )
    .await
    .context("取引の更新に失敗しました")?;

    find_by_id(conn, id, user_id).await
}

/// 指定 ID の取引を削除する。
pub async fn delete(conn: &Connection, id: i64, user_id: &str) -> anyhow::Result<()> {
    let affected = conn
        .execute(
            "DELETE FROM transactions WHERE id = ?1 AND user_id = ?2",
            vec![Value::Integer(id), Value::Text(user_id.to_string())],
        )
        .await
        .context("取引の削除に失敗しました")?;
    if affected == 0 {
        return Err(anyhow!("ID={id} の取引が見つかりません"));
    }
    Ok(())
}

/// 予算設定時に渡す入力データ。`month` は常に NULL（全月共通）として登録する。
pub struct NewBudget {
    /// ユーザーID（CLI では "local"、API では Google ID Token の sub クレーム）
    pub user_id: String,
    /// None = 月全体の予算、Some = カテゴリ別予算
    pub category: Option<Category>,
    /// 上限金額（円、正の整数）
    pub amount: i64,
}

fn row_to_budget(row: &libsql::Row) -> anyhow::Result<Budget> {
    // SELECT id, user_id, month, category, amount
    let category_str: Option<String> = row.get(3).context("category カラムの取得に失敗しました")?;
    let category = category_str
        .map(|s| {
            s.parse::<Category>()
                .context("DBのカテゴリ値を解析できませんでした")
        })
        .transpose()?;
    Ok(Budget {
        id: row.get(0).context("id カラムの取得に失敗しました")?,
        user_id: row.get(1).context("user_id カラムの取得に失敗しました")?,
        month: row.get(2).context("month カラムの取得に失敗しました")?,
        category,
        amount: row.get(4).context("amount カラムの取得に失敗しました")?,
    })
}

async fn find_budget_by_id(conn: &Connection, id: i64, user_id: &str) -> anyhow::Result<Budget> {
    let mut rows = conn
        .query(
            "SELECT id, user_id, month, category, amount FROM budgets WHERE id = ?1 AND user_id = ?2",
            vec![Value::Integer(id), Value::Text(user_id.to_string())],
        )
        .await?;
    match rows.next().await? {
        Some(row) => row_to_budget(&row),
        None => Err(anyhow!("ID={id} の予算が見つかりません")),
    }
}

/// 予算を設定する。同一条件の既存レコードがある場合は上書きする。
pub async fn set_budget(conn: &Connection, new_budget: &NewBudget) -> anyhow::Result<Budget> {
    let category_str = new_budget.category.map(|c| c.to_string());
    if new_budget.category.is_none() {
        conn.execute(
            "DELETE FROM budgets WHERE user_id = ?1 AND month IS NULL AND category IS NULL",
            vec![Value::Text(new_budget.user_id.clone())],
        )
        .await
        .context("既存の月全体予算の削除に失敗しました")?;
    } else {
        let cat_val = category_str
            .as_ref()
            .map(|s| Value::Text(s.clone()))
            .unwrap_or(Value::Null);
        conn.execute(
            "DELETE FROM budgets WHERE user_id = ?1 AND month IS NULL AND category = ?2",
            vec![Value::Text(new_budget.user_id.clone()), cat_val],
        )
        .await
        .context("既存のカテゴリ予算の削除に失敗しました")?;
    }
    let cat_val = category_str.map(Value::Text).unwrap_or(Value::Null);
    conn.execute(
        "INSERT INTO budgets (user_id, month, category, amount) VALUES (?1, NULL, ?2, ?3)",
        vec![
            Value::Text(new_budget.user_id.clone()),
            cat_val,
            Value::Integer(new_budget.amount),
        ],
    )
    .await
    .context("予算の設定に失敗しました")?;
    find_budget_by_id(conn, conn.last_insert_rowid(), &new_budget.user_id).await
}

/// 指定 ID の取引が（user_id に関わらず）存在するかを返す。403 / 404 の判定に使用する。
#[allow(dead_code)]
pub async fn transaction_exists(conn: &Connection, id: i64) -> anyhow::Result<bool> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM transactions WHERE id = ?1",
            vec![Value::Integer(id)],
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

/// 予算設定の一覧を返す。
pub async fn list_budgets(conn: &Connection, user_id: &str) -> anyhow::Result<Vec<Budget>> {
    let mut rows = conn
        .query(
            "SELECT id, user_id, month, category, amount FROM budgets WHERE user_id = ?1 ORDER BY id ASC",
            vec![Value::Text(user_id.to_string())],
        )
        .await?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        result.push(row_to_budget(&row)?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn setup_db() -> anyhow::Result<Connection> {
        let db = libsql::Builder::new_local(":memory:").build().await?;
        let conn = db.connect()?;
        db::migrate(&conn).await?;
        Ok(conn)
    }

    fn default_new_transaction() -> NewTransaction {
        NewTransaction {
            user_id: "local".to_string(),
            name: "テスト購入".to_string(),
            amount: 1500,
            date: "2025-04-15".to_string(),
            category: Category::Food,
            memo: None,
        }
    }

    // 正常系: 全フィールドが正しく保存・返却されること
    #[tokio::test]
    async fn add_transaction_stores_all_fields() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        let new_tx = NewTransaction {
            user_id: "local".to_string(),
            name: "UberEats".to_string(),
            amount: 1500,
            date: "2025-04-15".to_string(),
            category: Category::Food,
            memo: Some("夕食".to_string()),
        };

        let tx = add(&conn, &new_tx).await?;

        assert_eq!(tx.name, "UberEats");
        assert_eq!(tx.amount, 1500);
        assert_eq!(tx.date, "2025-04-15");
        assert_eq!(tx.category, Category::Food);
        assert_eq!(tx.memo.as_deref(), Some("夕食"));
        Ok(())
    }

    // 正常系: メモなしで追加した場合 None が返ること
    #[tokio::test]
    async fn add_transaction_without_memo_stores_none() -> anyhow::Result<()> {
        let conn = setup_db().await?;

        let tx = add(&conn, &default_new_transaction()).await?;

        assert!(tx.memo.is_none());
        Ok(())
    }

    // 正常系: 月フィルタで指定月の取引のみ返ること
    #[tokio::test]
    async fn list_transactions_filters_by_month() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        add(
            &conn,
            &NewTransaction {
                name: "4月購入".to_string(),
                date: "2025-04-10".to_string(),
                ..default_new_transaction()
            },
        )
        .await?;
        add(
            &conn,
            &NewTransaction {
                name: "5月購入".to_string(),
                date: "2025-05-01".to_string(),
                ..default_new_transaction()
            },
        )
        .await?;

        let result = list(
            &conn,
            &TransactionFilter {
                user_id: "local".to_string(),
                month: Some("2025-04".to_string()),
                category: None,
            },
        )
        .await?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "4月購入");
        Ok(())
    }

    // 正常系: カテゴリフィルタで指定カテゴリのみ返ること
    #[tokio::test]
    async fn list_transactions_filters_by_category() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        add(
            &conn,
            &NewTransaction {
                name: "食費".to_string(),
                category: Category::Food,
                ..default_new_transaction()
            },
        )
        .await?;
        add(
            &conn,
            &NewTransaction {
                name: "交通費".to_string(),
                category: Category::Transport,
                ..default_new_transaction()
            },
        )
        .await?;

        let result = list(
            &conn,
            &TransactionFilter {
                user_id: "local".to_string(),
                month: None,
                category: Some(Category::Food),
            },
        )
        .await?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "食費");
        Ok(())
    }

    // 正常系: フィルタなしで全件返ること
    #[tokio::test]
    async fn list_transactions_with_no_filter_returns_all() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        add(&conn, &default_new_transaction()).await?;
        add(&conn, &default_new_transaction()).await?;

        let result = list(
            &conn,
            &TransactionFilter {
                user_id: "local".to_string(),
                month: None,
                category: None,
            },
        )
        .await?;

        assert_eq!(result.len(), 2);
        Ok(())
    }

    // 正常系: 指定フィールドのみ更新され、他は変わらないこと
    #[tokio::test]
    async fn edit_transaction_updates_only_specified_fields() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        let tx = add(
            &conn,
            &NewTransaction {
                name: "更新前".to_string(),
                amount: 1000,
                ..default_new_transaction()
            },
        )
        .await?;

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
            "local",
        )
        .await?;

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
    #[tokio::test]
    async fn edit_transaction_with_nonexistent_id_returns_error() -> anyhow::Result<()> {
        let conn = setup_db().await?;

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
            "local",
        )
        .await;

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: 削除後に取引が存在しないこと
    #[tokio::test]
    async fn delete_transaction_removes_record() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        let tx = add(&conn, &default_new_transaction()).await?;

        delete(&conn, tx.id, "local").await?;

        let remaining = list(
            &conn,
            &TransactionFilter {
                user_id: "local".to_string(),
                month: None,
                category: None,
            },
        )
        .await?;
        assert!(remaining.is_empty());
        Ok(())
    }

    // 異常系: 存在しない ID を削除するとエラーになること
    #[tokio::test]
    async fn delete_transaction_with_nonexistent_id_returns_error() -> anyhow::Result<()> {
        let conn = setup_db().await?;

        let result = delete(&conn, 999, "local").await;

        assert!(result.is_err());
        Ok(())
    }

    // 正常系: 月全体の予算が正しく保存されること
    #[tokio::test]
    async fn set_budget_stores_total_budget() -> anyhow::Result<()> {
        let conn = setup_db().await?;

        let budget = set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: None,
                amount: 150000,
            },
        )
        .await?;

        assert!(budget.category.is_none());
        assert!(budget.month.is_none());
        assert_eq!(budget.amount, 150000);
        Ok(())
    }

    // 正常系: カテゴリ別予算が正しく保存されること
    #[tokio::test]
    async fn set_budget_stores_category_budget() -> anyhow::Result<()> {
        let conn = setup_db().await?;

        let budget = set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: Some(Category::Food),
                amount: 40000,
            },
        )
        .await?;

        assert_eq!(budget.category, Some(Category::Food));
        assert_eq!(budget.amount, 40000);
        Ok(())
    }

    // 正常系: 同一条件で再設定すると上書きされること
    #[tokio::test]
    async fn set_budget_overwrites_existing_record() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: None,
                amount: 100000,
            },
        )
        .await?;

        set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: None,
                amount: 150000,
            },
        )
        .await?;

        let budgets = list_budgets(&conn, "local").await?;
        // 上書きにより1件のみ残ること
        assert_eq!(budgets.len(), 1);
        assert_eq!(budgets[0].amount, 150000);
        Ok(())
    }

    // 正常系: 複数の予算設定が全件返ること
    #[tokio::test]
    async fn list_budgets_returns_all_records() -> anyhow::Result<()> {
        let conn = setup_db().await?;
        set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: None,
                amount: 150000,
            },
        )
        .await?;
        set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: Some(Category::Food),
                amount: 40000,
            },
        )
        .await?;
        set_budget(
            &conn,
            &NewBudget {
                user_id: "local".to_string(),
                category: Some(Category::Transport),
                amount: 10000,
            },
        )
        .await?;

        let budgets = list_budgets(&conn, "local").await?;

        assert_eq!(budgets.len(), 3);
        Ok(())
    }
}
