# データ移行手順: ローカル SQLite → Turso

## 概要

フェーズ1で `~/.kakeibo-cli/kakeibo.db` に蓄積したデータを Turso へ移行する手順。
Cloud Run 上の API から参照するためには Turso への移行が必要。

## 前提条件

- Turso CLI インストール済み・ログイン済み（`turso auth login`）
- `turso db create kakeibo` でDB作成済み
- `sqlite3` コマンドが使用可能（macOS はプリインストール済み）
- Google アカウントの `sub` 値を確認済み

## sub 値の確認方法

`sub` は Google アカウントを一意に識別する値。セッションをまたいでも変わらない。

```bash
# gcloud で ID Token を取得し tokeninfo エンドポイントで sub を確認
curl -s "https://oauth2.googleapis.com/tokeninfo?id_token=$(gcloud auth print-identity-token)" \
  | python3 -m json.tool | grep '"sub"'
```

> **注意**: `sub` は公開しないこと。LLM・チャットツールなどに貼り付けない。

---

## 移行手順

### 1. user_id を環境変数に設定

```bash
export MIGRATION_USER_ID="<確認した sub 値>"
```

### 2. ローカル SQLite からエクスポート

```bash
sqlite3 ~/.kakeibo-cli/kakeibo.db .dump > /tmp/kakeibo_backup.sql
```

### 3. user_id を置換して移行用 SQL を作成

フェーズ1のデータは `user_id = 'local'` で保存されている。
Turso では Google アカウントの `sub` 値が `user_id` となるため置換する。

```bash
sed "s/'local'/'${MIGRATION_USER_ID}'/g" /tmp/kakeibo_backup.sql > /tmp/kakeibo_migration.sql
```

### 4. 内容確認

件数が一致していることを確認する。

```bash
# 移行用 SQL の INSERT 件数
grep -c "INSERT" /tmp/kakeibo_migration.sql

# ローカル SQLite の件数
sqlite3 ~/.kakeibo-cli/kakeibo.db "SELECT COUNT(*) FROM transactions; SELECT COUNT(*) FROM budgets;"
```

user_id が正しく置換されていることをサンプルで確認:

```bash
grep "INSERT INTO transactions" /tmp/kakeibo_migration.sql | head -3
```

### 5. Turso へインポート

```bash
turso db shell kakeibo < /tmp/kakeibo_migration.sql
```

### 6. 移行後の件数確認

```bash
turso db shell kakeibo "SELECT COUNT(*) FROM transactions;"
turso db shell kakeibo "SELECT COUNT(*) FROM budgets;"
```

ローカル SQLite の件数と一致していれば移行完了。

### 7. 作業ファイルの削除

`sub` 値が含まれるファイルを削除する。

```bash
rm /tmp/kakeibo_backup.sql /tmp/kakeibo_migration.sql
```

---

## 備考

- `CREATE TABLE IF NOT EXISTS` や `ALTER TABLE` は冪等なため、再実行しても問題ない
- インポート時に `table already exists` などのエラーが出ても、`INSERT` が成功していれば移行は完了している
- 移行後もローカル SQLite は削除しない（CLI は引き続きローカルで動作する）
