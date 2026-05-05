# 認証付き API 手動テスト手順

## 概要

Google OAuth Playground を使用して、Cloud Run 上の `kakeibo-api` に対して
Google ID Token 認証付きのリクエストを送信しテストする手順書。

フェーズ3の Android 実装に先立ち、API の認証・認可が正しく動作していることを確認する。

---

## OAuth 2.0 と OpenID Connect の仕組み

### OAuth 2.0 とは

「認可（Authorization）」のプロトコル。
「このアプリがあなたの代わりにリソースへアクセスしてよいか」を委譲する仕組みであり、
**誰であるか（認証）** を証明するものではない。

### OpenID Connect（OIDC）とは

OAuth 2.0 の上に「認証（Authentication）」を追加した拡張仕様。
通常の OAuth 2.0 レスポンスに **ID Token** が加わる。

```
OAuth 2.0 レスポンス:  access_token のみ
OIDC レスポンス:       access_token + id_token（JWT）
```

`kakeibo-api` が使用するのは **id_token** であり、access_token は不要。

### ID Token（JWT）の構造

ID Token は `.` で区切られた 3 パーツの文字列（JWT）。

```
<ヘッダー>.<ペイロード>.<署名>
```

ペイロードに含まれる主要クレーム：

| クレーム | 説明 | kakeibo-api での用途 |
|---------|------|---------------------|
| `iss` | 発行者（Issuer）。`accounts.google.com` | 正規の Google 発行トークンか検証 |
| `aud` | 宛先クライアント（Audience）。OAuth クライアント ID | 自分の API 向けトークンか検証 |
| `sub` | ユーザー識別子（Subject）。Google アカウント固有の値 | `user_id` として DB クエリに使用 |
| `exp` | 有効期限（Unix 時刻）。発行から 1 時間 | トークン期限切れ検証 |

### Authorization Code Flow（認可コードフロー）

```
  クライアント         Google 認可サーバー        kakeibo-api
      │                      │                      │
      │─── ① 認可リクエスト ──▶│                      │
      │                      │ ログイン・同意画面      │
      │◀── ② 認可コード ───────│                      │
      │                      │                      │
      │─── ③ コード交換 ───────▶│                      │
      │◀── ④ id_token 取得 ───│                      │
      │                      │                      │
      │─── ⑤ Bearer id_token ─────────────────────▶│
      │                      │        ⑥ 署名・aud 検証│
      │◀────────────────────────────── ⑦ レスポンス ─│
```

### サーバー側（auth.rs）の検証内容

`kakeibo-api` はリクエストを受け取ると以下を順に実行する：

1. `Authorization: Bearer <token>` ヘッダーからトークンを抽出
2. JWT ヘッダーの `kid`（鍵 ID）を取得
3. Google の JWKS エンドポイント（`/oauth2/v3/certs`）から公開鍵一覧を取得
4. `kid` が一致する公開鍵で RS256 署名を検証
5. `iss` が `accounts.google.com` または `https://accounts.google.com` であることを確認
6. `aud` が環境変数 `GOOGLE_CLIENT_ID` と一致することを確認
7. `sub` を抽出し `user_id` として以降の DB クエリに使用

---

## OAuth Playground の位置付け

### Playground が担う役割

OAuth Playground は**テスト用の OAuth クライアント**として機能する。
フェーズ3では Android アプリが `requestIdToken(webClientId)` を呼び出して id_token を取得するが、
Playground はそのフローをブラウザ上で手動実行できる開発者ツール。

```
フェーズ3（本番）: Android アプリ → Google Sign-In → id_token
テスト（現在）:   OAuth Playground  → Authorization Code Flow → id_token
```

### 「自分の OAuth 認証情報を使用する」設定が重要な理由

**`aud` クレームはトークン発行時に使用したクライアント ID で決まる。**

Playground のデフォルト設定では Playground 自身のクライアント ID が `aud` になるため、
`kakeibo-api` の検証（`aud == GOOGLE_CLIENT_ID`）で弾かれる。

`kakeibo-api` のクライアント ID・シークレットを Playground に設定することで、
`aud = GOOGLE_CLIENT_ID` の id_token が発行され、API の検証を通過できる。

### なぜこのテストが成立するか

| 確認ポイント | 根拠 |
|------------|------|
| `iss` が正規 Google 発行である | Playground 経由でも Google 認可サーバーが発行するため同一 |
| `aud` が `GOOGLE_CLIENT_ID` と一致する | Playground に kakeibo-api のクライアント ID を設定するため |
| `sub` が Turso の `user_id` と一致する | データ移行時に同一 Google アカウントの `sub` を `user_id` として登録済みのため |
| 署名が有効である | Google が RS256 で署名し、API が JWKS で検証するため |

Android アプリが行う認証フローと **発行される id_token の性質は同一**であり、
このテストで通過した認証ロジックはフェーズ3でもそのまま機能する。

---

## 前提条件

- `kakeibo-api` が Cloud Run にデプロイ済みであること
- Turso にデータ移行済みであること（`SELECT COUNT(*) FROM transactions` で件数確認済み）
- GCP コンソールへのアクセス権限があること
- `kakeibo-api` の OAuth クライアントのシークレットを確認できること

---

## 手順

### 1. OAuth クライアントにリダイレクト URI を追加

GCP コンソール → **「API とサービス」→「認証情報」** を開く。

`kakeibo-api` の OAuth 2.0 クライアント ID を編集し、
「承認済みのリダイレクト URI」に以下を追加して保存する。

```
https://developers.google.com/oauthplayground
```

> **注意**: 変更が反映されるまで数分かかる場合がある。

### 2. クライアント ID・シークレットを控える

同じ認証情報ページで以下を確認しておく。

- クライアント ID（`GOOGLE_CLIENT_ID` に設定済みの値）
- クライアント シークレット

シークレットは第三者に共有しないこと。

### 3. OAuth Playground を開く

ブラウザで以下を開く。

```
https://developers.google.com/oauthplayground
```

### 4. 自分のクライアント認証情報を設定

画面右上の ⚙️ アイコン（Settings）をクリックし、以下を設定する。

- **「Use your own OAuth credentials」** にチェックを入れる
- **OAuth Client ID**: `kakeibo-api` のクライアント ID を入力
- **OAuth Client secret**: クライアント シークレットを入力
- 閉じる

### 5. スコープを選択して認可

左ペインの **Step 1** で以下を実行する。

1. スコープ一覧から **「Google OAuth2 API v2」** を展開
2. `https://www.googleapis.com/auth/userinfo.email` を選択
3. 「**Authorize APIs**」ボタンをクリック
4. Google ログイン画面が表示されるので、Turso にデータ移行した Google アカウントでログイン
5. 権限の確認画面で「続行」

### 6. 認可コードを id_token に交換

**Step 2** で「**Exchange authorization code for tokens**」をクリックする。

レスポンスに以下が含まれる。

```json
{
  "access_token": "...",
  "id_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6...",
  "expires_in": 3599,
  ...
}
```

**`id_token` の値**をコピーする（`access_token` は使用しない）。

> id_token は発行から **1 時間**で期限切れとなる。期限切れ後は Step 2 で再取得する。

### 7. 環境変数に設定

ターミナルでトークンを環境変数に設定しておくとコマンドがシンプルになる。

```bash
export API_TOKEN="<コピーした id_token>"
export API_BASE="<Cloud Run の URL>/api/v1"
```

---

## テスト手順

### テスト 1: 認証なしで 401 が返ること

```bash
curl -s -o /dev/null -w "%{http_code}" "$API_BASE/transactions"
```

**期待値**: `401`

### テスト 2: 認証ありで取引一覧が返ること

```bash
curl -s \
  -H "Authorization: Bearer $API_TOKEN" \
  "$API_BASE/transactions" | python3 -m json.tool
```

**期待値**: `200` + 移行済みトランザクション一覧が `data.transactions` に含まれる

### テスト 3: 取引を追加できること

```bash
curl -s -X POST \
  -H "Authorization: Bearer $API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"テスト支出","amount":500,"date":"2026-05-05","category":"food"}' \
  "$API_BASE/transactions" | python3 -m json.tool
```

**期待値**: `201` + 追加されたレコードが `data` に返る。返った `id` を控えておく。

### テスト 4: 追加した取引が一覧に反映されていること

```bash
curl -s \
  -H "Authorization: Bearer $API_TOKEN" \
  "$API_BASE/transactions?month=2026-05" | python3 -m json.tool
```

**期待値**: テスト 3 で追加したレコードが含まれる

### テスト 5: 月次集計が返ること

```bash
curl -s \
  -H "Authorization: Bearer $API_TOKEN" \
  "$API_BASE/summary?month=2026-05" | python3 -m json.tool
```

**期待値**: `200` + `data.expense` にテスト 3 の金額（500）が反映されている

### テスト 6: テストデータを削除して後片付け

テスト 3 で返った `id` を使用する。

```bash
curl -s -o /dev/null -w "%{http_code}" -X DELETE \
  -H "Authorization: Bearer $API_TOKEN" \
  "$API_BASE/transactions/<id>"
```

**期待値**: `204`

### テスト 7: 他ユーザーのリソースへのアクセスが 403 になること（任意）

存在するが自分のものではない ID（例: 他ユーザーが作成した ID）を指定して 403 になることを確認する。
本番環境で他ユーザーのデータが存在しない場合は省略可。

---

## 注意事項

- `id_token` はパスワードと同等の機密情報。ターミナル履歴・チャットツール・公開リポジトリに貼り付けないこと
- テスト完了後、Playground の「Revoke token」でトークンを無効化することを推奨
- GCP コンソールで追加したリダイレクト URI（`oauthplayground` の URL）はテスト後も残して問題ない
