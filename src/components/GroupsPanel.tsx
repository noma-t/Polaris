import { useRef, type CSSProperties } from "react";
import { useElementHeight } from "../hooks/useElementHeight";
import { REFRESH_COOLDOWN_SEC, type GroupInstancesState, type SortKey } from "../hooks/useGroupInstances";
import { useNow } from "../hooks/useNow";
import { formatClock } from "../lib/format";
import type { Group, GroupAccessType, GroupInstance } from "../lib/social";
import { ChevronDownIcon, GroupsIcon, VisibilityListIcon } from "./icons";
import { OpenInstanceButton } from "./OpenInstanceButton";
import { OverlayScrollArea } from "./OverlayScrollArea";

interface GroupsPanelProps {
  groups: Group[];
  groupState: GroupInstancesState;
  shownGroupIds: Record<string, boolean>;
  collapsedGroupIds: Record<string, boolean>;
  onToggleCollapsed: (groupId: string) => void;
  /** Open ボタンを無効化する理由。開ける状態なら null */
  openDisabledReason: string | null;
  onOpenInstance: (location: string, label: string) => void;
  onManage: () => void;
}

const SORT_OPTIONS: { key: SortKey; label: string }[] = [
  { key: "users", label: "Users" },
  { key: "created", label: "Created" },
];

const ACCESS_TYPE_LABELS: Record<GroupAccessType, string> = {
  public: "Group Public",
  plus: "Group+",
  members: "Group",
};

export function GroupsPanel({
  groups,
  groupState,
  shownGroupIds,
  collapsedGroupIds,
  onToggleCollapsed,
  openDisabledReason,
  onOpenInstance,
  onManage,
}: GroupsPanelProps) {
  const { sort, cooldownUntil, instancesByGroup, isLoading, hasLoadFailed, refreshGroup, selectSort } = groupState;
  const isCoolingCandidate = cooldownUntil > Date.now();
  const now = useNow(100, isCoolingCandidate);
  const cooldownMs = Math.min(REFRESH_COOLDOWN_SEC * 1000, Math.max(0, cooldownUntil - now));
  const isCooling = cooldownMs > 0;
  const shownGroups = groups.filter((g) => shownGroupIds[g.id]);
  const headerRef = useRef<HTMLDivElement>(null);
  // 1 カラム時は panel-header も sticky なので、group-card-header はその直下に貼り付ける
  const headerHeight = useElementHeight(headerRef, 68);

  const compareInstances = (a: GroupInstance, b: GroupInstance) =>
    sort.key === "users"
      ? sort.usersDir === "desc"
        ? b.userCount - a.userCount
        : a.userCount - b.userCount
      : sort.createdDir === "new"
        ? b.createdOrder - a.createdOrder
        : a.createdOrder - b.createdOrder;

  return (
    <>
      <div ref={headerRef} className="panel-header groups-panel-header">
        <h2 className="panel-title">
          <GroupsIcon size={26} className="panel-title-icon" />
          Groups
        </h2>
        <div className="panel-header-actions">
          <div className="sort-toggle" role="group" aria-label="Sort">
            {SORT_OPTIONS.map(({ key, label }) => {
              const isActive = sort.key === key;
              // Users は多い順、Created は古い順のときに下向き矢印を出す (Created は上向きで新しいものが上)
              const isArrowDown = key === "users" ? sort.usersDir === "desc" : sort.createdDir === "old";
              return (
                <button
                  key={key}
                  className={`sort-toggle-option ${isActive ? "is-active" : ""}`}
                  onClick={() => selectSort(key)}
                  aria-pressed={isActive}
                >
                  <span className="sort-toggle-label">{label}</span>
                  <ChevronDownIcon size={20} className="sort-toggle-direction" style={{ transform: isArrowDown ? "none" : "rotate(180deg)" }} />
                </button>
              );
            })}
          </div>
          <button className="panel-icon-button" onClick={onManage} title="Show / hide groups" aria-label="Show / hide groups">
            <VisibilityListIcon size={26} />
          </button>
        </div>
      </div>

      <OverlayScrollArea viewportClassName="panel-body">
        {shownGroups.length === 0 && <div className="panel-empty">No groups shown</div>}

        <div className="group-list" style={{ "--groups-panel-header-height": `${headerHeight}px` } as CSSProperties}>
          {shownGroups.map((group) => {
            const isOpen = !collapsedGroupIds[group.id];
            const instances = instancesByGroup?.[group.id] ?? [];
            const count = instances.length;
            const rows = isOpen ? instances.slice().sort(compareInstances) : [];
            return (
              <div key={group.id} className={`group-card ${isOpen ? "is-open" : ""}`}>
                <div className="group-card-header">
                  <button className="group-toggle" onClick={() => onToggleCollapsed(group.id)} aria-expanded={isOpen}>
                    <span className="group-toggle-chevron">{isOpen ? "▼" : "▶"}</span>
                    <span className="group-name">{group.name}</span>
                    {instancesByGroup && <span className={`group-instance-count ${count ? "" : "is-zero"}`}>{count}</span>}
                  </button>
                  {isOpen && (
                    <button className="group-refresh-button" onClick={() => refreshGroup(group.id)} disabled={isCooling}>
                      <span>Refresh</span>
                      <span
                        className="cooldown-progress"
                        style={{ width: `${(cooldownMs / (REFRESH_COOLDOWN_SEC * 1000)) * 100}%` }}
                      />
                    </button>
                  )}
                </div>
                {isOpen && (
                  <div className="group-instance-list">
                    {!instancesByGroup && (
                      <div className="group-list-message">{hasLoadFailed && !isLoading ? "Failed to load instances" : "Loading…"}</div>
                    )}
                    {instancesByGroup && count === 0 && <div className="group-list-message">No instances</div>}
                    {rows.map((instance) => (
                      <div key={instance.id} className="instance-row">
                        <div className="instance-info">
                          <div className="instance-primary">
                            <span className="instance-world">{instance.worldName}</span>
                            <span className={`instance-user-count ${instance.userCount >= instance.capacity ? "is-full" : ""}`}>
                              ({instance.userCount}/{instance.capacity})
                            </span>
                          </div>
                          <div className="instance-secondary">
                            <span className="instance-access-type">{ACCESS_TYPE_LABELS[instance.accessType]}</span>
                            <span className="instance-created" title="First seen by Polaris">
                              {formatClock(instance.firstSeenAt)}
                            </span>
                          </div>
                        </div>
                        <OpenInstanceButton
                          disabledReason={openDisabledReason}
                          onClick={() => onOpenInstance(instance.id, instance.worldName)}
                        />
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </OverlayScrollArea>
    </>
  );
}
