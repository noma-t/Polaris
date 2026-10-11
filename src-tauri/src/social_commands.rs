//! Friends / Groups / グループインスタンスの snapshot 取得・グループインスタンスの再取得と、pinned friend・展開中のグループ設定の Tauri コマンド。

use std::collections::HashMap;

use tauri::{AppHandle, Emitter, State};

use crate::social::{FriendView, GroupInstancesView, GroupView, InstanceDetailView, SocialState, FRIENDS_UPDATED_EVENT};
use crate::vrchat_client::AuthError;

/// listen 開始前に emit された分を取りこぼさないよう、初期表示時に現在の snapshot を返す
#[tauri::command]
pub fn social_get_friends(state: State<'_, SocialState>) -> Vec<FriendView> {
    state.friend_views()
}

#[tauri::command]
pub fn social_get_groups(state: State<'_, SocialState>) -> Vec<GroupView> {
    state.group_views()
}

/// 自分が最後にグループインスタンスに入った時刻 (location → epoch ms) の現在の snapshot を返す
#[tauri::command]
pub fn social_get_last_joined(state: State<'_, SocialState>) -> HashMap<String, i64> {
    state.last_joined_views()
}

/// グループインスタンス一覧の現在の snapshot を返す
#[tauri::command]
pub fn social_get_group_instances(state: State<'_, SocialState>) -> GroupInstancesView {
    state.group_instances_view()
}

/// 押されたグループのインスタンスを VRChat API から再取得する。結果は snapshot の emit で届く
#[tauri::command]
pub async fn social_refresh_group(state: State<'_, SocialState>, group_id: String) -> Result<(), AuthError> {
    state.refresh_group(&group_id).await
}

/// フロントエンドで表示中かつ展開中のグループを伝える。グループインスタンスの定期取得をするかの判断に使う
#[tauri::command]
pub fn social_set_open_groups(state: State<'_, SocialState>, group_ids: Vec<String>) {
    state.set_open_groups(group_ids);
}

/// フレンドがいるインスタンスの詳細を VRChat API から取得する
#[tauri::command]
pub async fn social_get_instance_detail(
    state: State<'_, SocialState>,
    location: String,
) -> Result<InstanceDetailView, AuthError> {
    state.fetch_instance_detail(&location).await
}

/// pinned friend を置き換える。World 名の取得対象と `isWorldLoading` が変わるので snapshot を送り直す
#[tauri::command]
pub fn social_set_pinned_friends(app: AppHandle, state: State<'_, SocialState>, ids: Vec<String>) {
    let views = state.set_pinned(ids);
    let _ = app.emit(FRIENDS_UPDATED_EVENT, views);
}
