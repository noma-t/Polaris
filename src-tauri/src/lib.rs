mod auth_commands;
mod credential_store;
mod image_protocol;
mod vrchat_client;

use auth_commands::AuthState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AuthState::new())
        .register_asynchronous_uri_scheme_protocol(image_protocol::SCHEME, image_protocol::handle)
        .invoke_handler(tauri::generate_handler![
            auth_commands::auth_login,
            auth_commands::auth_verify_two_factor,
            auth_commands::auth_restore_session,
            auth_commands::auth_logout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
