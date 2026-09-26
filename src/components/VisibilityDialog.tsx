import { useEffect, useState } from "react";
import { compareFriends, FRIEND_STATUS_META, friendLocationLabel } from "../lib/friendStatus";
import type { Friend, Group } from "../lib/social";
import { CheckIcon, CloseIcon } from "./icons";

export type VisibilityKind = "friends" | "groups";

interface VisibilityDialogProps {
  kind: VisibilityKind;
  friends: Friend[];
  groups: Group[];
  pinnedFriendIds: Record<string, boolean>;
  shownGroupIds: Record<string, boolean>;
  onToggleFriend: (friendId: string) => void;
  onToggleGroup: (groupId: string) => void;
  onClose: () => void;
}

type VisibilityTab = "all" | "shown";

interface VisibilityRow {
  id: string;
  name: string;
  status: string;
  dotColor?: string;
  isOn: boolean;
  onToggle: () => void;
}

/** Instances 画面の Friends / Groups に表示する項目を選ぶ modal dialog */
export function VisibilityDialog({
  kind,
  friends,
  groups,
  pinnedFriendIds,
  shownGroupIds,
  onToggleFriend,
  onToggleGroup,
  onClose,
}: VisibilityDialogProps) {
  const [tab, setTab] = useState<VisibilityTab>("all");
  const isGroups = kind === "groups";
  const title = isGroups ? "Show / hide groups" : "Show / hide friends";

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const allRows: VisibilityRow[] = isGroups
    ? groups.map((g) => ({
        id: g.id,
        name: g.name,
        status: "",
        isOn: !!shownGroupIds[g.id],
        onToggle: () => onToggleGroup(g.id),
      }))
    : [...friends].sort(compareFriends).map((f) => ({
        id: f.id,
        name: f.name,
        status: friendLocationLabel(f),
        dotColor: FRIEND_STATUS_META[f.status].color,
        isOn: !!pinnedFriendIds[f.id],
        onToggle: () => onToggleFriend(f.id),
      }));
  const shownRows = allRows.filter((r) => r.isOn);
  const rows = tab === "shown" ? shownRows : allRows;
  const tabs: { key: VisibilityTab; label: string; count: number }[] = [
    { key: "all", label: "All", count: allRows.length },
    { key: "shown", label: isGroups ? "Shown" : "Pinned", count: shownRows.length },
  ];

  return (
    <div className="dialog-layer">
      <div className="dialog-backdrop" onClick={onClose} />
      <div className="dialog visibility-dialog" role="dialog" aria-modal="true" aria-label={title}>
        <div className="dialog-header">
          <div className="dialog-title-row">
            <h2 className="dialog-title">{title}</h2>
            <button className="dialog-close-button" onClick={onClose} aria-label="Close" title="Close">
              <CloseIcon size={22} />
            </button>
          </div>
          <div className="segmented-control visibility-tabs" role="tablist">
            {tabs.map((t) => (
              <button
                key={t.key}
                role="tab"
                aria-selected={tab === t.key}
                className={`segmented-control-option ${tab === t.key ? "is-active" : ""}`}
                onClick={() => setTab(t.key)}
              >
                {t.label} <span className="visibility-tab-count">{t.count}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="dialog-body visibility-list">
          {rows.map((row) => (
            <button key={row.id} className="visibility-row" role="checkbox" aria-checked={row.isOn} onClick={row.onToggle}>
              <span className={`checkbox-box ${row.isOn ? "is-checked" : ""}`}>
                {row.isOn && <CheckIcon size={18} strokeWidth={2.8} />}
              </span>
              <span className="visibility-row-info">
                <span className="visibility-row-name">{row.name}</span>
                {(row.dotColor || row.status) && (
                  <span className="visibility-row-status">
                    {row.dotColor && <span className="status-dot is-small" style={{ background: row.dotColor }} />}
                    <span className="visibility-row-status-text">{row.status}</span>
                  </span>
                )}
              </span>
            </button>
          ))}
        </div>

        <div className="dialog-footer">
          <button className="btn-accent dialog-done-button" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
