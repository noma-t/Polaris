//! OS の資格情報ストア (Windows Credential Manager / macOS Keychain / Secret Service) への
//! VRChat セッション cookie の保存・読み出し。

use keyring::Entry;

use crate::auth_log;
use crate::vrchat_client::AuthError;

const SERVICE_NAME: &str = "com.polaris.vsuifinder";
const SESSION_ENTRY_NAME: &str = "vrchat-session";

fn session_entry() -> Result<Entry, AuthError> {
    Entry::new(SERVICE_NAME, SESSION_ENTRY_NAME).map_err(to_auth_error)
}

fn to_auth_error(err: keyring::Error) -> AuthError {
    AuthError::Unexpected(format!("Credential store error: {err}"))
}

pub fn save_session(cookies: &str) -> Result<(), AuthError> {
    let result = session_entry()?.set_password(cookies).map_err(to_auth_error);
    match &result {
        Ok(()) => auth_log::log!("credential store: saved session"),
        Err(err) => auth_log::log!("credential store: failed to save session: {err}"),
    }
    result
}

pub fn load_session() -> Result<Option<String>, AuthError> {
    match session_entry()?.get_password() {
        Ok(cookies) => Ok(Some(cookies)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => {
            auth_log::log!("credential store: failed to load session: {err}");
            Err(to_auth_error(err))
        }
    }
}

pub fn delete_session() -> Result<(), AuthError> {
    match session_entry()?.delete_credential() {
        Ok(()) => {
            auth_log::log!("credential store: deleted session");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => {
            auth_log::log!("credential store: failed to delete session: {err}");
            Err(to_auth_error(err))
        }
    }
}
