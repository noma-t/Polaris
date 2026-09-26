//! VRChat の画像を WebView に配信するカスタム URI scheme (`vrcimg`)。
//! VRChat の画像 URL は auth cookie を要求するため、Rust 側の認証済みクライアントで代理取得する。
//! フロントエンドは `convertFileSrc(url, "vrcimg")` で元 URL をこの scheme の URL に変換して使う。

use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::auth_commands::AuthState;
use crate::vrchat_client::AuthError;

pub const SCHEME: &str = "vrcimg";

/// 同じ URL は内容が変わらないため WebView 側でキャッシュさせる
const CACHE_CONTROL: &str = "private, max-age=86400";

pub fn handle<R: Runtime>(ctx: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let client = ctx.app_handle().state::<AuthState>().client();
    // パスは `/<percent-encoded な元 URL>` 形式
    let encoded = request.uri().path().trim_start_matches('/').to_owned();

    tauri::async_runtime::spawn(async move {
        let result = match urlencoding::decode(&encoded) {
            Ok(url) => client.fetch_image(&url).await,
            Err(err) => Err(AuthError::Unexpected(format!("Invalid image path: {err}"))),
        };
        let response = match result {
            Ok((content_type, body)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type.as_deref().unwrap_or("application/octet-stream"))
                .header(header::CACHE_CONTROL, CACHE_CONTROL)
                .body(body),
            Err(err) => Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(err.to_string().into_bytes()),
        };
        responder.respond(response.expect("failed to build image response"));
    });
}
