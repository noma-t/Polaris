//! Developer 向けの診断ログ。有効な間だけ app log dir の `auth.log` に追記する (ファイル名は認証ログだった頃のまま)。
//! 時間や条件が重ならないと再現しない不具合を後から追えるよう、次を記録する。
//! - VRChat API へのリクエスト (開始・完了・応答待ちが長引いたとき)
//! - ログイン・セッション復元・サインアウト・資格情報ストア・pipeline の出来事
//! - バックグラウンドのタスク (開始・終了・panic) と定期取得の各周期
//! - ウィンドウと画面の状態変化、フロントエンドの自動更新の判断
//!
//! cookie・認証トークン・パスワード・2FA コードの値は記録しない。

use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::async_runtime::JoinHandle;
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

/// 起動時に 1 度呼ぶ。panic はタスクの中で起きると握りつぶされるので、フックで必ず記録する
pub fn init(app: &AppHandle, is_enabled: bool) {
    if let Ok(dir) = app.path().app_log_dir() {
        let _ = LOG_DIR.set(dir);
    }
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log!(
            "PANIC on thread {:?}: {info}\n{}",
            std::thread::current().name().unwrap_or("<unnamed>"),
            std::backtrace::Backtrace::force_capture()
        );
        default_hook(info);
    }));
    set_enabled(is_enabled);
}

/// `future` をタスクとして起動し、開始・終了・panic・abort をログに残す。
/// 返したハンドルを abort すると中のタスクも止まる。ログが無いまま止まったタスクを見分けるために使う
pub fn spawn_logged(name: &'static str, future: impl Future<Output = ()> + Send + 'static) -> JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        /// 外側のタスクが abort されて drop されたとき、中のタスクも止める
        struct AbortOnDrop(JoinHandle<()>);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        /// await の途中で abort されたことを記録する (完了したら `is_done` を立てて無効にする)
        struct LogOnAbort {
            name: &'static str,
            is_done: bool,
        }
        impl Drop for LogOnAbort {
            fn drop(&mut self) {
                if !self.is_done {
                    log!("task {}: aborted", self.name);
                }
            }
        }

        log!("task {name}: started");
        let mut on_abort = LogOnAbort { name, is_done: false };
        let mut inner = AbortOnDrop(tauri::async_runtime::spawn(future));
        match (&mut inner.0).await {
            Ok(()) => log!("task {name}: finished"),
            Err(err) => log!("task {name}: failed: {err}"),
        }
        on_abort.is_done = true;
    })
}

/// フロントエンドの判断をログに残す。呼び出しは状態が変わったときだけにする
#[tauri::command]
pub fn auth_log_write(message: String) {
    const MAX_CHARS: usize = 2000;
    match message.char_indices().nth(MAX_CHARS) {
        Some((end, _)) => log!("ui: {}... ({} chars)", &message[..end], message.chars().count()),
        None => log!("ui: {message}"),
    }
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
