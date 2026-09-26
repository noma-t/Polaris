import { canOpenFriendLocation, compareFriends, FRIEND_STATUS_META, friendLocationLabel } from "../lib/friendStatus";
import type { Friend } from "../lib/social";
import { FriendIcon, VisibilityListIcon } from "./icons";

interface FriendsPanelProps {
  friends: Friend[];
  pinnedIds: Record<string, boolean>;
  onOpenInstance: (label: string) => void;
  onManage: () => void;
}

export function FriendsPanel({ friends, pinnedIds, onOpenInstance, onManage }: FriendsPanelProps) {
  const pinned = friends.filter((f) => pinnedIds[f.id]).sort(compareFriends);

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

      {pinned.length === 0 && <div className="panel-empty">No pinned friends</div>}

      <div className="friend-list">
        {pinned.map((friend) => {
          const canOpen = canOpenFriendLocation(friend);
          return (
            <div key={friend.id} className={`friend-row ${friend.locationKind === "offline" ? "is-offline" : ""}`}>
              <div className="friend-info">
                <span className="friend-name-line">
                  <span className="status-dot" style={{ background: FRIEND_STATUS_META[friend.status].color }} />
                  <span className="friend-name">{friend.name}</span>
                </span>
                {friend.isWorldLoading ? (
                  <span className="friend-location" aria-busy="true" aria-label="Loading world name">
                    <span className="friend-world is-loading" />
                  </span>
                ) : canOpen ? (
                  <span className="friend-location">
                    <span className="friend-world">{friend.worldName}</span>
                  </span>
                ) : (
                  <span className="friend-location-hidden">{friendLocationLabel(friend)}</span>
                )}
              </div>
              {canOpen && (
                <button className="btn-accent open-button" onClick={() => onOpenInstance(friend.worldName ?? friend.name)}>
                  Open
                </button>
              )}
            </div>
          );
        })}
      </div>
    </>
  );
}
