//! ログイン・2FA・セッション復元・ログアウトの Tauri コマンド。

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::auth_log;
use crate::credential_store;
use crate::social::SocialState;
use crate::vrchat_client::{cookie_names, AuthError, AuthUserResponse, CurrentUser, TwoFactorMethod, VrchatClient};

/// 認証済みの cookie jar を持つクライアント。ログアウト時は丸ごと作り直す
pub struct AuthState(Mutex<VrchatClient>);

impl AuthState {
    pub fn new() -> Self {
        Self(Mutex::new(VrchatClient::new()))
    }

    /// reqwest::Client は内部が Arc なので clone して await 中にロックを持たない
    pub fn client(&self) -> VrchatClient {
        self.0.lock().expect("auth state poisoned").clone()
    }

    fn reset(&self) -> VrchatClient {
        let client = VrchatClient::new();
        *self.0.lock().expect("auth state poisoned") = client.clone();
        client
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LoginResult {
    SignedIn { user: CurrentUser },
    RequiresTwoFactor { methods: Vec<TwoFactorMethod> },
}

fn persist_session(client: &VrchatClient) -> Result<(), AuthError> {
    match client.export_cookies() {
        Some(cookies) => credential_store::save_session(&cookies),
        None => Err(AuthError::Unexpected("No session cookie was issued by VRChat.".into())),
    }
}

#[tauri::command]
pub async fn auth_login(
    app: AppHandle,
    state: State<'_, AuthState>,
    social: State<'_, SocialState>,
    username: String,
    password: String,
) -> Result<LoginResult, AuthError> {
    auth_log::log!("login: start");
    // 前回セッションの cookie が残らないよう新しいクライアントで開始する
    social.stop();
    let client = state.reset();
    let response = client
        .login_with_basic(username.trim(), &password)
        .await
        .inspect_err(|err| auth_log::log!("login: failed: {err}"))?;
    match response {
        AuthUserResponse::SignedIn(user) => {
            auth_log::log!("login: signed in as {} cookies=[{}]", user.id, client.cookie_names().join(", "));
            persist_session(&client)?;
            social.start(app, client, user.id.clone());
            Ok(LoginResult::SignedIn { user })
        }
        AuthUserResponse::RequiresTwoFactor(methods) if !methods.is_empty() => {
            auth_log::log!("login: requires two-factor {methods:?}");
            Ok(LoginResult::RequiresTwoFactor { methods })
        }
        AuthUserResponse::RequiresTwoFactor(_) => {
            auth_log::log!("login: requires an unsupported two-factor method");
            Err(AuthError::Unexpected("This account uses a two-factor method Polaris does not support.".into()))
        }
        AuthUserResponse::Unauthorized => {
            auth_log::log!("login: invalid credentials");
            Err(AuthError::InvalidCredentials)
        }
    }
}

#[tauri::command]
pub async fn auth_verify_two_factor(
    app: AppHandle,
    state: State<'_, AuthState>,
    social: State<'_, SocialState>,
    method: TwoFactorMethod,
    code: String,
) -> Result<CurrentUser, AuthError> {
    let client = state.client();
    let result = async {
        client.verify_two_factor(method, code.trim()).await?;
        client.get_current_user().await
    }
    .await
    .inspect_err(|err| auth_log::log!("two-factor ({method:?}): failed: {err}"))?;
    match result {
        AuthUserResponse::SignedIn(user) => {
            auth_log::log!("two-factor ({method:?}): signed in as {} cookies=[{}]", user.id, client.cookie_names().join(", "));
            persist_session(&client)?;
            social.start(app, client, user.id.clone());
            Ok(user)
        }
        AuthUserResponse::RequiresTwoFactor(_) => {
            auth_log::log!("two-factor ({method:?}): still requires two-factor");
            Err(AuthError::InvalidCode)
        }
        AuthUserResponse::Unauthorized => {
            auth_log::log!("two-factor ({method:?}): unauthorized after verification");
            Err(AuthError::Unexpected("Session expired. Please sign in again.".into()))
        }
    }
}

/// 保存済み cookie でセッションを復元する。無効なら保存内容を削除して None を返す
#[tauri::command]
pub async fn auth_restore_session(
    app: AppHandle,
    state: State<'_, AuthState>,
    social: State<'_, SocialState>,
) -> Result<Option<CurrentUser>, AuthError> {
    let Some(cookies) = credential_store::load_session()? else {
        auth_log::log!("restore: no saved session");
        return Ok(None);
    };
    auth_log::log!("restore: loaded saved session cookies=[{}]", cookie_names(&cookies).join(", "));
    let client = state.reset();
    client.import_cookies(&cookies);
    // ネットワークエラー等では保存済みセッションを残したままログイン画面に戻る
    let response = client
        .get_current_user()
        .await
        .inspect_err(|err| auth_log::log!("restore: failed (saved session kept): {err}"))?;
    match response {
        AuthUserResponse::SignedIn(user) => {
            auth_log::log!("restore: signed in as {}", user.id);
            social.start(app, client, user.id.clone());
            Ok(Some(user))
        }
        AuthUserResponse::RequiresTwoFactor(_) | AuthUserResponse::Unauthorized => {
            auth_log::log!("restore: saved session was rejected by VRChat; deleting it");
            credential_store::delete_session()?;
            state.reset();
            Ok(None)
        }
    }
}

/// サインアウトのきっかけ (認証ログに記録する)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignOutReason {
    /// ユーザーが Sign out を押した
    User,
    /// API が 401 を返し、フロントエンドが session-expired を受けた
    SessionExpired,
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AuthState>, social: State<'_, SocialState>, reason: SignOutReason) -> Result<(), AuthError> {
    auth_log::log!("logout: reason={reason:?}");
    social.stop();
    // API 側の失敗 (オフライン等) に関わらずローカルのセッションは必ず破棄する
    let _ = state.client().logout().await;
    state.reset();
    credential_store::delete_session()
}
