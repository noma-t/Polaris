import { invoke } from "@tauri-apps/api/core";

/** Rust 側 `settings.json` に VRChat userId ごとに保存する表示設定 */
export interface UserSettings {
  pinnedFriendIds: string[];
  shownGroupIds: string[];
  /** 表示中の group は既定で展開する。折りたたんだものだけを記録する */
  collapsedGroupIds: string[];
}

export const loadUserSettings = (userId: string) => invoke<UserSettings>("settings_load_user", { userId });
export const saveUserSettings = (userId: string, settings: UserSettings) =>
  invoke<void>("settings_save_user", { userId, settings });
