//! Developer 向けの認証まわりの診断ログ。有効な間だけ app log dir の `auth.log` に追記する。
//! VRChat API へのリクエストと、ログイン・セッション復元・サインアウト・資格情報ストア・pipeline の出来事を記録する。
//! cookie・認証トークン・パスワード・2FA コードの値は記録しない。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::vrchat_client::AuthError;

const LOG_FILE_NAME: &str = "auth.log";
/// 上限を超えたら `auth.1.log` に退避して新しいファイルに書き始める (1 世代だけ残す)
const ROTATED_LOG_FILE_NAME: &str = "auth.1.log";
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

static IS_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 複数タスクからの追記とローテーションを直列化する
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// `auth_log::log!("...", args)`。無効な間は文字列の整形もしない
macro_rules! log {
    ($($arg:tt)*) => {
        if $crate::auth_log::is_enabled() {
            $crate::auth_log::write(&format!($($arg)*));
        }
    };
}
pub(crate) use log;

/// 起動時に 1 度呼ぶ
pub fn init(app: &AppHandle, is_enabled: bool) {
    if let Ok(dir) = app.path().app_log_dir() {
        let _ = LOG_DIR.set(dir);
    }
    set_enabled(is_enabled);
}

/// 有効・無効の切り替えもログに残す (無効化は切り替える前に書く)
pub fn set_enabled(is_enabled: bool) {
    if is_enabled == IS_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    if is_enabled {
        IS_ENABLED.store(true, Ordering::Relaxed);
        log!("---- auth logging enabled (Polaris {}) ----", env!("CARGO_PKG_VERSION"));
    } else {
        log!("---- auth logging disabled ----");
        IS_ENABLED.store(false, Ordering::Relaxed);
    }
}

pub fn is_enabled() -> bool {
    IS_ENABLED.load(Ordering::Relaxed)
}

pub fn write(message: &str) {
    let Some(dir) = LOG_DIR.get() else { return };
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Err(err) = append(dir, message) {
        eprintln!("[auth-log] failed to write: {err}");
    }
}

fn append(dir: &Path, message: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(LOG_FILE_NAME);
    if fs::metadata(&path).is_ok_and(|meta| meta.len() >= MAX_LOG_BYTES) {
        fs::rename(&path, dir.join(ROTATED_LOG_FILE_NAME))?;
    }
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{timestamp} {message}")
}

/// ログの保存先フォルダをエクスプローラー等で開く
#[tauri::command]
pub fn auth_log_open_folder(app: AppHandle) -> Result<(), AuthError> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|err| AuthError::Unexpected(format!("Could not resolve the log directory: {err}")))?;
    fs::create_dir_all(&dir).map_err(|err| AuthError::Unexpected(format!("Could not create the log directory: {err}")))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|err| AuthError::Unexpected(format!("Could not open the log directory: {err}")))
}
