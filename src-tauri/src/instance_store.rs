//! グループインスタンスを初めて観測した時刻と Created 順を、VRChat の userId ごとに app data dir の `instances.json` に保存する。
//! アプリを終了しても、開いたままのインスタンスの Created 表示とソート順を引き継ぐために使う。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::settings_store::write_json;
use crate::social::SeenInstance;

const INSTANCES_FILE_NAME: &str = "instances.json";

/// 読み込み・書き込みを直列化するためのロック
static FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct UserInstances {
    /// グループインスタンスの location → 観測済みの値
    seen: HashMap<String, SeenInstance>,
    /// 次に初めて見るグループインスタンスに振る Created ソート用の値
    next_created_order: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct InstancesFile {
    /// VRChat の userId ごとの記録
    users: HashMap<String, UserInstances>,
}

fn instances_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join(INSTANCES_FILE_NAME))
}

/// ファイルが無い・壊れている場合は空として扱う
fn read_file(path: &PathBuf) -> InstancesFile {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// 保存値より大きい Created 順が記録に含まれていても、次に初めて見るインスタンスが既存のどれよりも新しくなるようにする
fn restore(user: UserInstances) -> (HashMap<String, SeenInstance>, u32) {
    let max_order = user.seen.values().map(|seen| seen.created_order).max().unwrap_or(0);
    let next_created_order = user.next_created_order.max(max_order);
    (user.seen, next_created_order)
}

/// 保存済みの観測記録を読み込む
pub fn load(app: &AppHandle, user_id: &str) -> (HashMap<String, SeenInstance>, u32) {
    let _guard = FILE_LOCK.lock().expect("instances lock poisoned");
    let user = instances_path(app).and_then(|path| read_file(&path).users.remove(user_id)).unwrap_or_default();
    restore(user)
}

/// 観測記録を保存する。失敗しても表示は続けられるのでログだけ残す
pub fn save(app: &AppHandle, user_id: &str, seen: HashMap<String, SeenInstance>, next_created_order: u32) {
    let _guard = FILE_LOCK.lock().expect("instances lock poisoned");
    let Some(path) = instances_path(app) else {
        return eprintln!("[instance-store] could not resolve the app data directory");
    };
    let mut file = read_file(&path);
    file.users.insert(user_id.to_owned(), UserInstances { seen, next_created_order });
    if let Err(err) = write_json(&path, &file) {
        eprintln!("[instance-store] failed to save: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_file_format() {
        let json = r#"{"users":{"usr_1":{"seen":{"wrld_a:1":{"firstSeenAt":100,"createdOrder":3}},"nextCreatedOrder":3}}}"#;
        let mut file: InstancesFile = serde_json::from_str(json).unwrap();
        let user = file.users.remove("usr_1").unwrap();
        assert_eq!(user.seen["wrld_a:1"], SeenInstance { first_seen_at: 100, created_order: 3 });
        assert_eq!(user.next_created_order, 3);

        let text = serde_json::to_string(&UserInstances { seen: user.seen, next_created_order: 3 }).unwrap();
        assert!(text.contains(r#""firstSeenAt":100"#) && text.contains(r#""nextCreatedOrder":3"#));
    }

    #[test]
    fn restores_next_created_order_above_recorded_orders() {
        let seen = HashMap::from([("wrld_a:1".to_owned(), SeenInstance { first_seen_at: 100, created_order: 7 })]);
        let (_, next) = restore(UserInstances { seen, next_created_order: 2 });
        assert_eq!(next, 7);
    }
}
