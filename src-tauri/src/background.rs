//! バックグラウンド常時実行 (× でシステムトレイに格納) とスタートアップ起動の制御。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::auth_log;

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const MENU_OPEN_ID: &str = "open";
const MENU_QUIT_ID: &str = "quit";

/// Run in background が有効か。ウィンドウの close 要求時と定期取得の判定で参照する
#[derive(Default)]
pub struct BackgroundState {
    is_enabled: AtomicBool,
}

impl BackgroundState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled.load(Ordering::Relaxed)
    }
}

/// Run in background の設定を反映する。有効な間だけトレイアイコンを表示する
pub fn apply(app: &AppHandle, enabled: bool) {
    let was_enabled = app.state::<BackgroundState>().is_enabled.swap(enabled, Ordering::Relaxed);
    if was_enabled != enabled {
        auth_log::log!("background: run in background {}", if enabled { "enabled" } else { "disabled" });
    }
    let has_tray = app.tray_by_id(TRAY_ID).is_some();
    if enabled && !has_tray {
        match build_tray(app) {
            Ok(()) => auth_log::log!("background: tray icon created"),
            Err(err) => {
                eprintln!("Failed to create the tray icon: {err}");
                auth_log::log!("background: failed to create the tray icon: {err}");
            }
        }
    } else if !enabled && has_tray {
        let _ = app.remove_tray_by_id(TRAY_ID);
        auth_log::log!("background: tray icon removed");
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN_ID, "Open Polaris", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Polaris")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN_ID => {
                auth_log::log!("tray: open selected");
                show_main_window(app);
            }
            MENU_QUIT_ID => {
                auth_log::log!("tray: quit selected");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                auth_log::log!("tray: icon clicked");
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// トレイ格納・最小化中のメインウィンドウを前面に表示する
pub fn show_main_window(app: &AppHandle) {
    auth_log::log!("window: show requested");
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// バックグラウンドの定期取得をするかの判定に使う条件。ログにも出せるよう、判定とは別に値を持つ
#[derive(Debug, Clone, Copy)]
pub struct PollConditions {
    /// Run in background の設定
    pub is_enabled: bool,
    /// メインウィンドウが存在するか
    pub has_window: bool,
    /// `None` は OS への問い合わせに失敗したとき
    pub is_visible: Option<bool>,
    pub is_minimized: Option<bool>,
}

impl PollConditions {
    /// Run in background が有効で、ウィンドウが見えていない (トレイ格納・最小化) 間は Rust 側で観測を続ける。
    /// ウィンドウ表示中はフロントエンドの自動更新が取得する
    pub fn should_poll(&self) -> bool {
        self.is_enabled && self.has_window && (!self.is_visible.unwrap_or(true) || self.is_minimized.unwrap_or(false))
    }
}

pub fn poll_conditions(app: &AppHandle) -> PollConditions {
    let is_enabled = app.state::<BackgroundState>().is_enabled();
    let window = app.get_webview_window(MAIN_WINDOW_LABEL);
    PollConditions {
        is_enabled,
        has_window: window.is_some(),
        is_visible: window.as_ref().and_then(|window| window.is_visible().ok()),
        is_minimized: window.as_ref().and_then(|window| window.is_minimized().ok()),
    }
}

/// 直近に記録した最小化状態。変化したときだけログに残すために持つ
static LAST_LOGGED_MINIMIZED: AtomicBool = AtomicBool::new(false);

/// メインウィンドウのイベントのうち、定期取得の判断に関わるもの (最小化・フォーカス・× での格納) をログに残す
pub fn log_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }
    match event {
        // 最小化・復元はサイズ変更として通知されるので、最小化状態が変わったときだけ記録する
        tauri::WindowEvent::Resized(_) => {
            let is_minimized = window.is_minimized().unwrap_or(false);
            if LAST_LOGGED_MINIMIZED.swap(is_minimized, Ordering::Relaxed) != is_minimized {
                auth_log::log!(
                    "window: {} (visible={:?})",
                    if is_minimized { "minimized" } else { "restored from minimized" },
                    window.is_visible().ok()
                );
            }
        }
        tauri::WindowEvent::Focused(is_focused) => auth_log::log!("window: focus {}", if *is_focused { "gained" } else { "lost" }),
        tauri::WindowEvent::CloseRequested { .. } => {
            let is_enabled = window.state::<BackgroundState>().is_enabled();
            auth_log::log!("window: close requested ({})", if is_enabled { "hide to tray" } else { "exit" });
        }
        tauri::WindowEvent::Destroyed => auth_log::log!("window: destroyed"),
        _ => {}
    }
}

/// Launch at startup の設定を OS の自動起動登録に反映する
pub fn set_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else if autolaunch.is_enabled().unwrap_or(false) {
        autolaunch.disable()
    } else {
        Ok(())
    };
    result.map_err(|err| err.to_string())
}

/// 起動時に呼ぶ。有効なら登録し直し、アップデート等で実行ファイルのパスが変わっても追従させる
pub fn sync_autostart(app: &AppHandle, enabled: bool) {
    if let Err(err) = set_autostart(app, enabled) {
        eprintln!("Failed to sync launch at startup: {err}");
        auth_log::log!("background: failed to sync launch at startup (enabled={enabled}): {err}");
    }
}
