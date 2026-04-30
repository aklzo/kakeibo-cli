use std::path::PathBuf;

use anyhow::Context;
use rusqlite::Connection;

/// アプリデータディレクトリ（~/.kakeibo-cli/）を返す。存在しない場合は作成する。
fn data_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME環境変数が設定されていません")?;
    let dir = PathBuf::from(home).join(".kakeibo-cli");
    std::fs::create_dir_all(&dir).context("データディレクトリの作成に失敗しました")?;
    Ok(dir)
}

/// SQLite に接続してマイグレーションを実行し、接続を返す。
pub fn open() -> anyhow::Result<Connection> {
    let path = data_dir()?.join("kakeibo.db");
    let conn = Connection::open(&path).context("データベースへの接続に失敗しました")?;
    migrate(&conn)?;
    Ok(conn)
}

/// transactions・budgets テーブルを作成する（既存の場合はスキップ）。
fn migrate(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS transactions (
            id         INTEGER PRIMARY KEY,
            name       TEXT    NOT NULL,
            amount     INTEGER NOT NULL,
            date       TEXT    NOT NULL,
            category   TEXT    NOT NULL,
            memo       TEXT,
            created_at TEXT    NOT NULL
        );
        CREATE TABLE IF NOT EXISTS budgets (
            id       INTEGER PRIMARY KEY,
            month    TEXT,
            category TEXT,
            amount   INTEGER NOT NULL
        );
        ",
    )
    .context("マイグレーションに失敗しました")
}
