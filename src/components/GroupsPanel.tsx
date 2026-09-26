import { REFRESH_COOLDOWN_SEC, type GroupInstancesState, type SortKey } from "../hooks/useGroupInstances";
import { useNow } from "../hooks/useNow";
import type { Group } from "../lib/social";
import { ChevronDownIcon, GroupsIcon, VisibilityListIcon } from "./icons";
import { OverlayScrollArea } from "./OverlayScrollArea";

interface GroupsPanelProps {
  groups: Group[];
  groupState: GroupInstancesState;
  shownGroupIds: Record<string, boolean>;
  collapsedGroupIds: Record<string, boolean>;
  onToggleCollapsed: (groupId: string) => void;
  onManage: () => void;
}

const SORT_OPTIONS: { key: SortKey; label: string }[] = [
  { key: "users", label: "Users" },
  { key: "created", label: "Created" },
];

export function GroupsPanel({
  groups,
  groupState,
  shownGroupIds,
  collapsedGroupIds,
  onToggleCollapsed,
  onManage,
}: GroupsPanelProps) {
  const { sort, cooldownUntil, refreshGroup, selectSort } = groupState;
  const isCoolingCandidate = cooldownUntil > Date.now();
  const now = useNow(100, isCoolingCandidate);
  const cooldownMs = Math.min(REFRESH_COOLDOWN_SEC * 1000, Math.max(0, cooldownUntil - now));
  const isCooling = cooldownMs > 0;
  const shownGroups = groups.filter((g) => shownGroupIds[g.id]);

  return (
    <>
      <div className="panel-header groups-panel-header">
        <h2 className="panel-title">
          <GroupsIcon size={26} className="panel-title-icon" />
          Groups
        </h2>
        <div className="panel-header-actions">
          <div className="sort-toggle" role="group" aria-label="Sort">
            {SORT_OPTIONS.map(({ key, label }) => {
              const isActive = sort.key === key;
              const isDescending = key === "users" ? sort.usersDir === "desc" : sort.createdDir === "new";
              return (
                <button
                  key={key}
                  className={`sort-toggle-option ${isActive ? "is-active" : ""}`}
                  onClick={() => selectSort(key)}
                  aria-pressed={isActive}
                >
                  <span className="sort-toggle-label">{label}</span>
                  <ChevronDownIcon size={20} className="sort-toggle-direction" style={{ transform: isDescending ? "none" : "rotate(180deg)" }} />
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

        <div className="group-list">
          {shownGroups.map((group) => {
            const isOpen = !collapsedGroupIds[group.id];
            return (
              <div key={group.id} className={`group-card ${isOpen ? "is-open" : ""}`}>
                <div className="group-card-header">
                  <button className="group-toggle" onClick={() => onToggleCollapsed(group.id)} aria-expanded={isOpen}>
                    <span className="group-toggle-chevron">{isOpen ? "▼" : "▶"}</span>
                    <span className="group-name">{group.name}</span>
                  </button>
                  {isOpen && (
                    <button className="group-refresh-button" onClick={refreshGroup} disabled={isCooling}>
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
                    {/* インスタンス取得は未実装 */}
                    <div className="group-list-message">No instances</div>
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
