//! VRChat API クライアント (認証まわり)。
//! auth cookie は Rust 側の cookie jar にのみ保持し、WebView には渡さない。

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{
    cookie::{CookieStore, Jar},
    header, Client, Response, StatusCode, Url,
};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::vrchat_models::{ApiFriend, ApiGroupInstanceList, ApiUserGroup, ApiWorld};

const API_BASE: &str = "https://api.vrchat.cloud/api/1";
/// VRChat API は識別可能な User-Agent を必須としている
pub const USER_AGENT: &str = concat!("Polaris/", env!("CARGO_PKG_VERSION"), " noma-t");

/// `GET /auth/user/friends` の 1 ページあたりの最大件数
pub const FRIENDS_PAGE_SIZE: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid username or password.")]
    InvalidCredentials,
    #[error("Invalid verification code.")]
    InvalidCode,
    #[error("Session expired. Please sign in again.")]
    Unauthorized,
    #[error("Rate limited by VRChat. Please wait and try again.")]
    RateLimited,
    #[error("Could not reach VRChat: {0}")]
    Network(String),
    #[error("{0}")]
    Unexpected(String),
}

impl AuthError {
    fn kind(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "invalidCredentials",
            Self::InvalidCode => "invalidCode",
            Self::Unauthorized => "unauthorized",
            Self::RateLimited => "rateLimited",
            Self::Network(_) => "network",
            Self::Unexpected(_) => "unexpected",
        }
    }
}

/// フロントエンドへは `{ kind, message }` 形式で渡す
impl Serialize for AuthError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AuthError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl From<reqwest::Error> for AuthError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "ApiCurrentUser")]
pub struct CurrentUser {
    pub id: String,
    pub display_name: String,
    /// 表示用アイコンの元 URL。未設定なら None
    pub icon_url: Option<String>,
}

/// `GET /auth/user` のレスポンスのうち必要なフィールド。未設定の画像は空文字列で返る
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiCurrentUser {
    id: String,
    display_name: String,
    #[serde(default)]
    user_icon: String,
    #[serde(default)]
    profile_pic_override_thumbnail: String,
    #[serde(default)]
    profile_pic_override: String,
    #[serde(default)]
    current_avatar_thumbnail_image_url: String,
}

impl From<ApiCurrentUser> for CurrentUser {
    /// VRChat クライアントと同じく userIcon → profilePicOverride → アバターサムネイルの順で採用する
    fn from(user: ApiCurrentUser) -> Self {
        let icon_url = [
            user.user_icon,
            user.profile_pic_override_thumbnail,
            user.profile_pic_override,
            user.current_avatar_thumbnail_image_url,
        ]
        .into_iter()
        .find(|url| !url.is_empty());
        Self {
            id: user.id,
            display_name: user.display_name,
            icon_url,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TwoFactorMethod {
    Totp,
    EmailOtp,
}

impl TwoFactorMethod {
    fn from_api(name: &str) -> Option<Self> {
        match name {
            "totp" => Some(Self::Totp),
            "emailOtp" => Some(Self::EmailOtp),
            _ => None,
        }
    }

    fn endpoint(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::EmailOtp => "emailotp",
        }
    }
}

/// `GET /auth/user` の結果
pub enum AuthUserResponse {
    SignedIn(CurrentUser),
    RequiresTwoFactor(Vec<TwoFactorMethod>),
    Unauthorized,
}

#[derive(Clone)]
pub struct VrchatClient {
    http: Client,
    cookie_jar: Arc<Jar>,
}

impl VrchatClient {
    pub fn new() -> Self {
        let cookie_jar = Arc::new(Jar::default());
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(cookie_jar.clone())
            .build()
            .expect("failed to build HTTP client");
        Self { http, cookie_jar }
    }

    fn api_url(path: &str) -> Url {
        Url::parse(&format!("{API_BASE}{path}")).expect("invalid API URL")
    }

    /// Basic 認証で `auth` cookie を取得する
    pub async fn login_with_basic(&self, username: &str, password: &str) -> Result<AuthUserResponse, AuthError> {
        let credentials = format!("{}:{}", urlencoding::encode(username), urlencoding::encode(password));
        let response = self
            .http
            .get(Self::api_url("/auth/user"))
            .header(header::AUTHORIZATION, format!("Basic {}", STANDARD.encode(credentials)))
            .send()
            .await?;
        Self::parse_auth_user(response).await
    }

    /// cookie のみで現在のユーザーを取得する
    pub async fn get_current_user(&self) -> Result<AuthUserResponse, AuthError> {
        let response = self.http.get(Self::api_url("/auth/user")).send().await?;
        Self::parse_auth_user(response).await
    }

    pub async fn verify_two_factor(&self, method: TwoFactorMethod, code: &str) -> Result<(), AuthError> {
        let path = format!("/auth/twofactorauth/{}/verify", method.endpoint());
        let response = self
            .http
            .post(Self::api_url(&path))
            .json(&serde_json::json!({ "code": code }))
            .send()
            .await?;
        match response.status() {
            StatusCode::TOO_MANY_REQUESTS => Err(AuthError::RateLimited),
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED => Err(AuthError::InvalidCode),
            status if status.is_success() => {
                let body: Value = response.json().await?;
                if body.get("verified").and_then(Value::as_bool) == Some(true) {
                    Ok(())
                } else {
                    Err(AuthError::InvalidCode)
                }
            }
            status => Err(AuthError::Unexpected(format!("Unexpected response from VRChat ({status})."))),
        }
    }

    /// VRChat 配下の画像を auth cookie 付きで取得し、`(Content-Type, 本体)` を返す。
    /// cookie 付きリクエストを任意の URL に送らないよう、VRChat のホスト以外は拒否する
    pub async fn fetch_image(&self, url: &str) -> Result<(Option<String>, Vec<u8>), AuthError> {
        let url = Url::parse(url).map_err(|err| AuthError::Unexpected(format!("Invalid image URL: {err}")))?;
        let is_vrchat_host = url
            .host_str()
            .is_some_and(|host| ["vrchat.cloud", "vrchat.com"].iter().any(|d| host == *d || host.ends_with(&format!(".{d}"))));
        if url.scheme() != "https" || !is_vrchat_host {
            return Err(AuthError::Unexpected(format!("Refused to fetch non-VRChat image: {url}")));
        }

        let response = self.http.get(url).send().await?;
        match response.status() {
            StatusCode::TOO_MANY_REQUESTS => return Err(AuthError::RateLimited),
            status if !status.is_success() => {
                return Err(AuthError::Unexpected(format!("Unexpected response from VRChat ({status}).")))
            }
            _ => {}
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await?.to_vec();
        Ok((content_type, body))
    }

    /// 1 ページ分 (最大 100 件) の friend を取得する。`offline` で online / offline のどちらかを選ぶ
    pub async fn get_friends(&self, offline: bool, offset: usize) -> Result<Vec<ApiFriend>, AuthError> {
        let mut url = Self::api_url("/auth/user/friends");
        url.query_pairs_mut()
            .append_pair("offline", if offline { "true" } else { "false" })
            .append_pair("n", &FRIENDS_PAGE_SIZE.to_string())
            .append_pair("offset", &offset.to_string());
        self.get_json(url).await
    }

    pub async fn get_world(&self, world_id: &str) -> Result<ApiWorld, AuthError> {
        self.get_json(Self::api_url(&format!("/worlds/{}", urlencoding::encode(world_id)))).await
    }

    pub async fn get_user_groups(&self, user_id: &str) -> Result<Vec<ApiUserGroup>, AuthError> {
        self.get_json(Self::api_url(&format!("/users/{}/groups", urlencoding::encode(user_id)))).await
    }

    /// 所属する全グループのインスタンスを 1 リクエストで取得する
    pub async fn get_user_group_instances(&self, user_id: &str) -> Result<ApiGroupInstanceList, AuthError> {
        self.get_json(Self::api_url(&format!("/users/{}/instances/groups", urlencoding::encode(user_id)))).await
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T, AuthError> {
        let response = self.http.get(url).send().await?;
        match response.status() {
            StatusCode::UNAUTHORIZED => return Err(AuthError::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => return Err(AuthError::RateLimited),
            status if !status.is_success() => {
                return Err(AuthError::Unexpected(format!("Unexpected response from VRChat ({status}).")))
            }
            _ => {}
        }
        response
            .json()
            .await
            .map_err(|err| AuthError::Unexpected(format!("Failed to parse VRChat response: {err}")))
    }

    /// pipeline (WebSocket) 接続用に `auth` cookie の値を取り出す
    pub fn auth_token(&self) -> Option<String> {
        self.export_cookies().as_deref().and_then(extract_auth_token)
    }

    pub async fn logout(&self) -> Result<(), AuthError> {
        self.http.put(Self::api_url("/logout")).send().await?;
        Ok(())
    }

    /// 永続化用に cookie を `name=value; name=value` 形式で書き出す
    pub fn export_cookies(&self) -> Option<String> {
        self.cookie_jar
            .cookies(&Self::api_url("/"))
            .and_then(|value| value.to_str().ok().map(str::to_owned))
    }

    pub fn import_cookies(&self, cookies: &str) {
        let url = Self::api_url("/");
        for pair in cookies.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            self.cookie_jar.add_cookie_str(&format!("{pair}; Path=/"), &url);
        }
    }

    async fn parse_auth_user(response: Response) -> Result<AuthUserResponse, AuthError> {
        match response.status() {
            StatusCode::UNAUTHORIZED => return Ok(AuthUserResponse::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => return Err(AuthError::RateLimited),
            status if !status.is_success() => {
                return Err(AuthError::Unexpected(format!("Unexpected response from VRChat ({status}).")))
            }
            _ => {}
        }

        let body: Value = response.json().await?;
        if let Some(methods) = body.get("requiresTwoFactorAuth").and_then(Value::as_array) {
            let methods = methods
                .iter()
                .filter_map(Value::as_str)
                .filter_map(TwoFactorMethod::from_api)
                .collect();
            return Ok(AuthUserResponse::RequiresTwoFactor(methods));
        }

        serde_json::from_value(body)
            .map(AuthUserResponse::SignedIn)
            .map_err(|err| AuthError::Unexpected(format!("Failed to parse user: {err}")))
    }
}

/// `name=value; name=value` 形式の cookie 文字列から `auth` の値を取り出す
fn extract_auth_token(cookies: &str) -> Option<String> {
    cookies
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, value)| *name == "auth" && !value.is_empty())
        .map(|(_, value)| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::extract_auth_token;

    #[test]
    fn extracts_auth_token_from_cookie_header() {
        let cookies = "twoFactorAuth=tfa_value; auth=authcookie_abc123";
        assert_eq!(extract_auth_token(cookies).as_deref(), Some("authcookie_abc123"));
    }

    #[test]
    fn returns_none_without_auth_cookie() {
        assert_eq!(extract_auth_token("twoFactorAuth=tfa_value"), None);
        assert_eq!(extract_auth_token("auth="), None);
    }
}
