# kakeibo-cli

個人用家計簿アプリ。Rust 製の CLI と REST API で構成されます。

- **CLI** (`kakeibo`): ローカル SQLite にデータを保存
- **API** (`kakeibo-api`): Google Cloud Run 上で動作、Turso（SQLite 互換クラウド DB）にデータを保存。Google ID Token による認証が必要

## インストール

```bash
cargo install --path .
```

## 使い方

### 取引の追加

```bash
kakeibo add --name "UberEats" --amount 1500 --category food
kakeibo add -n "UberEats" -a 1500 -c food --date 2025-04-30 --memo "夕食"
```

| オプション | 短縮形 | 必須 | デフォルト |
|-----------|--------|------|-----------|
| `--name` | `-n` | ✓ | - |
| `--amount` | `-a` | ✓ | - |
| `--category` | `-c` | ✓ | - |
| `--date` | `-d` | - | 実行日 |
| `--memo` | `-m` | - | なし |

### 取引の一覧表示

```bash
kakeibo list                      # 当月の全取引
kakeibo list --month 2025-04      # 指定月の全取引
kakeibo list --category food      # 当月をカテゴリ絞り込み
```

### 取引の編集

```bash
kakeibo edit 3 --amount 2000
kakeibo edit 3 --name "新しい名前" --memo "新しいメモ"
```

指定した ID の任意フィールドを上書きします。複数フィールドの同時指定も可能です。

### 取引の削除

```bash
kakeibo delete 3
```

### 月次集計

```bash
kakeibo summary                   # 当月
kakeibo summary --month 2025-04   # 指定月
kakeibo summary --by-category     # カテゴリ別
```

```
== 2025年04月 集計 ==

収入
  収入          200,000円

支出
  食費           32,000円
  固定費         60,000円
  ─────────────────────
  合計          120,000円

収支         +80,000円
```

### 予算の設定

```bash
kakeibo budget set --total 150000            # 月全体の上限を設定
kakeibo budget set --category food 40000     # カテゴリ別の上限を設定
kakeibo budget show                          # 現在の予算設定を表示
```

### 消費進捗率

```bash
kakeibo progress                             # 月全体 + カテゴリ別（予算対比）
kakeibo progress --total                     # 月全体のみ
kakeibo progress --by-category              # カテゴリ別のみ
kakeibo progress --last-month               # 昨月実績対比
kakeibo progress --last-month --total       # 昨月実績対比・月全体のみ
kakeibo progress --last-month --by-category # 昨月実績対比・カテゴリ別のみ
```

```
== 2025年04月 消費進捗率（予算対比）==

[ 月全体 ]
予算上限    150,000円
現在支出    120,000円
進捗率       80.0%  [████████░░]

[ カテゴリ別 ]
食費       32,000 / 40,000円   80.0%  [████████░░]
固定費     60,000 / 60,000円  100.0%  [██████████]
交通費      8,000 / 15,000円   53.3%  [█████░░░░░]
```

## カテゴリ一覧

| 識別子 | 表示名 | 種別 |
|--------|--------|------|
| `fixed` | 固定費 | 支出 |
| `subscription` | サブスク | 支出 |
| `food` | 食費 | 支出 |
| `daily` | 日用品 | 支出 |
| `transport` | 交通費 | 支出 |
| `clothing` | 被服費 | 支出 |
| `medical` | 医療費 | 支出 |
| `beauty` | 美容費 | 支出 |
| `social` | 交際費 | 支出 |
| `special` | 特別日 | 支出 |
| `learning` | 学習 | 支出 |
| `hobby` | 趣味 | 支出 |
| `interior` | インテリア費 | 支出 |
| `income` | 収入 | 収入 |

## データ保存先

| 実行環境 | 保存先 |
|----------|--------|
| CLI（ローカル） | `~/.kakeibo-cli/kakeibo.db`（SQLite） |
| API（Cloud Run） | Turso（`DATABASE_URL` 環境変数で接続先を指定） |

## API

Google Cloud Run 上で稼働する REST API。すべてのリクエストに `Authorization: Bearer <Google ID Token>` ヘッダーが必要。

エンドポイント詳細は [`docs/api-spec.md`](docs/api-spec.md) を参照。

## 技術スタック

- **言語**: Rust（edition 2024）
- **DB**: libsql（ローカルは SQLite、Cloud Run は Turso に接続）
- **CLI パーサー**: clap v4（derive feature）
- **HTTP フレームワーク**: axum（API サーバー）
- **非同期ランタイム**: tokio
- **エラーハンドリング**: anyhow

## 開発

```bash
# ビルド・テスト・静的解析
cargo build
cargo test
cargo clippy

# CLI の動作確認（default-run により --bin 不要）
cargo run -- list
cargo run -- add --name "テスト" --amount 1000 --category food

# API サーバーの起動（認証スキップ・ローカル SQLite 使用）
SKIP_AUTH=true cargo run --bin kakeibo-api

# API サーバーの起動（Turso 接続・認証あり）
DATABASE_URL=<turso_url> AUTH_TOKEN=<token> GOOGLE_CLIENT_ID=<client_id> cargo run --bin kakeibo-api
```
