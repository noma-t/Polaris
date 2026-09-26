import { useCallback, useEffect, useLayoutEffect, useRef, useState, type PointerEvent } from "react";
import { FriendsPanel } from "../components/FriendsPanel";
import { GroupsPanel } from "../components/GroupsPanel";
import { FriendIcon, GroupsIcon } from "../components/icons";
import { useElementWidth } from "../hooks/useElementWidth";
import type { GroupInstancesState } from "../hooks/useGroupInstances";

export const MIN_GROUPS_WIDTH = 410;
const MIN_FRIENDS_WIDTH = 240;
/** 最小幅のこの割合を超えて押し込むと、その欄を折りたたむ */
const COLLAPSE_SNAP = 2 / 3;
const COLLAPSED_STRIP_WIDTH = 36;
const SPLIT_HANDLE_WIDTH = 10;
const DEFAULT_GROUPS_SHARE = 58;
/** grid 幅がこれ以上なら Groups / Friends を 2 カラムで並べる */
const TWO_COLUMN_MIN_WIDTH = MIN_GROUPS_WIDTH + SPLIT_HANDLE_WIDTH + MIN_FRIENDS_WIDTH;

interface InstancesScreenProps {
  groupState: GroupInstancesState;
  pinnedFriendIds: Record<string, boolean>;
  hiddenGroupIds: Record<string, boolean>;
  onOpenInstance: (label: string) => void;
  onManageFriends: () => void;
  onManageGroups: () => void;
}

export function InstancesScreen({
  groupState,
  pinnedFriendIds,
  hiddenGroupIds,
  onOpenInstance,
  onManageFriends,
  onManageGroups,
}: InstancesScreenProps) {
  const gridRef = useRef<HTMLDivElement>(null);
  const gridWidth = useElementWidth(gridRef, 1280);
  const [groupsShare, setGroupsShare] = useState(DEFAULT_GROUPS_SHARE);
  const [collapsedPanel, setCollapsedPanel] = useState<"groups" | "friends" | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [gripHeight, setGripHeight] = useState<number | null>(null);

  const isTwoColumn = gridWidth >= TWO_COLUMN_MIN_WIDTH;
  const isGroupsCollapsed = isTwoColumn && collapsedPanel === "groups";
  const isFriendsCollapsed = isTwoColumn && collapsedPanel === "friends";

  // 分割ハンドルのグリップを、スクロールコンテナ内で見えている範囲の高さに合わせる
  const measureGrip = useCallback(() => {
    const grid = gridRef.current;
    if (!grid) return;
    let scroller = grid.parentElement;
    while (scroller && !/auto|scroll/.test(getComputedStyle(scroller).overflowY)) scroller = scroller.parentElement;
    if (!scroller) return;
    const gr = grid.getBoundingClientRect();
    const sr = scroller.getBoundingClientRect();
    const visible = Math.max(0, Math.round(Math.min(gr.bottom, sr.bottom) - Math.max(gr.top, sr.top)));
    setGripHeight((h) => (h === visible ? h : visible));
  }, []);

  useLayoutEffect(measureGrip);
  useEffect(() => {
    const grid = gridRef.current;
    if (!grid) return;
    const observer = new ResizeObserver(() => requestAnimationFrame(measureGrip));
    observer.observe(grid);
    document.addEventListener("scroll", measureGrip, true);
    return () => {
      observer.disconnect();
      document.removeEventListener("scroll", measureGrip, true);
    };
  }, [measureGrip]);

  const onSplitPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    const grid = gridRef.current;
    if (!grid) return;
    e.preventDefault();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);

    const onMove = (ev: globalThis.PointerEvent) => {
      const rect = grid.getBoundingClientRect();
      const width = grid.offsetWidth - SPLIT_HANDLE_WIDTH;
      const x = ev.clientX - rect.left;
      const lo = Math.min(50, (MIN_GROUPS_WIDTH / width) * 100);
      const hi = Math.max(50, 100 - (MIN_FRIENDS_WIDTH / width) * 100);
      if (x < MIN_GROUPS_WIDTH * (1 - COLLAPSE_SNAP)) return setCollapsedPanel("groups");
      if (x > width - MIN_FRIENDS_WIDTH * (1 - COLLAPSE_SNAP)) return setCollapsedPanel("friends");
      setCollapsedPanel(null);
      setGroupsShare(Math.round(Math.min(hi, Math.max(lo, (x / width) * 100)) * 10) / 10);
    };
    const onUp = () => {
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      handle.removeEventListener("pointercancel", onUp);
      setIsDragging(false);
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
    handle.addEventListener("pointercancel", onUp);
    setIsDragging(true);
  };

  const gridTemplateColumns = !isTwoColumn
    ? "minmax(0,1fr)"
    : isGroupsCollapsed
      ? `${COLLAPSED_STRIP_WIDTH}px ${SPLIT_HANDLE_WIDTH}px minmax(0,1fr)`
      : isFriendsCollapsed
        ? `minmax(0,1fr) ${SPLIT_HANDLE_WIDTH}px ${COLLAPSED_STRIP_WIDTH}px`
        : `minmax(${MIN_GROUPS_WIDTH}px,${groupsShare}fr) ${SPLIT_HANDLE_WIDTH}px minmax(${MIN_FRIENDS_WIDTH}px,${100 - groupsShare}fr)`;

  return (
    <div className="instances-screen">
      <div ref={gridRef} className={`instances-grid ${isTwoColumn ? "is-two-column" : ""}`} style={{ gridTemplateColumns }}>
        <section className={`instances-panel groups-panel ${isGroupsCollapsed ? "is-collapsed" : ""}`}>
          {isGroupsCollapsed ? (
            <button className="panel-expand-button" onClick={() => setCollapsedPanel(null)} title="Show Groups">
              <GroupsIcon size={26} className="panel-title-icon" />
            </button>
          ) : (
            <GroupsPanel
              groupState={groupState}
              hiddenGroupIds={hiddenGroupIds}
              onOpenInstance={onOpenInstance}
              onManage={onManageGroups}
            />
          )}
        </section>

        {isTwoColumn && (
          <div
            className={`split-handle ${isDragging ? "is-dragging" : ""}`}
            onPointerDown={onSplitPointerDown}
            onDoubleClick={() => setGroupsShare(DEFAULT_GROUPS_SHARE)}
            title="Drag to resize"
          >
            <div className="split-grip" style={{ height: gripHeight != null ? Math.max(0, gripHeight - 16) : "calc(100% - 16px)" }} />
          </div>
        )}

        <section className={`instances-panel friends-panel ${isFriendsCollapsed ? "is-collapsed" : ""}`}>
          {isFriendsCollapsed ? (
            <button className="panel-expand-button" onClick={() => setCollapsedPanel(null)} title="Show Friends">
              <FriendIcon size={26} className="panel-title-icon" />
            </button>
          ) : (
            <FriendsPanel pinnedIds={pinnedFriendIds} onOpenInstance={onOpenInstance} onManage={onManageFriends} />
          )}
        </section>
      </div>
    </div>
  );
}
