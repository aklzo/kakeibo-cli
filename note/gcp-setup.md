# GCP セットアップ手順

## 概要

kakeibo-api を Cloud Run にデプロイするための GCP 環境構築手順。
再現性確保のため可能な限り CLI（gcloud）で実施する。

## 前提条件

- Google アカウント
- クレジットカード（無料枠内でも登録が必要）
- Homebrew インストール済み（macOS）

---

## ステップ① GCP アカウント・プロジェクト作成

**ブラウザで実施（CLI 不可）**

```
1. https://console.cloud.google.com にアクセス
2. Google アカウントでログイン
3.「プロジェクトを作成」
   - プロジェクト名: kakeibo
   - プロジェクト ID: kakeibo-<任意の文字列>（世界で一意である必要がある）
4. 請求先アカウントの設定（カード登録）
```

---

## ステップ② gcloud CLI のインストール

```bash
brew install google-cloud-sdk

# インストール確認
gcloud --version
```

---

## ステップ③ gcloud の初期設定

```bash
# ログイン
gcloud auth login

# プロジェクトの設定
gcloud config set project kakeibo-<your-id>

# 設定確認
gcloud config list
```

---

## ステップ④ 必要な API の有効化

```bash
gcloud services enable run.googleapis.com
gcloud services enable artifactregistry.googleapis.com
gcloud services enable cloudbuild.googleapis.com

# 有効化確認
gcloud services list --enabled
```

| API | 用途 |
|-----|------|
| `run.googleapis.com` | Cloud Run |
| `artifactregistry.googleapis.com` | Docker イメージの保存 |
| `cloudbuild.googleapis.com` | GitHub Actions からのデプロイ |

---

## ステップ⑤ Artifact Registry の作成

Docker イメージの保存先。

```bash
gcloud artifacts repositories create kakeibo \
  --repository-format=docker \
  --location=asia-northeast1 \
  --description="kakeibo API server"

# 確認
gcloud artifacts repositories list
```

---

## ステップ⑥ OAuth 2.0 クライアント ID の作成

**ブラウザで実施（gcloud CLI では作成不可）**

Google ID Token 認証に必要なクライアント ID を 2 種類作成する。

```
Google Cloud Console
→ API とサービス → 認証情報 → 認証情報を作成 → OAuth クライアント ID
```

### 1. Web アプリケーション型（バックエンド用）

| 項目 | 値 |
|------|----|
| アプリケーションの種類 | Web アプリケーション |
| 名前 | `kakeibo-api` |
| 承認済みの JavaScript 生成元 | 空欄でよい |
| 承認済みのリダイレクト URI | 空欄でよい |

作成後に表示される **クライアント ID**（`xxxxxx.apps.googleusercontent.com`）を控える。
→ Cloud Run の `GOOGLE_CLIENT_ID` 環境変数として使用する。

> **JavaScript 生成元・リダイレクト URI について**
> これらはブラウザベースの OAuth フロー専用の設定。
> Android アプリから Google Sign-In SDK 経由でトークンを取得する今回の構成では不要。
> 将来 Web フロントエンドを追加する場合に設定すればよい。

### 2. Android 型（Android アプリ用）

| 項目 | 値 |
|------|----|
| アプリケーションの種類 | Android |
| 名前 | `kakeibo-android` |
| パッケージ名 | Android アプリのパッケージ名 |
| SHA-1 証明書フィンガープリント | デバッグ用キーストアの SHA-1 |

SHA-1 の取得方法:
```bash
keytool -list -v -keystore ~/.android/debug.keystore \
  -alias androiddebugkey -storepass android -keypass android \
  | grep SHA1
```

### Android アプリ側の設定

Android アプリで Google Sign-In を実装する際、**Web アプリケーション型**のクライアント ID を audience として指定する。
これにより発行された ID Token の `aud` クレームが Cloud Run の検証を通過する。

```kotlin
val gso = GoogleSignInOptions.Builder(GoogleSignInOptions.DEFAULT_SIGN_IN)
    .requestIdToken("xxxxxx.apps.googleusercontent.com") // Web アプリケーション型のクライアント ID
    .build()
```

---

## ステップ⑦ Cloud Run 初回デプロイ

初回のみ手動で実施。以降は GitHub Actions で自動化する。

```bash
# Docker イメージのビルドとプッシュ
gcloud builds submit \
  --tag asia-northeast1-docker.pkg.dev/<project-id>/kakeibo/kakeibo-api:latest

# Cloud Run へデプロイ
gcloud run deploy kakeibo-api \
  --image asia-northeast1-docker.pkg.dev/<project-id>/kakeibo/kakeibo-api:latest \
  --region asia-northeast1 \
  --no-allow-unauthenticated \
  --platform managed \
  --set-env-vars GOOGLE_CLIENT_ID=<ステップ⑥で取得した Web アプリケーション型のクライアント ID>

# 確認
gcloud run services list
```

`--no-allow-unauthenticated` により Google 認証が必須になる。

> **注意**: `GOOGLE_CLIENT_ID` を指定しないとリリースビルドの起動時にエラーとなり、
> Cloud Run が「コンテナがポートを listen しなかった」として失敗する。

---

## ステップ⑧ GitHub Actions 用サービスアカウント・WIF 設定

GitHub Actions が GCP を操作するための権限設定。
JSON 鍵を持たない Workload Identity Federation（WIF）を使用する。

### WIF 用 API の有効化

```bash
gcloud services enable iamcredentials.googleapis.com
gcloud services enable sts.googleapis.com
```

### サービスアカウント作成と権限付与

```bash
# サービスアカウント作成
gcloud iam service-accounts create github-actions \
  --display-name="GitHub Actions"

# 必要な権限を付与
gcloud projects add-iam-policy-binding <project-id> \
  --member="serviceAccount:github-actions@<project-id>.iam.gserviceaccount.com" \
  --role="roles/run.admin"

gcloud projects add-iam-policy-binding <project-id> \
  --member="serviceAccount:github-actions@<project-id>.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.writer"

gcloud projects add-iam-policy-binding <project-id> \
  --member="serviceAccount:github-actions@<project-id>.iam.gserviceaccount.com" \
  --role="roles/iam.serviceAccountUser"
```

### Workload Identity Pool の作成

```bash
gcloud iam workload-identity-pools create github-actions \
  --project=<project-id> \
  --location=global \
  --display-name="GitHub Actions"
```

### OIDC Provider の作成

```bash
gcloud iam workload-identity-pools providers create-oidc github \
  --project=<project-id> \
  --location=global \
  --workload-identity-pool=github-actions \
  --display-name="GitHub" \
  --issuer-uri="https://token.actions.githubusercontent.com" \
  --attribute-mapping="google.subject=assertion.sub,attribute.repository=assertion.repository" \
  --attribute-condition='assertion.repository=="aklzo/kakeibo-cli"'
```

> **注意**: `--attribute-condition` の外側はシングルクォート、内側の文字列はダブルクォートにすること。
> シングルクォートで囲むと CEL 式内のダブルクォートをエスケープせずに書ける。

### サービスアカウントへの impersonate 権限付与

```bash
# プロジェクト番号を取得
PROJECT_NUMBER=$(gcloud projects describe <project-id> --format="value(projectNumber)")

gcloud iam service-accounts add-iam-policy-binding \
  github-actions@<project-id>.iam.gserviceaccount.com \
  --project=<project-id> \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/github-actions/attribute.repository/aklzo/kakeibo-cli"
```

### WIF_PROVIDER の値を確認

```bash
gcloud iam workload-identity-pools providers describe github \
  --project=<project-id> \
  --location=global \
  --workload-identity-pool=github-actions \
  --format="value(name)"
# 出力例: projects/123456789/locations/global/workloadIdentityPools/github-actions/providers/github
```

---

## ステップ⑨ GitHub Secrets の設定

**ブラウザで実施**

```
GitHub リポジトリ
→ Settings → Secrets and variables → Actions → New repository secret

登録する値：
  Name:  WIF_PROVIDER
  Value: ステップ⑧の確認コマンドで出力された projects/... の文字列

  Name:  WIF_SERVICE_ACCOUNT
  Value: github-actions@<project-id>.iam.gserviceaccount.com

  Name:  GCP_PROJECT_ID
  Value: kakeibo-<your-id>

  Name:  GOOGLE_CLIENT_ID
  Value: ステップ⑥で取得した Web アプリケーション型のクライアント ID
```

---

## ステップ⑩ Turso のセットアップ

Turso は GCP とは独立したサービス（SQLite 互換のクラウド DB）。

```bash
# Turso CLI のインストール
brew install tursodatabase/tap/turso

# ログイン
turso auth login

# DB の作成（nrt = 東京リージョン）
turso db create kakeibo --location nrt

# 接続情報の取得
turso db show kakeibo --url     # DATABASE_URL として使用
turso db tokens create kakeibo  # AUTH_TOKEN として使用
```

取得した値を Cloud Run の環境変数に設定する。

```bash
gcloud run services update kakeibo-api \
  --region asia-northeast1 \
  --update-env-vars DATABASE_URL=<turso-url>,AUTH_TOKEN=<turso-token>
```

Turso の接続情報も GitHub Secrets に登録する。

```
Name:  TURSO_DATABASE_URL
Value: <turso-url>

Name:  TURSO_AUTH_TOKEN
Value: <turso-token>
```

---

## 手順まとめ

| ステップ | 作業 | 方法 |
|---------|------|------|
| ① | GCP アカウント・プロジェクト作成 | ブラウザ |
| ② | gcloud CLI インストール | CLI |
| ③ | gcloud 初期設定 | CLI |
| ④ | API の有効化 | CLI |
| ⑤ | Artifact Registry 作成 | CLI |
| ⑥ | OAuth 2.0 クライアント ID 作成 | ブラウザ |
| ⑦ | Cloud Run 初回デプロイ | CLI |
| ⑧ | サービスアカウント・WIF 設定 | CLI |
| ⑨ | GitHub Secrets 設定 | ブラウザ |
| ⑩ | Turso セットアップ | CLI |

ブラウザが必要な箇所は①⑥⑨。

---

## 参考

- [Cloud Run 公式ドキュメント](https://cloud.google.com/run/docs)
- [gcloud CLI リファレンス](https://cloud.google.com/sdk/gcloud/reference)
- [Turso ドキュメント](https://docs.turso.tech)
