//! アプリ共通の設定 (launch.exe のパス) と、ユーザーごとの表示設定
//! (pinned friend・表示する group・折りたたんだ group) を app data dir の `settings.json` に保存する。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::game_monitor::GameState;
use crate::vrchat_client::AuthError;

const SETTINGS_FILE_NAME: &str = "settings.json";
/// Steam 版 VRChat を既定の場所にインストールした場合の launch.exe
const DEFAULT_LAUNCHER_PATH: &str = "C:/Program Files (x86)/Steam/steamapps/common/VRChat/launch.exe";

/// VRChat アカウントに依らない、PC 単位の設定
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// インスタンスを開くのに使う VRChat の launch.exe
    pub launcher_path: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { launcher_path: DEFAULT_LAUNCHER_PATH.to_owned() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserSettings {
    pub pinned_friend_ids: Vec<String>,
    pub shown_group_ids: Vec<String>,
    /// 表示中の group は既定で展開する。ユーザーが折りたたんだものだけを記録する
    pub collapsed_group_ids: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SettingsFile {
    app: AppSettings,
    /// VRChat の userId ごとの設定
    users: HashMap<String, UserSettings>,
}

/// 読み込み・書き込みを直列化するためのロック
#[derive(Default)]
pub struct SettingsState(Mutex<()>);

impl SettingsState {
    pub fn new() -> Self {
        Self::default()
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, AuthError> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(SETTINGS_FILE_NAME))
        .map_err(|err| AuthError::Unexpected(format!("Could not resolve the app data directory: {err}")))
}

/// ファイルが無い・壊れている場合は空の設定として扱う
fn read_file(path: &PathBuf) -> SettingsFile {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 書き込み途中で落ちても元のファイルが壊れないよう、一時ファイルに書いてから置き換える
fn write_file(path: &PathBuf, file: &SettingsFile) -> Result<(), AuthError> {
    let to_error = |err: std::io::Error| AuthError::Unexpected(format!("Failed to save settings: {err}"));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(to_error)?;
    }
    let text = serde_json::to_string_pretty(file).map_err(|err| AuthError::Unexpected(err.to_string()))?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, text).map_err(to_error)?;
    fs::rename(&temp_path, path).map_err(to_error)
}

/// 起動時に保存済みの launch.exe パスを読み込む
pub fn load_app_settings(app: &AppHandle) -> AppSettings {
    let state = app.state::<SettingsState>();
    let _guard = state.0.lock().expect("settings lock poisoned");
    settings_path(app).map(|path| read_file(&path).app).unwrap_or_default()
}

#[tauri::command]
pub fn settings_load_app(app: AppHandle, state: State<'_, SettingsState>) -> Result<AppSettings, AuthError> {
    let _guard = state.0.lock().expect("settings lock poisoned");
    let path = settings_path(&app)?;
    Ok(read_file(&path).app)
}

/// 保存後、次の poll を待たずに launch.exe の存在確認を反映する
#[tauri::command]
pub fn settings_save_app(
    app: AppHandle,
    state: State<'_, SettingsState>,
    game: State<'_, GameState>,
    settings: AppSettings,
) -> Result<(), AuthError> {
    let _guard = state.0.lock().expect("settings lock poisoned");
    let path = settings_path(&app)?;
    let mut file = read_file(&path);
    let launcher_path = settings.launcher_path.clone();
    file.app = settings;
    write_file(&path, &file)?;
    game.set_launcher_path(&app, launcher_path);
    Ok(())
}

#[tauri::command]
pub fn settings_load_user(app: AppHandle, state: State<'_, SettingsState>, user_id: String) -> Result<UserSettings, AuthError> {
    let _guard = state.0.lock().expect("settings lock poisoned");
    let path = settings_path(&app)?;
    Ok(read_file(&path).users.remove(&user_id).unwrap_or_default())
}

#[tauri::command]
pub fn settings_save_user(
    app: AppHandle,
    state: State<'_, SettingsState>,
    user_id: String,
    settings: UserSettings,
) -> Result<(), AuthError> {
    let _guard = state.0.lock().expect("settings lock poisoned");
    let path = settings_path(&app)?;
    let mut file = read_file(&path);
    file.users.insert(user_id, settings);
    write_file(&path, &file)
}
