//! VRChat のプロセスと launch.exe の存在を定期的に確認し、状態が変わったときだけフロントエンドへ emit する。
//! launch.exe 経由でインスタンスを開くコマンドもここに置く。

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, State};

use crate::vrchat_client::AuthError;

const GAME_PROCESS_NAME: &str = "VRChat.exe";
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const STATUS_CHANGED_EVENT: &str = "game://status-changed";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStatus {
    is_running: bool,
    is_launcher_found: bool,
}

#[derive(Default)]
struct GameInner {
    status: GameStatus,
    launcher_path: String,
}

#[derive(Clone, Default)]
pub struct GameState {
    inner: Arc<Mutex<GameInner>>,
}

impl GameState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 監視 task を起動する。app の生存期間中ずっと動き続ける。
    /// launch.exe は後から移動・削除されうるので、プロセスと同じ間隔で確認し続ける。
    pub fn start_monitor(&self, app: AppHandle, launcher_path: String) {
        self.inner.lock().expect("game state poisoned").launcher_path = launcher_path;
        let state = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut system = System::new();
            loop {
                let is_running = is_game_running(&mut system);
                state.update(&app, |status| status.is_running = is_running);
                state.refresh_launcher(&app);
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
    }

    /// 設定変更時。次の poll を待たずに存在確認して反映する
    pub fn set_launcher_path(&self, app: &AppHandle, launcher_path: String) {
        self.inner.lock().expect("game state poisoned").launcher_path = launcher_path;
        self.refresh_launcher(app);
    }

    fn refresh_launcher(&self, app: &AppHandle) {
        let path = self.inner.lock().expect("game state poisoned").launcher_path.clone();
        let is_found = Path::new(&path).is_file();
        self.update(app, |status| status.is_launcher_found = is_found);
    }

    fn update(&self, app: &AppHandle, apply: impl FnOnce(&mut GameStatus)) {
        let next = {
            let mut inner = self.inner.lock().expect("game state poisoned");
            let prev = inner.status;
            apply(&mut inner.status);
            (inner.status != prev).then_some(inner.status)
        };
        if let Some(status) = next {
            let _ = app.emit(STATUS_CHANGED_EVENT, status);
        }
    }
}

fn is_game_running(system: &mut System) -> bool {
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .values()
        .any(|process| process.name().eq_ignore_ascii_case(GAME_PROCESS_NAME))
}

/// `wrld_…:…` 形式で、URL や引数として解釈が変わる文字を含まない location か
pub(crate) fn is_valid_instance_location(location: &str) -> bool {
    let Some((world_id, instance_id)) = location.split_once(':') else {
        return false;
    };
    world_id.starts_with("wrld_")
        && !instance_id.is_empty()
        && location
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '~' | '(' | ')' | '.'))
}

#[tauri::command]
pub fn game_get_status(state: State<'_, GameState>) -> GameStatus {
    state.inner.lock().expect("game state poisoned").status
}

/// launch.exe に `vrchat://launch` URL を渡し、起動中の VRChat でインスタンスを開く
#[tauri::command]
pub fn game_open_instance(state: State<'_, GameState>, location: String) -> Result<(), AuthError> {
    if !is_valid_instance_location(&location) {
        return Err(AuthError::Unexpected(format!("Invalid instance location: {location}")));
    }
    let (is_running, launcher_path) = {
        let inner = state.inner.lock().expect("game state poisoned");
        (inner.status.is_running, inner.launcher_path.clone())
    };
    // 起動していない VRChat を新たに立ち上げることはしない
    if !is_running {
        return Err(AuthError::Unexpected("VRChat is not running".into()));
    }
    let launcher = Path::new(&launcher_path);
    if !launcher.is_file() {
        return Err(AuthError::Unexpected("launch.exe was not found".into()));
    }
    let mut command = Command::new(launcher);
    command.arg(format!("vrchat://launch?ref=vrchat.com&id={location}&attach=1"));
    if let Some(dir) = launcher.parent() {
        command.current_dir(dir);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| AuthError::Unexpected(format!("Failed to run launch.exe: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_instance_locations() {
        assert!(is_valid_instance_location("wrld_abc-123:12345~group(grp_1)~groupAccessType(public)~region(jp)"));
        assert!(is_valid_instance_location("wrld_abc:98765~private(usr_1)~canRequestInvite~region(us)"));
        assert!(!is_valid_instance_location("private"));
        assert!(!is_valid_instance_location("wrld_abc:"));
        assert!(!is_valid_instance_location("wrld_abc:1&attach=0"));
        assert!(!is_valid_instance_location("wrld_abc:1 --flag"));
    }
}
