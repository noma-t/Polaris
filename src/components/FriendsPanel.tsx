import { useEffect, useState, type KeyboardEvent } from "react";
import { canOpenFriendLocation, compareFriends, FRIEND_STATUS_META, friendLocationLabel } from "../lib/friendStatus";
import type { Friend } from "../lib/social";
import { FriendDetail } from "./FriendDetail";
import { FriendIcon, VisibilityListIcon } from "./icons";
import { OpenInstanceButton } from "./OpenInstanceButton";
import { OverlayScrollArea } from "./OverlayScrollArea";

interface FriendsPanelProps {
  friends: Friend[];
  pinnedIds: Record<string, boolean>;
  /** Open ボタンを無効化する理由。開ける状態なら null */
  openDisabledReason: string | null;
  onOpenInstance: (location: string, label: string) => void;
  onManage: () => void;
}

export function FriendsPanel({ friends, pinnedIds, openDisabledReason, onOpenInstance, onManage }: FriendsPanelProps) {
  const pinned = friends.filter((f) => pinnedIds[f.id]).sort(compareFriends);
  /** インライン展開中の friend。複数行を同時に開け、閉じるまで開いたままにする */
  const [expandedIds, setExpandedIds] = useState<Record<string, boolean>>({});

  // インスタンスを離れた (Offline / Private / Traveling など) friend は閉じ、World に戻っても再展開しない。
  // World 間の移動中 (World 名の取得待ち) は閉じない
  useEffect(() => {
    setExpandedIds((prev) => {
      const ids = Object.keys(prev).filter((id) => friends.find((f) => f.id === id)?.locationKind === "world");
      return ids.length === Object.keys(prev).length ? prev : Object.fromEntries(ids.map((id) => [id, true]));
    });
  }, [friends]);

  return (
    <>
      <div className="panel-header">
        <h2 className="panel-title">
          <FriendIcon size={26} className="panel-title-icon" />
          Friends
        </h2>
        <button className="panel-icon-button" onClick={onManage} title="Show / hide friends" aria-label="Show / hide friends">
          <VisibilityListIcon size={26} />
        </button>
      </div>

      <OverlayScrollArea viewportClassName="panel-body">
        {pinned.length === 0 && <div className="panel-empty">No pinned friends</div>}

        <div className="friend-list">
          {pinned.map((friend) => {
            const canExpand = canOpenFriendLocation(friend);
            const isExpanded = canExpand && !!expandedIds[friend.id];
            const toggleExpanded = () =>
              setExpandedIds((prev) => {
                const { [friend.id]: _, ...rest } = prev;
                return isExpanded ? rest : { ...prev, [friend.id]: true };
              });
            return (
              <div
                key={friend.id}
                className={`friend-card ${friend.locationKind === "offline" ? "is-offline" : ""} ${isExpanded ? "is-expanded" : ""}`}
              >
                <div
                  className={`friend-row ${canExpand ? "is-expandable" : ""}`}
                  {...(canExpand && {
                    role: "button",
                    tabIndex: 0,
                    "aria-expanded": isExpanded,
                    onClick: toggleExpanded,
                    onKeyDown: (e: KeyboardEvent) => {
                      if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
                      e.preventDefault();
                      toggleExpanded();
                    },
                  })}
                >
                  <div className="friend-info">
                    <span className="friend-name-line">
                      <span className="status-dot" style={{ background: FRIEND_STATUS_META[friend.status].color }} />
                      <span className="friend-name">{friend.name}</span>
                    </span>
                    {friend.isWorldLoading ? (
                      <span className="friend-location" aria-busy="true" aria-label="Loading world name">
                        <span className="friend-world is-loading" />
                      </span>
                    ) : canExpand ? (
                      <span className="friend-location">
                        <span className="friend-world">{friend.worldName}</span>
                      </span>
                    ) : (
                      <span className="friend-location-hidden">{friendLocationLabel(friend)}</span>
                    )}
                  </div>
                  {canExpand && (
                    <>
                      <span className={`friend-expand-chevron ${isExpanded ? "is-expanded" : ""}`} aria-hidden="true">
                        ▼
                      </span>
                      {/* Open の押下で行の展開・折りたたみが起きないようにする */}
                      <span className="friend-open-action" onClick={(e) => e.stopPropagation()}>
                        <OpenInstanceButton
                          disabledReason={openDisabledReason}
                          onClick={() => onOpenInstance(friend.location, friend.worldName)}
                        />
                      </span>
                    </>
                  )}
                </div>
                {isExpanded && <FriendDetail friend={friend} />}
              </div>
            );
          })}
        </div>
      </OverlayScrollArea>
    </>
  );
}
