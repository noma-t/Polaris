import { invoke } from "@tauri-apps/api/core";
import { NAV_SCREENS, type NavScreen } from "../components/navigation";
import type { SortState } from "../hooks/useGroupInstances";

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
  /** サムネイル撮影用に、実 Friends を隠してダミーの Friends を表示する (developerMode が有効な場合のみ効く) */
  thumbnailMode: boolean;
  /** グループインスタンスの「最後に入ってからの経過時間」を実記録の代わりに見本の値で表示する (developerMode が有効な場合のみ効く) */
  simulateLastJoined: boolean;
  /** VRChat API へのリクエストと、認証・バックグラウンド取得・自動更新・ウィンドウ状態などの診断情報を `auth.log` に記録する (developerMode が有効な場合のみ効く) */
  authLogging: boolean;
  /** × でシステムトレイに格納し、ウィンドウを閉じている間もグループインスタンスの観測を続ける */
  runInBackground: boolean;
  /** OS へのログイン時に Polaris を起動する */
  launchAtStartup: boolean;
}

export const DEFAULT_LAUNCHER_PATH = "C:/Program Files (x86)/Steam/steamapps/common/VRChat/launch.exe";

export const DEFAULT_APP_SETTINGS: AppSettings = {
  launcherPath: DEFAULT_LAUNCHER_PATH,
  developerMode: false,
  simulateUpdateAvailable: false,
  thumbnailMode: false,
  simulateLastJoined: false,
  authLogging: false,
  runInBackground: false,
  launchAtStartup: false,
};

export const loadAppSettings = () => invoke<AppSettings>("settings_load_app");
export const saveAppSettings = (settings: AppSettings) => invoke<void>("settings_save_app", { settings });

/** Rust 側 `settings.json` に保存する、次回起動時に復元する PC 単位の UI 状態 */
export interface UiState {
  activeScreen: NavScreen;
  /** Sidebar を展開表示したいか (幅不足で rail 表示に固定される場合も保持する) */
  isSidebarOpen: boolean;
  /** Instances 画面で Groups 欄が占める割合 (%) */
  groupsShare: number;
  /** 折りたたんでいる欄。どちらも表示中なら null */
  collapsedPanel: CollapsedPanel | null;
  sort: SortState;
}

export type CollapsedPanel = "groups" | "friends";

export const DEFAULT_GROUPS_SHARE = 58;

export const DEFAULT_UI_STATE: UiState = {
  activeScreen: "instances",
  isSidebarOpen: true,
  groupsShare: DEFAULT_GROUPS_SHARE,
  collapsedPanel: null,
  sort: { key: "users", usersDir: "desc", createdDir: "new" },
};

const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
  allowed.includes(value as T) ? (value as T) : fallback;

/** 手編集や旧バージョンで想定外の値が入っていても、既定値に置き換えて使えるようにする */
const normalizeUiState = (raw: Partial<UiState>): UiState => {
  const d = DEFAULT_UI_STATE;
  const sort: Partial<SortState> = raw.sort ?? {};
  const groupsShare = raw.groupsShare;
  return {
    activeScreen: pick(raw.activeScreen, NAV_SCREENS, d.activeScreen),
    isSidebarOpen: typeof raw.isSidebarOpen === "boolean" ? raw.isSidebarOpen : d.isSidebarOpen,
    groupsShare: typeof groupsShare === "number" && groupsShare > 0 && groupsShare < 100 ? groupsShare : d.groupsShare,
    collapsedPanel: raw.collapsedPanel === "groups" || raw.collapsedPanel === "friends" ? raw.collapsedPanel : null,
    sort: {
      key: pick(sort.key, ["users", "created"], d.sort.key),
      usersDir: pick(sort.usersDir, ["desc", "asc"], d.sort.usersDir),
      createdDir: pick(sort.createdDir, ["new", "old"], d.sort.createdDir),
    },
  };
};

export const loadUiState = () => invoke<Partial<UiState>>("settings_load_ui").then(normalizeUiState);
export const saveUiState = (ui: UiState) => invoke<void>("settings_save_ui", { ui });
