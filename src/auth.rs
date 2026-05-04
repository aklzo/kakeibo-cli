use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::json;

/// axum の State から Google Client ID を取得するためのトレイト。
pub trait HasClientId {
    fn client_id(&self) -> &str;
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
    S: Send + Sync + HasClientId,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[cfg(debug_assertions)]
        if std::env::var(SKIP_AUTH_ENV).as_deref() == Ok("true") {
            return Ok(AuthUser { user_id: "local".to_string() });
        }
        let token = extract_bearer(parts)?;
        verify_token(&token, state.client_id())
            .await
            .map(|user_id| AuthUser { user_id })
            .map_err(|_| AuthError(StatusCode::UNAUTHORIZED, "Unauthorized"))
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

    let jwks = reqwest::get(GOOGLE_JWKS_URL)
        .await?
        .json::<Jwks>()
        .await?;

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
