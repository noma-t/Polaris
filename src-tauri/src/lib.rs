mod auth_commands;
mod credential_store;
mod image_protocol;
mod pipeline;
mod social;
mod social_commands;
mod vrchat_client;
mod vrchat_models;

use auth_commands::AuthState;
use social::SocialState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // tokio-tungstenite (rustls) は process 既定の CryptoProvider を使うため、先に ring を登録しておく
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AuthState::new())
        .manage(SocialState::new())
        .register_asynchronous_uri_scheme_protocol(image_protocol::SCHEME, image_protocol::handle)
        .invoke_handler(tauri::generate_handler![
            auth_commands::auth_login,
            auth_commands::auth_verify_two_factor,
            auth_commands::auth_restore_session,
            auth_commands::auth_logout,
            social_commands::social_get_friends,
            social_commands::social_get_groups,
            social_commands::social_set_pinned_friends,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
