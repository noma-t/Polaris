mod auth_commands;
mod auth_log;
mod background;
mod credential_store;
mod game_monitor;
mod image_protocol;
mod instance_store;
mod pipeline;
mod settings_store;
mod social;
mod social_commands;
mod vrchat_client;
mod vrchat_models;

use auth_commands::AuthState;
use background::BackgroundState;
use game_monitor::GameState;
use settings_store::SettingsState;
use social::SocialState;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_window_state::StateFlags;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // tokio-tungstenite (rustls) は process 既定の CryptoProvider を使うため、先に ring を登録しておく
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        // トレイ格納中に再度起動された場合は、新しいプロセスを立ち上げずに既存のウィンドウを表示する
        .plugin(tauri_plugin_single_instance::init(|app, _, _| background::show_main_window(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        // 位置は OS 既定のままにし、サイズと最大化状態だけを復元する
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::MAXIMIZED)
                .build(),
        )
        .manage(AuthState::new())
        .manage(SocialState::new())
        .manage(SettingsState::new())
        .manage(GameState::new())
        .manage(BackgroundState::new())
        .setup(|app| {
            let settings = settings_store::load_app_settings(app.handle());
            auth_log::init(app.handle(), settings.is_auth_logging_enabled());
            app.state::<GameState>().start_monitor(app.handle().clone(), settings.launcher_path);
            background::apply(app.handle(), settings.run_in_background);
            background::sync_autostart(app.handle(), settings.launch_at_startup);
            Ok(())
        })
        // Run in background が有効なら、× ではプロセスを終了せずトレイに格納する
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<BackgroundState>().is_enabled() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .register_asynchronous_uri_scheme_protocol(image_protocol::SCHEME, image_protocol::handle)
        .invoke_handler(tauri::generate_handler![
            auth_commands::auth_login,
            auth_commands::auth_verify_two_factor,
            auth_commands::auth_restore_session,
            auth_commands::auth_logout,
            auth_log::auth_log_open_folder,
            social_commands::social_get_friends,
            social_commands::social_get_groups,
            social_commands::social_get_group_instances,
            social_commands::social_get_last_joined,
            social_commands::social_get_instances_of_group,
            social_commands::social_get_instance_detail,
            social_commands::social_set_pinned_friends,
            settings_store::settings_load_app,
            settings_store::settings_save_app,
            settings_store::settings_load_user,
            settings_store::settings_save_user,
            settings_store::settings_load_ui,
            settings_store::settings_save_ui,
            game_monitor::game_get_status,
            game_monitor::game_open_instance,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
