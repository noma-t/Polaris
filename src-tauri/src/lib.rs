mod auth_commands;
mod credential_store;
mod game_monitor;
mod image_protocol;
mod pipeline;
mod settings_store;
mod social;
mod social_commands;
mod vrchat_client;
mod vrchat_models;

use auth_commands::AuthState;
use game_monitor::GameState;
use settings_store::SettingsState;
use social::SocialState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // tokio-tungstenite (rustls) は process 既定の CryptoProvider を使うため、先に ring を登録しておく
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AuthState::new())
        .manage(SocialState::new())
        .manage(SettingsState::new())
        .manage(GameState::new())
        .setup(|app| {
            let launcher_path = settings_store::load_app_settings(app.handle()).launcher_path;
            app.state::<GameState>().start_monitor(app.handle().clone(), launcher_path);
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol(image_protocol::SCHEME, image_protocol::handle)
        .invoke_handler(tauri::generate_handler![
            auth_commands::auth_login,
            auth_commands::auth_verify_two_factor,
            auth_commands::auth_restore_session,
            auth_commands::auth_logout,
            social_commands::social_get_friends,
            social_commands::social_get_groups,
            social_commands::social_get_group_instances,
            social_commands::social_set_pinned_friends,
            settings_store::settings_load_app,
            settings_store::settings_save_app,
            settings_store::settings_load_user,
            settings_store::settings_save_user,
            game_monitor::game_get_status,
            game_monitor::game_open_instance,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
