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

/** Rust 側 `settings.json` に保存する、VRChat アカウントに依らない設定 */
export interface AppSettings {
  /** インスタンスを開くのに使う VRChat の launch.exe */
  launcherPath: string;
  /** Settings 画面に Developer 向けの設定項目を表示する */
  developerMode: boolean;
  /** 通信せずにダミーの update を表示する (developerMode が有効な場合のみ効く) */
  simulateUpdateAvailable: boolean;
}

export const DEFAULT_LAUNCHER_PATH = "C:/Program Files (x86)/Steam/steamapps/common/VRChat/launch.exe";

export const DEFAULT_APP_SETTINGS: AppSettings = {
  launcherPath: DEFAULT_LAUNCHER_PATH,
  developerMode: false,
  simulateUpdateAvailable: false,
};

export const loadAppSettings = () => invoke<AppSettings>("settings_load_app");
export const saveAppSettings = (settings: AppSettings) => invoke<void>("settings_save_app", { settings });
