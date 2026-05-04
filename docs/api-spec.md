# 家計簿API 仕様書

## 1. 概要

| 項目 | 内容 |
|------|------|
| 実行環境 | Google Cloud Run（asia-northeast1） |
| 認証方式 | Google ID Token（Bearer Token） |
| レスポンス形式 | JSON |
| 文字コード | UTF-8 |
| APIバージョン | v1 |
| ベースURL | `https://<service>.run.app/api/v1` |

---

## 2. バイナリ構成

```toml
# Cargo.toml
[[bin]]
name = "kakeibo"        # CLI（既存）
path = "src/main.rs"

[[bin]]
name = "kakeibo-api"    # API サーバー
path = "src/bin/api.rs"
```

Cloud Run には `kakeibo-api` のみデプロイする。
`repository.rs` / `model.rs` / `db.rs` は CLI・API 共有。

---

## 3. 認証

すべてのエンドポイントで Google ID Token による認証を必須とする。

```
Authorization: Bearer <Google ID Token>
```

### user_id の算出

Google ID Token の `sub` クレームをそのまま `user_id` として使用する。

```
sub: "1234567890"  →  user_id: "1234567890"
```

### 認証エラー

| ケース | HTTP ステータス |
|--------|----------------|
| Authorization ヘッダーなし | 401 |
| Token 無効・期限切れ | 401 |
| 対象リソースが他ユーザーのもの | 403 |

---

## 4. DB スキーマ変更

### transactions テーブル（変更）

```sql
CREATE TABLE transactions (
    id         INTEGER PRIMARY KEY,
    user_id    TEXT NOT NULL,        -- 追加
    name       TEXT NOT NULL,
    amount     INTEGER NOT NULL,
    date       TEXT NOT NULL,
    category   TEXT NOT NULL,
    memo       TEXT,
    created_at TEXT NOT NULL
);
```

### budgets テーブル（変更）

```sql
CREATE TABLE budgets (
    id       INTEGER PRIMARY KEY,
    user_id  TEXT NOT NULL,          -- 追加
    month    TEXT,
    category TEXT,
    amount   INTEGER NOT NULL
);
```

---

## 5. 共通レスポンス形式

### 成功時

```json
{
  "data": { }
}
```

### エラー時

```json
{
  "error": "Transaction not found"
}
```

### 主なエラーメッセージ一覧

| ケース | メッセージ |
|--------|-----------|
| リソースが存在しない | `"Transaction not found"` |
| バリデーションエラー | `"Invalid request: <field> is required"` |
| 認証エラー | `"Unauthorized"` |
| 権限エラー | `"Forbidden"` |
| サーバーエラー | `"Internal server error"` |

---

## 6. エンドポイント一覧

### 6-1. 取引の追加

```
POST /api/v1/transactions
```

**リクエスト**

```json
{
  "name": "UberEats",
  "amount": 1500,
  "date": "2025-05-01",
  "category": "food",
  "memo": "夕食"
}
```

| フィールド | 必須 | 備考 |
|-----------|------|------|
| name | ✓ | |
| amount | ✓ | 正の整数のみ |
| date | ✓ | YYYY-MM-DD形式 |
| category | ✓ | カテゴリ識別子（spec.md 参照） |
| memo | - | |

**レスポンス** `201 Created`

```json
{
  "data": {
    "id": 1,
    "name": "UberEats",
    "amount": 1500,
    "date": "2025-05-01",
    "category": "food",
    "memo": "夕食",
    "created_at": "2025-05-01T12:00:00Z"
  }
}
```

---

### 6-2. 取引の一覧取得

```
GET /api/v1/transactions
```

**クエリパラメータ**

| パラメータ | 必須 | デフォルト | 備考 |
|-----------|------|-----------|------|
| month | - | 当月 | YYYY-MM形式 |
| category | - | なし | カテゴリ絞り込み |
| limit | - | 50 | 最大取得件数 |
| offset | - | 0 | 取得開始位置 |

**レスポンス** `200 OK`

```json
{
  "data": {
    "transactions": [
      {
        "id": 1,
        "name": "UberEats",
        "amount": 1500,
        "date": "2025-05-01",
        "category": "food",
        "memo": "夕食",
        "created_at": "2025-05-01T12:00:00Z"
      }
    ],
    "total": 1
  }
}
```

---

### 6-3. 取引の編集

```
PATCH /api/v1/transactions/:id
```

**リクエスト**（変更したいフィールドのみ送信）

```json
{
  "amount": 2000,
  "memo": "新しいメモ"
}
```

**レスポンス** `200 OK`

```json
{
  "data": {
    "id": 1,
    "name": "UberEats",
    "amount": 2000,
    "date": "2025-05-01",
    "category": "food",
    "memo": "新しいメモ",
    "created_at": "2025-05-01T12:00:00Z"
  }
}
```

---

### 6-4. 取引の削除

```
DELETE /api/v1/transactions/:id
```

**レスポンス** `204 No Content`

---

### 6-5. 月次集計

```
GET /api/v1/summary
```

**クエリパラメータ**

| パラメータ | 必須 | デフォルト |
|-----------|------|-----------|
| month | - | 当月 |
| by_category | - | false |

**レスポンス** `200 OK`

```json
{
  "data": {
    "month": "2025-05",
    "income": 200000,
    "expense": 120000,
    "balance": 80000,
    "by_category": [
      { "category": "food", "amount": 32000 },
      { "category": "fixed", "amount": 60000 }
    ]
  }
}
```

---

### 6-6. 予算の設定

```
POST /api/v1/budgets
```

**リクエスト**

```json
{
  "month": null,
  "category": null,
  "amount": 150000
}
```

| フィールド | 必須 | 備考 |
|-----------|------|------|
| month | - | null の場合は全月共通 |
| category | - | null の場合は月全体の予算 |
| amount | ✓ | 正の整数 |

**レスポンス** `201 Created`

```json
{
  "data": {
    "id": 1,
    "month": null,
    "category": null,
    "amount": 150000
  }
}
```

---

### 6-7. 予算の取得

```
GET /api/v1/budgets
```

**レスポンス** `200 OK`

```json
{
  "data": {
    "budgets": [
      { "id": 1, "month": null, "category": null, "amount": 150000 },
      { "id": 2, "month": null, "category": "food", "amount": 40000 }
    ]
  }
}
```

---

### 6-8. 消費進捗率

```
GET /api/v1/progress
```

**クエリパラメータ**

| パラメータ | 必須 | デフォルト | 備考 |
|-----------|------|-----------|------|
| month | - | 当月 | |
| mode | - | `budget` | `budget` または `last_month` |
| scope | - | `both` | `total` / `by_category` / `both` |

**レスポンス** `200 OK`

```json
{
  "data": {
    "month": "2025-05",
    "mode": "budget",
    "total": {
      "base": 150000,
      "current": 120000,
      "percentage": 80.0
    },
    "by_category": [
      {
        "category": "food",
        "base": 40000,
        "current": 32000,
        "percentage": 80.0
      }
    ]
  }
}
```

---

## 7. 技術スタック

| 用途 | 選択 |
|------|------|
| HTTP フレームワーク | `axum` |
| 非同期ランタイム | `tokio` |
| ミドルウェア | `tower-http` |
| DB | `Turso`（libsql クレート） |
| 認証 | Google ID Token 検証 |

---

## 8. フェーズ管理

### フェーズ2（API 実装）
- [ ] axum による API サーバー実装
- [ ] Google ID Token 認証ミドルウェア
- [ ] Turso への DB 移行
- [ ] Cloud Run へのデプロイ
- [ ] GitHub Actions による自動デプロイ設定

### フェーズ3（Android アプリ）
- [ ] Kotlin + Jetpack Compose によるアプリ実装
- [ ] Google Sign-In 実装
- [ ] 各 API エンドポイントとの接続

### 将来課題
- [ ] ページネーション（limit/offset は実装済みのため UI 対応のみ）
- [ ] オフライン対応（Android ローカルキャッシュ）
- [ ] ratatui によるグラフ描画（CLI）
- [ ] ローカル LLM による支出分析
