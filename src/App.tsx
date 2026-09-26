import { openUrl } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useRef, useState } from "react";
import { OverlayScrollArea } from "./components/OverlayScrollArea";
import { PolarisLogo } from "./components/PolarisLogo";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Toast } from "./components/Toast";
import { VisibilityDialog, type VisibilityKind } from "./components/VisibilityDialog";
import type { Screen } from "./components/navigation";
import { MOCK_RATE_LIMITED } from "./data/mock";
import type { VsuiGroup } from "./data/vsuiGroups";
import { useElementWidth } from "./hooks/useElementWidth";
import { useGroupInstances } from "./hooks/useGroupInstances";
import { useSocial } from "./hooks/useSocial";
import { useUserSettings } from "./hooks/useUserSettings";
import { useToast } from "./hooks/useToast";
import { logout, restoreSession, type CurrentUser } from "./lib/auth";
import { formatClock } from "./lib/format";
import { AboutScreen } from "./screens/AboutScreen";
import { InstancesScreen, MIN_GROUPS_WIDTH } from "./screens/InstancesScreen";
import { LoginScreen } from "./screens/LoginScreen";
import { RecommendScreen } from "./screens/RecommendScreen";
import { SettingsScreen } from "./screens/SettingsScreen";

const APP_VERSION = "0.1.0";
const RATE_LIMIT_RETRY_MS = 240_000;
const SIDEBAR_OPEN_WIDTH = 256;
/** main 領域の左右 padding 分の余白 */
const MAIN_GUTTER = 44;
/** この幅未満では Sidebar を rail 表示に固定する */
const SIDEBAR_EXPANDABLE_MIN_WIDTH = SIDEBAR_OPEN_WIDTH + MAIN_GUTTER + MIN_GROUPS_WIDTH;

export default function App() {
  const rootRef = useRef<HTMLDivElement>(null);
  const layoutWidth = useElementWidth(rootRef, 1280);

  const [screen, setScreen] = useState<Screen>("login");
  /** 起動時に保存済みセッションを確認している間は true */
  const [isRestoringSession, setIsRestoringSession] = useState(true);
  const [currentUser, setCurrentUser] = useState<CurrentUser | null>(null);
  const [visibilityDialog, setVisibilityDialog] = useState<VisibilityKind | null>(null);
  const {
    pinnedFriendIds,
    shownGroupIds,
    collapsedGroupIds,
    togglePinnedFriend,
    toggleShownGroup,
    toggleGroupCollapsed,
  } = useUserSettings(currentUser?.id ?? null);
  const { toast, showToast, hideToast } = useToast();
  const { friends, groups, updatedAt } = useSocial({
    isSignedIn: currentUser !== null,
    pinnedFriendIds,
    onSessionExpired: () => {
      showToast("Session expired. Please sign in again.", "error");
      void signOut();
    },
  });
  const hasOpenGroup = screen === "instances" && groups.some((g) => shownGroupIds[g.id] && !collapsedGroupIds[g.id]);
  const groupState = useGroupInstances({
    isSignedIn: currentUser !== null,
    hasOpenGroup,
    onError: (message) => showToast(message, "error"),
  });
  const lastUpdatedAt = Math.max(updatedAt ?? 0, groupState.updatedAt ?? 0) || null;

  const completeSignIn = (user: CurrentUser) => {
    setCurrentUser(user);
    setScreen("instances");
  };

  useEffect(() => {
    let isCancelled = false;
    restoreSession()
      .then((user) => {
        if (user && !isCancelled) completeSignIn(user);
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

  const signOut = async () => {
    try {
      await logout();
    } catch {
      showToast("Failed to clear the saved session", "error");
    }
    groupState.reset();
    setVisibilityDialog(null);
    setCurrentUser(null);
    setScreen("login");
  };

  const navigate = (next: Screen) => {
    setVisibilityDialog(null);
    setScreen(next);
  };

  const closeVisibilityDialog = useCallback(() => setVisibilityDialog(null), []);
  const openInVRChat = (label: string) => showToast(`Opened “${label}” in VRChat`);
  const openGroupPageInBrowser = (group: VsuiGroup) => {
    openUrl(`https://vrchat.com/home/group/${group.groupId}`).catch(() => showToast(`Failed to open “${group.name}” in browser`, "error"));
  };

  return (
    <div ref={rootRef} className={`app-root ${screen === "login" ? "" : "is-signed-in"}`}>
      {isRestoringSession ? (
        <div className="app-splash" aria-busy="true" aria-label="Restoring session">
          <PolarisLogo className="app-splash-logo" />
        </div>
      ) : screen === "login" ? (
        <LoginScreen onSignedIn={completeSignIn} onError={(message) => showToast(message, "error")} />
      ) : (
        <div className="app-shell">
          <div className="app-body">
            <Sidebar
              activeScreen={screen}
              username={currentUser?.displayName ?? ""}
              userIconUrl={currentUser?.iconUrl ?? null}
              canExpand={layoutWidth >= SIDEBAR_EXPANDABLE_MIN_WIDTH}
              onNavigate={navigate}
              onSignOut={signOut}
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
                      friends={friends}
                      groups={groups}
                      groupState={groupState}
                      pinnedFriendIds={pinnedFriendIds}
                      shownGroupIds={shownGroupIds}
                      collapsedGroupIds={collapsedGroupIds}
                      onToggleGroupCollapsed={toggleGroupCollapsed}
                      onOpenInstance={openInVRChat}
                      onManageFriends={() => setVisibilityDialog("friends")}
                      onManageGroups={() => setVisibilityDialog("groups")}
                    />
                  )}
                  {screen === "recommend" && (
                    <RecommendScreen onOpenGroupPage={openGroupPageInBrowser} />
                  )}
                  {screen === "settings" && <SettingsScreen version={APP_VERSION} />}
                  {screen === "info" && <AboutScreen version={APP_VERSION} />}
                </div>
              </OverlayScrollArea>
            </main>
          </div>
          <StatusBar updatedAt={lastUpdatedAt} />
        </div>
      )}

      {screen === "instances" && visibilityDialog && (
        <VisibilityDialog
          kind={visibilityDialog}
          friends={friends}
          groups={groups}
          pinnedFriendIds={pinnedFriendIds}
          shownGroupIds={shownGroupIds}
          onToggleFriend={togglePinnedFriend}
          onToggleGroup={toggleShownGroup}
          onClose={closeVisibilityDialog}
        />
      )}
      <Toast toast={toast} onClose={hideToast} />
    </div>
  );
}
