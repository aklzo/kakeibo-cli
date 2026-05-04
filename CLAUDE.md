# 家計簿CLIアプリ 実装ガイドライン

## プロジェクト概要
RustによるCLI家計簿アプリ。学習目的のため「基本に忠実・Rustらしい設計」を最優先とする。

## 技術スタック
- **DB**: rusqlite（SQLite、`bundled` feature使用）
- **CLIパーサー**: clap v4（`derive` feature使用）
- **シリアライズ**: serde + serde_json
- **エラーハンドリング**: anyhow

## 設計上の制約

### 所有権・借用
- `clone()` による回避を禁止する。借用で解決できる場合は必ず借用を使うこと
- ライフタイム注釈が必要な場合は省略せず明示すること

### 型設計
- プリミティブ型（`i64`, `String`）の裸使用を避け、ドメインに即した型を定義すること
  - 悪い例: `fn add(amount: i64, memo: String)`
  - 良い例: `fn add(transaction: Transaction)`
- `unwrap()` / `expect()` の使用を禁止する。必ず `?` 演算子か `match` で処理すること
- `pub` は必要最小限にとどめること（モジュール外から使うものだけ公開）

### エラーハンドリング
- `anyhow::Result` を戻り値とし `?` で伝播させること
- エラーメッセージは日本語でユーザーが理解できる文言にすること
  - 例: `.context("金額の解析に失敗しました")`

### モジュール構成
以下の責務分離を守ること。1ファイルに全処理を書かない。

```
src/
  main.rs        # エントリーポイント、CLIの起動のみ
  cli.rs         # clapの定義（引数・サブコマンド）
  db.rs          # SQLite接続・マイグレーション
  model.rs       # ドメイン型の定義（Transaction等）
  repository.rs  # DBへのCRUD操作
```

### コーディングスタイル
- `cargo fmt` の結果に従うこと
- `cargo clippy` の警告を全て解消すること
- 関数・構造体・publicな要素には doc comment（`///`）を付けること
- マジックナンバーは定数（`const`）として定義すること

## やってはいけないこと
- `unsafe` ブロックの使用
- `std::process::exit()` による強制終了（`main` で `anyhow::Result` を返すこと）
- グローバル変数・`static mut` の使用

## 仕様書
- 格納場所: `docs/spec.md`
- 実装前に必ず参照すること
- 仕様に不明点がある場合は実装を進めず確認すること

## テスト方針
- テストは同一ファイル末尾の `#[cfg(test)] mod tests` に記載すること
- テスト関数名は `snake_case` で「何をテストするか」を表現すること
  - 例: `add_transaction_with_empty_name_returns_error`
- 正常系・異常系それぞれにコメントで補足を記載すること
- テスト用DBは `Connection::open_in_memory()` を使用すること

## フェーズ2 参照ドキュメント
- API 仕様: `docs/api-spec.md`
- 技術選定背景: `docs/adr.md`

## フェーズ2 追加制約
- CLI と API でビジネスロジックを重複実装しないこと
- API のエラーメッセージは英語で統一すること
- 全エンドポイントで Google ID Token 認証を必須とすること

