use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use serde_json::json;

/// axum の State から Google Client ID を取得するためのトレイト。
pub trait HasClientId {
    fn client_id(&self) -> &str;
}

/// axum の State から API の利用を許可するユーザーの一覧を取得するためのトレイト。
pub trait HasAllowedUsers {
    fn allowed_users(&self) -> &AllowedUsers;
}

/// 許可リストの区切り文字。
const ALLOWED_USERS_SEPARATOR: char = ',';

/// API の利用を許可する Google アカウントの一覧（ID Token の `sub` の集合）。
///
/// Google の ID Token は、このアプリのクライアント ID 宛てであれば
/// どの Google アカウントでも発行される。個人利用に限定するため、
/// 署名・発行者・宛先の検証に加えて、この一覧に含まれる `sub` だけを通す。
///
/// axum の State はリクエストごとに複製されるため、中身は `Arc` で共有する。
#[derive(Debug, Clone, Default)]
pub struct AllowedUsers(Arc<HashSet<String>>);

impl AllowedUsers {
    /// カンマ区切りの `sub` 一覧（例: `"111,222"`）から許可リストを作る。
    ///
    /// 各要素の前後の空白と空の要素は無視する。
    /// 有効な `sub` が 1 件もない場合は、誰も使えない設定ミスとしてエラーを返す。
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let users: HashSet<String> = raw
            .split(ALLOWED_USERS_SEPARATOR)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if users.is_empty() {
            anyhow::bail!("許可するユーザーが 1 件も指定されていません");
        }
        Ok(Self(Arc::new(users)))
    }

    /// 誰も許可しない許可リスト。デバッグビルドで未設定のときに使う。
    pub fn deny_all() -> Self {
        Self::default()
    }

    /// 指定した `sub` が許可されているかを返す。
    fn is_allowed(&self, user_id: &str) -> bool {
        self.0.contains(user_id)
    }
}

/// Google ID Token 検証後のユーザー情報。axum エクストラクタとして使用する。
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// Google ID Token の `sub` クレーム。repository の user_id として使用する。
    pub user_id: String,
}

/// 認証エラーの HTTP レスポンス。
pub struct AuthError(StatusCode, &'static str);

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    n: String,
    e: String,
}

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUER_SHORT: &str = "accounts.google.com";
const GOOGLE_ISSUER_HTTPS: &str = "https://accounts.google.com";

/// 開発用: デバッグビルドかつ `SKIP_AUTH=true` のとき認証を省略する。
#[cfg(debug_assertions)]
const SKIP_AUTH_ENV: &str = "SKIP_AUTH";

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync + HasClientId + HasAllowedUsers,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[cfg(debug_assertions)]
        if std::env::var(SKIP_AUTH_ENV).as_deref() == Ok("true") {
            return Ok(AuthUser {
                user_id: "local".to_string(),
            });
        }
        let token = extract_bearer(parts)?;
        let user_id = verify_token(&token, state.client_id())
            .await
            .map_err(|_| AuthError(StatusCode::UNAUTHORIZED, "Unauthorized"))?;
        authorize(user_id, state.allowed_users())
    }
}

/// 検証済みの `sub` が許可リストに含まれていれば `AuthUser` を返す。
///
/// トークン自体は正しいが許可されていないアカウントは 403 とする。
fn authorize(user_id: String, allowed_users: &AllowedUsers) -> Result<AuthUser, AuthError> {
    if allowed_users.is_allowed(&user_id) {
        Ok(AuthUser { user_id })
    } else {
        Err(AuthError(StatusCode::FORBIDDEN, "Forbidden"))
    }
}

/// `Authorization: Bearer <token>` ヘッダーからトークン文字列を取り出す。
fn extract_bearer(parts: &Parts) -> Result<String, AuthError> {
    parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_owned())
        .ok_or(AuthError(StatusCode::UNAUTHORIZED, "Unauthorized"))
}

/// Google ID Token を検証し `sub` クレームを返す。
async fn verify_token(token: &str, client_id: &str) -> anyhow::Result<String> {
    let header = decode_header(token)?;
    let kid = header
        .kid
        .ok_or_else(|| anyhow::anyhow!("missing kid in token header"))?;

    let jwks = reqwest::get(GOOGLE_JWKS_URL).await?.json::<Jwks>().await?;

    let key = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| anyhow::anyhow!("matching public key not found"))?;

    let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[GOOGLE_ISSUER_SHORT, GOOGLE_ISSUER_HTTPS]);
    validation.set_audience(&[client_id]);

    let data = decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(data.claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 正常系: 1 件だけ指定した sub が許可されること
    #[test]
    fn parse_single_user_allows_that_user() -> anyhow::Result<()> {
        let allowed = AllowedUsers::parse("111")?;
        assert!(allowed.is_allowed("111"));
        assert!(!allowed.is_allowed("222"));
        Ok(())
    }

    // 正常系: カンマ区切りの複数指定で、前後の空白を除いた各 sub が許可されること
    #[test]
    fn parse_comma_separated_users_trims_whitespace() -> anyhow::Result<()> {
        let allowed = AllowedUsers::parse(" 111 , 222 ")?;
        assert!(allowed.is_allowed("111"));
        assert!(allowed.is_allowed("222"));
        assert!(!allowed.is_allowed(" 111 "));
        Ok(())
    }

    // 正常系: 空の要素（連続したカンマ・末尾のカンマ）は無視されること
    #[test]
    fn parse_ignores_empty_entries() -> anyhow::Result<()> {
        let allowed = AllowedUsers::parse("111,,222,")?;
        assert!(allowed.is_allowed("111"));
        assert!(allowed.is_allowed("222"));
        assert!(!allowed.is_allowed(""));
        Ok(())
    }

    // 異常系: 空文字列は誰も使えない設定ミスとしてエラーになること
    #[test]
    fn parse_empty_string_returns_error() {
        assert!(AllowedUsers::parse("").is_err());
    }

    // 異常系: 区切り文字と空白だけの指定もエラーになること
    #[test]
    fn parse_only_separators_returns_error() {
        assert!(AllowedUsers::parse(" , ,").is_err());
    }

    // 正常系: deny_all は誰も許可しないこと
    #[test]
    fn deny_all_allows_nobody() {
        let allowed = AllowedUsers::deny_all();
        assert!(!allowed.is_allowed("111"));
        assert!(!allowed.is_allowed(""));
    }

    // 正常系: 許可リストに含まれる sub は AuthUser として通ること
    #[test]
    fn authorize_allowed_user_returns_auth_user() -> anyhow::Result<()> {
        let allowed = AllowedUsers::parse("111")?;
        let Ok(user) = authorize("111".to_owned(), &allowed) else {
            anyhow::bail!("許可されたユーザーが拒否されました");
        };
        assert_eq!(user.user_id, "111");
        Ok(())
    }

    // 異常系: 許可リストにない sub は 403 Forbidden になること
    #[test]
    fn authorize_unknown_user_returns_forbidden() -> anyhow::Result<()> {
        let allowed = AllowedUsers::parse("111")?;
        let Err(AuthError(status, message)) = authorize("222".to_owned(), &allowed) else {
            anyhow::bail!("許可されていないユーザーが通過しました");
        };
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(message, "Forbidden");
        Ok(())
    }
}
