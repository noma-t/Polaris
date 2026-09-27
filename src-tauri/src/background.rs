//! バックグラウンド常時実行 (× でシステムトレイに格納) とスタートアップ起動の制御。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

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
    app.state::<BackgroundState>().is_enabled.store(enabled, Ordering::Relaxed);
    let has_tray = app.tray_by_id(TRAY_ID).is_some();
    if enabled && !has_tray {
        if let Err(err) = build_tray(app) {
            eprintln!("Failed to create the tray icon: {err}");
        }
    } else if !enabled && has_tray {
        let _ = app.remove_tray_by_id(TRAY_ID);
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
            MENU_OPEN_ID => show_main_window(app),
            MENU_QUIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
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
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Run in background が有効で、ウィンドウが見えていない (トレイ格納・最小化) 間は Rust 側で観測を続ける。
/// ウィンドウ表示中はフロントエンドの自動更新が取得する
pub fn should_poll_in_background(app: &AppHandle) -> bool {
    if !app.state::<BackgroundState>().is_enabled() {
        return false;
    }
    app.get_webview_window(MAIN_WINDOW_LABEL).is_some_and(|window| {
        !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false)
    })
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
    }
}
