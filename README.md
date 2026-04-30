# kakeibo-cli

個人用 CLI 家計簿アプリ。Rust 製、データは SQLite に保存されます。

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

`~/.kakeibo-cli/kakeibo.db`（SQLite）

## 技術スタック

- **言語**: Rust（edition 2024）
- **DB**: SQLite（rusqlite、bundled feature）
- **CLI パーサー**: clap v4（derive feature）
- **エラーハンドリング**: anyhow

## 開発

```bash
cargo build
cargo test
cargo clippy
```
