//! VRChat のプロセスを定期的に走査し、起動状態が変わったときだけフロントエンドへ emit する。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, State};

const GAME_PROCESS_NAME: &str = "VRChat.exe";
const POLL_INTERVAL: Duration = Duration::from_secs(3);
const RUNNING_CHANGED_EVENT: &str = "game://running-changed";

#[derive(Clone, Default)]
pub struct GameState {
    is_running: Arc<AtomicBool>,
}

impl GameState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 監視 task を起動する。app の生存期間中ずっと動き続ける。
    pub fn start_monitor(&self, app: AppHandle) {
        let is_running = self.is_running.clone();
        tauri::async_runtime::spawn(async move {
            let mut system = System::new();
            loop {
                let next = is_game_running(&mut system);
                if is_running.swap(next, Ordering::Relaxed) != next {
                    let _ = app.emit(RUNNING_CHANGED_EVENT, next);
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
    }
}

fn is_game_running(system: &mut System) -> bool {
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .values()
        .any(|process| process.name().eq_ignore_ascii_case(GAME_PROCESS_NAME))
}

#[tauri::command]
pub fn game_is_running(state: State<'_, GameState>) -> bool {
    state.is_running.load(Ordering::Relaxed)
}
