import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import { OverlayScrollArea } from "./components/OverlayScrollArea";
import { PolarisLogo } from "./components/PolarisLogo";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Toast } from "./components/Toast";
import { VisibilityDialog, type VisibilityKind } from "./components/VisibilityDialog";
import type { NavScreen, Screen } from "./components/navigation";
import { MOCK_RATE_LIMITED } from "./data/mock";
import { isDummyLocation, THUMBNAIL_FRIENDS, THUMBNAIL_PINNED_IDS } from "./data/thumbnailFriends";
import type { VsuiGroup } from "./data/vsuiGroups";
import { useAppSettings } from "./hooks/useAppSettings";
import { useAppVersion } from "./hooks/useAppVersion";
import { useElementWidth } from "./hooks/useElementWidth";
import { useGameStatus } from "./hooks/useGameStatus";
import { useGroupInstances, type SortState } from "./hooks/useGroupInstances";
import { useSocial } from "./hooks/useSocial";
import { useUserSettings } from "./hooks/useUserSettings";
import { useToast } from "./hooks/useToast";
import { useUiState } from "./hooks/useUiState";
import { useUpdater } from "./hooks/useUpdater";
import { useWindowDiagnostics } from "./hooks/useWindowDiagnostics";
import { logout, restoreSession, type CurrentUser, type SignOutReason } from "./lib/auth";
import { diagnosticLog } from "./lib/diagnosticLog";
import { formatClock } from "./lib/format";
import { openInstanceDisabledReason, openInstanceInGame } from "./lib/game";
import { AboutScreen } from "./screens/AboutScreen";
import { InstancesScreen, MIN_GROUPS_WIDTH } from "./screens/InstancesScreen";
import { LoginScreen } from "./screens/LoginScreen";
import { RecommendScreen } from "./screens/RecommendScreen";
import { SettingsScreen } from "./screens/SettingsScreen";

const RATE_LIMIT_RETRY_MS = 240_000;
/** Settings を開いた状態で、この時間内に Settings をこの回数クリックすると Developer mode の設定項目を表示する */
const DEVELOPER_MODE_REVEAL_CLICKS = 5;
const DEVELOPER_MODE_REVEAL_WINDOW_MS = 1000;
const SIDEBAR_OPEN_WIDTH = 256;
/** main 領域の左右 padding 分の余白 */
const MAIN_GUTTER = 44;
/** この幅未満では Sidebar を rail 表示に固定する */
const SIDEBAR_EXPANDABLE_MIN_WIDTH = SIDEBAR_OPEN_WIDTH + MAIN_GUTTER + MIN_GROUPS_WIDTH;

export default function App() {
  const rootRef = useRef<HTMLDivElement>(null);
  const layoutWidth = useElementWidth(rootRef, 1280);

  /** 起動時に保存済みセッションを確認している間は true */
  const [isRestoringSession, setIsRestoringSession] = useState(true);
  const [currentUser, setCurrentUser] = useState<CurrentUser | null>(null);
  const [visibilityDialog, setVisibilityDialog] = useState<VisibilityKind | null>(null);
  /** 隠しコマンドで Developer mode の設定項目を表示したか (保存しない) */
  const [isDeveloperModeRevealed, setIsDeveloperModeRevealed] = useState(false);
  /** Settings を開いた状態で Settings をクリックした時刻 */
  const settingsClickTimesRef = useRef<number[]>([]);
  const { uiState, isUiStateLoaded, updateUiState } = useUiState();
  /** サインイン中は前回開いていた画面を復元する */
  const screen: Screen = currentUser ? uiState.activeScreen : "login";
  const {
    pinnedFriendIds,
    shownGroupIds,
    collapsedGroupIds,
    togglePinnedFriend,
    toggleShownGroup,
    toggleGroupCollapsed,
  } = useUserSettings(currentUser?.id ?? null);
  const { toast, showToast, hideToast } = useToast();
  const appVersion = useAppVersion((version) => showToast(`Updated to version ${version}`));
  const { appSettings, updateAppSettings } = useAppSettings();
  const updater = useUpdater({
    isReady: appSettings !== null,
    simulate: (appSettings?.developerMode && appSettings.simulateUpdateAvailable) ?? false,
    onSimulatedInstallFinished: () => showToast("Simulated update finished. Nothing was installed."),
  });
  const { friends, groups, lastJoinedAt, updatedAt } = useSocial({
    isSignedIn: currentUser !== null,
    pinnedFriendIds,
    onSessionExpired: () => {
      showToast("Session expired. Please sign in again.", "error");
      void signOut("sessionExpired");
    },
  });
  /** Thumbnail mode 中は実 Friends を隠し、ダミーだけを表示する (Rust 側へ送る pinned や保存される設定は実データのまま) */
  const isThumbnailMode = (appSettings?.developerMode && appSettings.thumbnailMode) ?? false;
  /** Thumbnail mode 中の pin 状態。保存せず、Thumbnail mode に入るたびに初期値へ戻す */
  const [thumbnailPinnedIds, setThumbnailPinnedIds] = useState(THUMBNAIL_PINNED_IDS);
  useEffect(() => {
    if (isThumbnailMode) setThumbnailPinnedIds(THUMBNAIL_PINNED_IDS);
  }, [isThumbnailMode]);
  const toggleThumbnailPinnedFriend = useCallback(
    (id: string) => setThumbnailPinnedIds((prev) => ({ ...prev, [id]: !prev[id] })),
    [],
  );
  const displayedFriends = isThumbnailMode ? THUMBNAIL_FRIENDS : friends;
  const displayedPinnedFriendIds = isThumbnailMode ? thumbnailPinnedIds : pinnedFriendIds;
  const onToggleDisplayedFriend = isThumbnailMode ? toggleThumbnailPinnedFriend : togglePinnedFriend;
  const changeSort = useCallback((sort: SortState) => updateUiState({ sort }), [updateUiState]);
  const hasOpenGroup = screen === "instances" && groups.some((g) => shownGroupIds[g.id] && !collapsedGroupIds[g.id]);
  useWindowDiagnostics();
  // 自動更新の有無は screen と展開状態で決まるので、変化を残す (Instances 以外の画面にいる間は自動更新が止まる)
  useEffect(() => {
    diagnosticLog(`ui: screen=${screen} hasOpenGroup=${hasOpenGroup} groups=${groups.length}`);
  }, [screen, hasOpenGroup, groups.length]);
  const groupState = useGroupInstances({
    isSignedIn: currentUser !== null,
    hasOpenGroup,
    sort: uiState.sort,
    onSortChange: changeSort,
    onError: (message) => showToast(message, "error"),
  });
  const lastUpdatedAt = Math.max(updatedAt ?? 0, groupState.updatedAt ?? 0) || null;
  const gameStatus = useGameStatus();
  const openDisabledReason = openInstanceDisabledReason(gameStatus);

  useEffect(() => {
    let isCancelled = false;
    restoreSession()
      .then((user) => {
        if (user && !isCancelled) setCurrentUser(user);
      })
      .catch(() => {
        // 復元に失敗した場合 (オフライン等) はログイン画面から始める
      })
      .finally(() => {
        if (!isCancelled) setIsRestoringSession(false);
      });
    return () => {
      isCancelled = true;
    };
  }, []);

  const signOut = async (reason: SignOutReason) => {
    try {
      await logout(reason);
    } catch {
      showToast("Failed to clear the saved session", "error");
    }
    groupState.reset();
    setVisibilityDialog(null);
    setCurrentUser(null);
  };

  /** Developer mode がオンの間は、オフに戻せるよう常に表示する */
  const isDeveloperModeVisible = isDeveloperModeRevealed || (appSettings?.developerMode ?? false);
  useEffect(() => {
    if (appSettings?.developerMode) setIsDeveloperModeRevealed(true);
  }, [appSettings?.developerMode]);

  const countSettingsClick = () => {
    const now = Date.now();
    const recent = [...settingsClickTimesRef.current, now].filter((t) => now - t < DEVELOPER_MODE_REVEAL_WINDOW_MS);
    settingsClickTimesRef.current = recent;
    if (recent.length < DEVELOPER_MODE_REVEAL_CLICKS || isDeveloperModeVisible) return;
    settingsClickTimesRef.current = [];
    setIsDeveloperModeRevealed(true);
    showToast("Developer mode is now available");
  };

  const navigate = (next: NavScreen) => {
    if (next === "settings" && screen === "settings") countSettingsClick();
    else settingsClickTimesRef.current = [];
    setVisibilityDialog(null);
    updateUiState({ activeScreen: next });
  };

  const closeVisibilityDialog = useCallback(() => setVisibilityDialog(null), []);
  const openInVRChat = (location: string, label: string) => {
    if (isDummyLocation(location)) return showToast(`“${label}” is a dummy instance (Thumbnail mode)`);
    openInstanceInGame(location)
      .then(() => showToast(`Opened “${label}” in VRChat`))
      .catch(() => showToast(`Failed to open “${label}” in VRChat`, "error"));
  };
  const openGroupPageInBrowser = (group: VsuiGroup) => {
    openUrl(`https://vrchat.com/home/group/${group.groupId}`).catch(() => showToast(`Failed to open “${group.name}” in browser`, "error"));
  };

  return (
    <div ref={rootRef} className={`app-root ${screen === "login" ? "" : "is-signed-in"}`}>
      {isRestoringSession || !isUiStateLoaded ? (
        <div className="app-splash" aria-busy="true" aria-label="Restoring session">
          <PolarisLogo className="app-splash-logo" />
        </div>
      ) : screen === "login" ? (
        <LoginScreen onSignedIn={setCurrentUser} onError={(message) => showToast(message, "error")} />
      ) : (
        <div className="app-shell">
          <div className="app-body">
            <Sidebar
              activeScreen={screen}
              username={currentUser?.displayName ?? ""}
              userIconUrl={currentUser?.iconUrl ?? null}
              isOpenPreferred={uiState.isSidebarOpen}
              onOpenPreferredChange={(isSidebarOpen) => updateUiState({ isSidebarOpen })}
              canExpand={layoutWidth >= SIDEBAR_EXPANDABLE_MIN_WIDTH}
              badgedScreens={updater.isUpdateAvailable ? ["settings"] : []}
              onNavigate={navigate}
              onSignOut={() => void signOut("user")}
            />

            <main className="main-scroll">
              <OverlayScrollArea>
                <div className="main-content">
                  {MOCK_RATE_LIMITED && screen === "instances" && (
                    <div className="rate-limit-banner" role="alert">
                      <span className="rate-limit-title">Rate limited</span>
                      <span className="rate-limit-detail">Retrying around {formatClock(Date.now() + RATE_LIMIT_RETRY_MS)}</span>
                    </div>
                  )}

                  {screen === "instances" && (
                    <InstancesScreen
                      friends={displayedFriends}
                      groups={groups}
                      groupState={groupState}
                      lastJoinedAt={lastJoinedAt}
                      isLastJoinedSimulated={(appSettings?.developerMode && appSettings.simulateLastJoined) ?? false}
                      pinnedFriendIds={displayedPinnedFriendIds}
                      shownGroupIds={shownGroupIds}
                      collapsedGroupIds={collapsedGroupIds}
                      onToggleGroupCollapsed={toggleGroupCollapsed}
                      openDisabledReason={openDisabledReason}
                      onOpenInstance={openInVRChat}
                      onManageFriends={() => setVisibilityDialog("friends")}
                      onManageGroups={() => setVisibilityDialog("groups")}
                      groupsShare={uiState.groupsShare}
                      collapsedPanel={uiState.collapsedPanel}
                      onSplitChange={updateUiState}
                    />
                  )}
                  {screen === "recommend" && (
                    <RecommendScreen onOpenGroupPage={openGroupPageInBrowser} />
                  )}
                  {screen === "settings" && (
                    <SettingsScreen
                      version={appVersion}
                      appSettings={appSettings}
                      onUpdateAppSettings={updateAppSettings}
                      updater={updater}
                      isDeveloperModeVisible={isDeveloperModeVisible}
                      isLauncherFound={gameStatus.isLauncherFound}
                      onError={(message) => showToast(message, "error")}
                    />
                  )}
                  {screen === "info" && <AboutScreen version={appVersion} />}
                </div>
              </OverlayScrollArea>
            </main>
          </div>
          <StatusBar updatedAt={lastUpdatedAt} isGameRunning={gameStatus.isRunning} />
        </div>
      )}

      {screen === "instances" && visibilityDialog && (
        <VisibilityDialog
          kind={visibilityDialog}
          friends={displayedFriends}
          groups={groups}
          pinnedFriendIds={displayedPinnedFriendIds}
          shownGroupIds={shownGroupIds}
          onToggleFriend={onToggleDisplayedFriend}
          onToggleGroup={toggleShownGroup}
          onClose={closeVisibilityDialog}
        />
      )}
      <Toast toast={toast} onClose={hideToast} />
    </div>
  );
}
