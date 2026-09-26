//! Friends / Groups の snapshot 取得と pinned friend 設定の Tauri コマンド。

use tauri::{AppHandle, Emitter, State};

use crate::social::{FriendView, GroupView, SocialState, FRIENDS_UPDATED_EVENT};

/// listen 開始前に emit された分を取りこぼさないよう、初期表示時に現在の snapshot を返す
#[tauri::command]
pub fn social_get_friends(state: State<'_, SocialState>) -> Vec<FriendView> {
    state.friend_views()
}

#[tauri::command]
pub fn social_get_groups(state: State<'_, SocialState>) -> Vec<GroupView> {
    state.group_views()
}

/// pinned friend を置き換える。World 名の取得対象と `isWorldLoading` が変わるので snapshot を送り直す
#[tauri::command]
pub fn social_set_pinned_friends(app: AppHandle, state: State<'_, SocialState>, ids: Vec<String>) {
    let views = state.set_pinned(ids);
    let _ = app.emit(FRIENDS_UPDATED_EVENT, views);
}
