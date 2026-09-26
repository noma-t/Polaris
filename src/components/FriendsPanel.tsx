import { MOCK_FRIENDS } from "../data/mock";
import { FRIEND_STATUS_META, friendLocationLabel, friendRank } from "../lib/friendStatus";
import { FriendIcon, VisibilityListIcon } from "./icons";

interface FriendsPanelProps {
  pinnedIds: Record<string, boolean>;
  onOpenInstance: (label: string) => void;
  onManage: () => void;
}

export function FriendsPanel({ pinnedIds, onOpenInstance, onManage }: FriendsPanelProps) {
  const pinned = MOCK_FRIENDS.filter((f) => pinnedIds[f.id]).sort(
    (a, b) => friendRank(a) - friendRank(b) || (b.userCount ?? 0) - (a.userCount ?? 0),
  );

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
          const canOpen = !friend.isOffline && !friend.isPrivate;
          return (
            <div key={friend.id} className={`friend-row ${friend.isOffline ? "is-offline" : ""}`}>
              <div className="friend-info">
                <span className="friend-name-line">
                  <span className="status-dot" style={{ background: FRIEND_STATUS_META[friend.status].color }} />
                  <span className="friend-name">{friend.name}</span>
                </span>
                {canOpen ? (
                  <span className="friend-location">
                    <span className="friend-world">{friend.world}</span>
                    <span className="friend-user-count">
                      ({friend.userCount}/{friend.capacity})
                    </span>
                  </span>
                ) : (
                  <span className="friend-location-hidden">{friendLocationLabel(friend)}</span>
                )}
              </div>
              {canOpen && (
                <button className="btn-accent open-button" onClick={() => onOpenInstance(friend.world ?? friend.name)}>
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
