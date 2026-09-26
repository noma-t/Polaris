//! ログイン・2FA・セッション復元・ログアウトの Tauri コマンド。

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::credential_store;
use crate::social::SocialState;
use crate::vrchat_client::{AuthError, AuthUserResponse, CurrentUser, TwoFactorMethod, VrchatClient};

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
    // 前回セッションの cookie が残らないよう新しいクライアントで開始する
    social.stop();
    let client = state.reset();
    match client.login_with_basic(username.trim(), &password).await? {
        AuthUserResponse::SignedIn(user) => {
            persist_session(&client)?;
            social.start(app, client, user.id.clone());
            Ok(LoginResult::SignedIn { user })
        }
        AuthUserResponse::RequiresTwoFactor(methods) if !methods.is_empty() => Ok(LoginResult::RequiresTwoFactor { methods }),
        AuthUserResponse::RequiresTwoFactor(_) => Err(AuthError::Unexpected(
            "This account uses a two-factor method Polaris does not support.".into(),
        )),
        AuthUserResponse::Unauthorized => Err(AuthError::InvalidCredentials),
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
    client.verify_two_factor(method, code.trim()).await?;
    match client.get_current_user().await? {
        AuthUserResponse::SignedIn(user) => {
            persist_session(&client)?;
            social.start(app, client, user.id.clone());
            Ok(user)
        }
        AuthUserResponse::RequiresTwoFactor(_) => Err(AuthError::InvalidCode),
        AuthUserResponse::Unauthorized => Err(AuthError::Unexpected("Session expired. Please sign in again.".into())),
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
        return Ok(None);
    };
    let client = state.reset();
    client.import_cookies(&cookies);
    match client.get_current_user().await? {
        AuthUserResponse::SignedIn(user) => {
            social.start(app, client, user.id.clone());
            Ok(Some(user))
        }
        AuthUserResponse::RequiresTwoFactor(_) | AuthUserResponse::Unauthorized => {
            credential_store::delete_session()?;
            state.reset();
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AuthState>, social: State<'_, SocialState>) -> Result<(), AuthError> {
    social.stop();
    // API 側の失敗 (オフライン等) に関わらずローカルのセッションは必ず破棄する
    let _ = state.client().logout().await;
    state.reset();
    credential_store::delete_session()
}
