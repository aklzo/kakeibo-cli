# 家計簿CLIアプリ 仕様書

## 1. 入力項目

| フィールド | 型 | 必須 | 備考 |
|-----------|-----|------|------|
| 名称 | 文字列 | ✓ | 例: "UberEats", "Python入門書" |
| 金額 | 整数（円） | ✓ | 収入カテゴリの場合も正の値で入力 |
| 日付 | 日付 | ✓ | デフォルト: コマンド実行日 |
| カテゴリ | enum | ✓ | 下記カテゴリ一覧参照 |
| メモ | 文字列 | - | 購入場所など補足情報 |

### カテゴリ一覧

| 識別子（CLI入力値） | 表示名 | 種別 |
|--------------------|--------|------|
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

---

## 2. コマンド仕様

### 2-1. 取引の追加

```bash
kakeibo add --name "UberEats" --amount 1500 --category food
kakeibo add --name "UberEats" --amount 1500 --category food --date 2025-04-30
kakeibo add --name "UberEats" --amount 1500 --category food --memo "夕食"

# 定型取引（2-9 参照）の内容で登録する
kakeibo add --preset commute
kakeibo add --preset commute --date 2026-11-14
kakeibo add --preset commute --amount 320 --memo "片道のみ"   # 指定した項目だけ上書き
```

| オプション | 短縮形 | 必須 | デフォルト |
|-----------|--------|------|-----------|
| `--name` | `-n` | ✓（`--preset` 指定時は省略可） | `--preset` 指定時は定型取引の値 |
| `--amount` | `-a` | ✓（`--preset` 指定時は省略可） | `--preset` 指定時は定型取引の値 |
| `--category` | `-c` | ✓（`--preset` 指定時は省略可） | `--preset` 指定時は定型取引の値 |
| `--date` | `-d` | - | 実行日 |
| `--memo` | `-m` | - | `--preset` 指定時は定型取引の値、それ以外はなし |
| `--preset` | `-p` | - | なし |

### 2-2. 取引の一覧表示

```bash
kakeibo list                     # 当月の全取引
kakeibo list --month 2025-04     # 指定月の全取引
kakeibo list --category food     # 当月をカテゴリ絞り込み
```

### 2-3. 取引の編集

```bash
kakeibo edit 3 --amount 2000        # ID=3の金額を上書き
kakeibo edit 3 --memo "新しいメモ"  # ID=3のメモを上書き
```

指定したIDの任意フィールドを上書きする。複数フィールドの同時指定も可。

### 2-4. 取引の削除

```bash
kakeibo delete 3    # ID=3の取引を削除
```

### 2-5. 月次集計

```bash
kakeibo summary                  # 当月
kakeibo summary --month 2025-04  # 指定月
```

**出力例：**
```
== 2025年04月 集計 ==

収入
  収入          200,000円

支出
  食費           32,000円
  固定費         60,000円
  交通費          8,000円
  ...
  ─────────────────────
  合計          120,000円

収支         +80,000円
```

### 2-6. カテゴリ別集計

```bash
kakeibo summary --by-category                # 当月
kakeibo summary --by-category --month 2025-04
```

### 2-7. 消費進捗率

```bash
kakeibo progress                             # 月全体 + カテゴリ別（予算対比、デフォルト）
kakeibo progress --total                     # 月全体のみ（予算対比）
kakeibo progress --by-category              # カテゴリ別のみ（予算対比）
kakeibo progress --last-month               # 月全体 + カテゴリ別（昨月実績対比）
kakeibo progress --last-month --total       # 月全体のみ（昨月実績対比）
kakeibo progress --last-month --by-category # カテゴリ別のみ（昨月実績対比）
```

**表示モードの仕様：**

| モード | 基準値 | 基準値が無効な場合の挙動 |
|--------|--------|--------------------------|
| デフォルト（予算対比） | `budget` テーブルの設定値 | 予算未設定時はエラーを表示して終了 |
| `--last-month`（昨月対比） | 昨月の実績支出額 | 昨月の支出データが 0（取引なし・収入のみ）の場合は N/A を表示する |

**予算対比 出力例：**
```
== 2025年04月 消費進捗率（予算対比）==

[ 月全体 ]
予算上限  150,000円
現在支出  120,000円
進捗率     80.0%  [████████░░]

[ カテゴリ別 ]
食費       32,000 / 40,000円   80.0%  [████████░░]
固定費     60,000 / 60,000円  100.0%  [██████████]
交通費      8,000 / 15,000円   53.3%  [█████░░░░░]
```

**昨月実績対比 出力例：**
```
== 2025年04月 消費進捗率（昨月実績対比）==

[ 月全体 ]
昨月実績  115,000円
現在支出   92,000円
進捗率     80.0%  [████████░░]

[ カテゴリ別 ]
食費       32,000 / 30,000円  106.7%  [██████████]
固定費     60,000 / 60,000円  100.0%  [██████████]
交通費      8,000 /  9,000円   88.9%  [████████░░]
```

**昨月支出データなし（取引なし・収入のみ）の場合の出力例：**
```
== 2025年04月 消費進捗率（昨月実績対比）==

[ 月全体 ]
昨月実績  （支出データなし）
現在支出       32,000円
進捗率    N/A  [----------]

[ カテゴリ別 ]
食費       32,000 / （データなし）  N/A  [----------]
```

### 2-8. 予算の設定

```bash
kakeibo budget set --total 150000             # 月全体の上限
kakeibo budget set --category food 40000      # カテゴリ別上限
kakeibo budget show                           # 現在の予算設定を表示
```

予算は月を指定しない場合、全月共通のデフォルト予算として登録される。

### 2-9. 定型取引（プリセット）

出社時の交通費のように、名称・金額・カテゴリが毎回同じ取引をひな形として登録しておく。
`kakeibo add --preset <キー>` で、ひな形の値をコピーして通常の取引として登録する。

```bash
kakeibo preset add commute --name "通勤（本社）" --amount 640 --category transport
kakeibo preset add commute-shinagawa --name "通勤（品川）" --amount 820 --category transport
kakeibo preset edit commute --amount 700        # 運賃改定・引越しなどで金額が変わった
kakeibo preset list
kakeibo preset delete commute-shinagawa
```

**`preset add` の引数：**

| 引数・オプション | 短縮形 | 必須 | 備考 |
|-----------------|--------|------|------|
| `<KEY>` | - | ✓ | 定型取引の識別子（`add --preset` で指定する値） |
| `--name` | `-n` | ✓ | 登録する取引の名称 |
| `--amount` | `-a` | ✓ | 登録する取引の金額 |
| `--category` | `-c` | ✓ | 登録する取引のカテゴリ |
| `--memo` | `-m` | - | 登録する取引のメモ |

**`preset edit` の引数：** `<KEY>` に加え、`--name` / `--amount` / `--category` / `--memo` のうち変更したい項目だけを指定する（複数同時指定可）。

**`preset list` の出力例：**
```
キー               名称           金額    カテゴリ
──────────────────────────────────────────────
commute            通勤（本社）    700円  交通費
commute-shinagawa  通勤（品川）    820円  交通費
```

**動作の仕様：**

- `add --preset` は定型取引の値を取引にコピーして登録する。登録後の取引と定型取引は紐付けない
- 定型取引を編集・削除しても、登録済みの取引は変わらない
- 定型取引が保持する金額は現在の値のみとする（適用開始日による金額の履歴は持たない）
- 汎用の仕組みとし、交通費以外の定型的な取引（ジムの会費など）にも使える

**通勤交通費での使い方：**

| 場面 | 操作 |
|------|------|
| 通常のオフィスに出社した | `kakeibo add --preset commute` |
| 運賃改定・引越しで金額が変わった | `kakeibo preset edit commute --amount 700`（以降の登録から新しい金額になる） |
| 別のオフィスに繰り返し行く | オフィスごとに定型取引を追加し `--preset commute-shinagawa` で登録 |
| 別のオフィスに一度だけ行く | 定型取引を作らず、通常の `add`（`--category transport`）で登録 |
| 片道だけ・途中下車などで金額が違う日 | `--preset commute --amount 320` で金額だけ上書き |
| 運賃改定前の日付をあとから登録する | `--preset commute --amount <旧運賃>` で金額を指定 |

キー名は、将来の Claude 連携（MCP）で Claude が定型取引を選ぶ手がかりにもなる。
通常のオフィスは `commute`、それ以外は `commute-<場所>` のように付けることを推奨する。

---

## 3. データ設計

### transactions テーブル

| カラム | 型 | 備考 |
|-------|-----|------|
| id | INTEGER PRIMARY KEY | 自動採番、削除後は欠番 |
| user_id | TEXT NOT NULL | Google アカウントの `sub` クレーム（CLI では `'local'`） |
| name | TEXT NOT NULL | 名称 |
| amount | INTEGER NOT NULL | 金額（円、正の整数） |
| date | TEXT NOT NULL | ISO 8601形式 YYYY-MM-DD |
| category | TEXT NOT NULL | カテゴリ識別子 |
| memo | TEXT | メモ（任意） |
| created_at | TEXT NOT NULL | 登録日時 |

### budgets テーブル

| カラム | 型 | 備考 |
|-------|-----|------|
| id | INTEGER PRIMARY KEY | 自動採番 |
| user_id | TEXT NOT NULL | Google アカウントの `sub` クレーム（CLI では `'local'`） |
| month | TEXT | YYYY-MM 形式。NULL の場合は全月共通のデフォルト予算 |
| category | TEXT | NULL の場合は月全体の予算 |
| amount | INTEGER NOT NULL | 上限金額（円） |

### presets テーブル

| カラム | 型 | 備考 |
|-------|-----|------|
| id | INTEGER PRIMARY KEY | 自動採番 |
| user_id | TEXT NOT NULL | Google アカウントの `sub` クレーム（CLI では `'local'`） |
| key | TEXT NOT NULL | 定型取引の識別子。`(user_id, key)` で一意（UNIQUE 制約） |
| name | TEXT NOT NULL | 登録する取引の名称 |
| amount | INTEGER NOT NULL | 登録する取引の金額（円、正の整数） |
| category | TEXT NOT NULL | 登録する取引のカテゴリ識別子 |
| memo | TEXT | 登録する取引のメモ（任意） |

---

## 4. 制約・境界条件

- 金額は正の整数のみ（小数点以下不可、0以下不可）
- 日付の入力フォーマットは `YYYY-MM-DD` のみ受け付ける
- IDは削除後に欠番になる（採番のやり直しはしない）
- list コマンドの件数上限は設けない（全件表示）
- `progress` コマンドは予算未設定時にエラーを表示して終了する
- `progress --last-month` は昨月の支出データが 0（取引なし・収入のみ）の場合、進捗率を N/A として表示する
- カテゴリは上記一覧の識別子のみ受け付ける（任意文字列は不可）
- 定型取引のキーは英小文字・数字・ハイフンのみ、1〜32 文字とする
- 同じキーの定型取引は 1 ユーザーにつき 1 件まで（既存のキーで `preset add` した場合はエラー）
- 存在しないキーを `add --preset` / `preset edit` / `preset delete` に指定した場合はエラー
- 定型取引の金額にも、取引と同じ制約（正の整数のみ）を適用する
- `add --preset` で上書きした値にも、通常の `add` と同じ制約を適用する

---

## 5. フェーズ管理

### フェーズ1（初期実装）
- [x] 取引のCRUD（add / list / edit / delete）
- [x] 月次集計（summary）
- [x] カテゴリ別集計（summary --by-category）
- [x] 消費進捗率（progress）
- [x] 予算設定（budget）

### フェーズ4（定型取引）
- [ ] presets テーブル・リポジトリ層
- [ ] 定型取引の管理（preset add / edit / list / delete）
- [ ] 定型取引からの登録（add --preset）

### 将来課題（定型取引）
- [ ] 定型取引の REST API と Android の設定画面
- [ ] 適用開始日による金額の履歴（運賃改定前の日付を後から登録する頻度が高い場合）

### 将来課題（CLI）
- [ ] グラフ描画（`ratatui` による棒グラフ）
- [ ] 期間指定集計
- [ ] ローカルLLMによる支出分析
- [ ] データエクスポート（CSV）
