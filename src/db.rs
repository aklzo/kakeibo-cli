use std::path::PathBuf;

use anyhow::Context;
use libsql::Connection;

/// アプリデータディレクトリ（~/.kakeibo-cli/）を返す。存在しない場合は作成する。
fn data_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME環境変数が設定されていません")?;
    let dir = PathBuf::from(home).join(".kakeibo-cli");
    std::fs::create_dir_all(&dir).context("データディレクトリの作成に失敗しました")?;
    Ok(dir)
}

/// SQLite に接続してマイグレーションを実行し、接続を返す。
pub async fn open() -> anyhow::Result<Connection> {
    let path = data_dir()?.join("kakeibo.db");
    let db = libsql::Builder::new_local(&path)
        .build()
        .await
        .context("データベースへの接続に失敗しました")?;
    let conn = db.connect().context("データベース接続の取得に失敗しました")?;
    migrate(&conn).await?;
    Ok(conn)
}

/// transactions・budgets テーブルを作成し、既存テーブルへの user_id カラム追加も行う。
pub(crate) async fn migrate(conn: &Connection) -> anyhow::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS transactions (
            id         INTEGER PRIMARY KEY,
            user_id    TEXT    NOT NULL,
            name       TEXT    NOT NULL,
            amount     INTEGER NOT NULL,
            date       TEXT    NOT NULL,
            category   TEXT    NOT NULL,
            memo       TEXT,
            created_at TEXT    NOT NULL
        )",
        (),
    )
    .await
    .context("transactionsテーブルの作成に失敗しました")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS budgets (
            id       INTEGER PRIMARY KEY,
            user_id  TEXT    NOT NULL,
            month    TEXT,
            category TEXT,
            amount   INTEGER NOT NULL
        )",
        (),
    )
    .await
    .context("budgetsテーブルの作成に失敗しました")?;

    // user_id カラムが存在しない既存 DB への後付けマイグレーション。
    // 既にカラムが存在する場合はエラーになるが無視する。
    let _ = conn
        .execute(
            "ALTER TABLE transactions ADD COLUMN user_id TEXT NOT NULL DEFAULT 'local'",
            (),
        )
        .await;
    let _ = conn
        .execute(
            "ALTER TABLE budgets ADD COLUMN user_id TEXT NOT NULL DEFAULT 'local'",
            (),
        )
        .await;

    Ok(())
}
