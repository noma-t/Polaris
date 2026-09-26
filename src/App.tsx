import { useCallback, useRef, useState } from "react";
import { OverlayScrollArea } from "./components/OverlayScrollArea";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Toast } from "./components/Toast";
import { VisibilityDialog, type VisibilityKind } from "./components/VisibilityDialog";
import type { Screen } from "./components/navigation";
import { INITIAL_PINNED_FRIENDS, MOCK_RATE_LIMITED } from "./data/mock";
import { useElementWidth } from "./hooks/useElementWidth";
import { useGroupInstances } from "./hooks/useGroupInstances";
import { useToast } from "./hooks/useToast";
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
  const [username, setUsername] = useState("");
  const [pinnedFriendIds, setPinnedFriendIds] = useState(INITIAL_PINNED_FRIENDS);
  const [hiddenGroupIds, setHiddenGroupIds] = useState<Record<string, boolean>>({});
  const [visibilityDialog, setVisibilityDialog] = useState<VisibilityKind | null>(null);
  const groupState = useGroupInstances({ rateLimited: MOCK_RATE_LIMITED });
  const { toast, showToast, hideToast } = useToast();

  const signOut = () => {
    groupState.reset();
    setScreen("login");
  };

  const navigate = (next: Screen) => {
    setVisibilityDialog(null);
    setScreen(next);
  };

  const closeVisibilityDialog = useCallback(() => setVisibilityDialog(null), []);
  const openInVRChat = (label: string) => showToast(`Opened “${label}” in VRChat`);

  return (
    <div ref={rootRef} className={`app-root ${screen === "login" ? "" : "is-signed-in"}`}>
      {screen === "login" ? (
        <LoginScreen
          onSignedIn={(name) => {
            setUsername(name);
            setScreen("instances");
          }}
        />
      ) : (
        <div className="app-shell">
          <div className="app-body">
            <Sidebar
              activeScreen={screen}
              username={username}
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
                      groupState={groupState}
                      pinnedFriendIds={pinnedFriendIds}
                      hiddenGroupIds={hiddenGroupIds}
                      onOpenInstance={openInVRChat}
                      onManageFriends={() => setVisibilityDialog("friends")}
                      onManageGroups={() => setVisibilityDialog("groups")}
                    />
                  )}
                  {screen === "recommend" && (
                    <RecommendScreen onOpenGroupPage={(name) => showToast(`Opened “${name}” group page in VRChat`)} />
                  )}
                  {screen === "settings" && <SettingsScreen version={APP_VERSION} />}
                  {screen === "info" && <AboutScreen version={APP_VERSION} />}
                </div>
              </OverlayScrollArea>
            </main>
          </div>
          <StatusBar updatedAt={groupState.updatedAt} />
        </div>
      )}

      {screen === "instances" && visibilityDialog && (
        <VisibilityDialog
          kind={visibilityDialog}
          pinnedFriendIds={pinnedFriendIds}
          hiddenGroupIds={hiddenGroupIds}
          onToggleFriend={(id) => setPinnedFriendIds((s) => ({ ...s, [id]: !s[id] }))}
          onToggleGroup={(id) => setHiddenGroupIds((s) => ({ ...s, [id]: !s[id] }))}
          onClose={closeVisibilityDialog}
        />
      )}
      <Toast toast={toast} onClose={hideToast} />
    </div>
  );
}
