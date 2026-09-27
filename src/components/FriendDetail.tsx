import { useInstanceDetail } from "../hooks/useInstanceDetail";
import { toVrchatImageSrc } from "../lib/auth";
import { INSTANCE_TYPE_LABELS, type Friend } from "../lib/social";
import { RefreshIcon, UsersIcon } from "./icons";

interface FriendDetailProps {
  friend: Friend & { location: string; worldName: string };
}

/** Friends 欄の行をインライン展開したときの詳細 (ステータスメッセージと、いるインスタンスの情報) */
export function FriendDetail({ friend }: FriendDetailProps) {
  const { detail, error, isLoading, isCoolingDown, refresh } = useInstanceDetail(friend.location);
  const isRefreshLocked = isLoading || isCoolingDown;
  const isFull = detail !== null && detail.capacity > 0 && detail.userCount >= detail.capacity;

  return (
    <div className="friend-detail">
      {friend.statusMessage && <p className="friend-status-message">{friend.statusMessage}</p>}

      <div className="friend-instance">
        <div className="friend-instance-info">
          <div className="friend-instance-header">
            <span className="friend-instance-label">Instance</span>
            <button
              className="instance-refresh-button"
              onClick={refresh}
              disabled={isRefreshLocked}
              aria-label="Refresh instance"
              title="Refresh"
            >
              <RefreshIcon size={15} className={isLoading ? "icon-spin" : undefined} />
              {isCoolingDown && (
                <svg className="instance-refresh-cooldown" viewBox="0 0 24 24" aria-hidden="true">
                  <circle cx="12" cy="12" r="10.8" />
                </svg>
              )}
            </button>
          </div>
          <span className="friend-instance-world">{detail?.worldName || friend.worldName}</span>
          {detail ? (
            <>
              <span className="friend-instance-type">
                {INSTANCE_TYPE_LABELS[detail.instanceType]}
                {detail.hostName && ` (${detail.hostName})`}
              </span>
              <span className={`friend-instance-user-count ${isFull ? "is-full" : ""}`}>
                <UsersIcon size={18} className="friend-instance-user-icon" />
                {detail.userCount}/{detail.capacity}
              </span>
            </>
          ) : error ? (
            <span className="friend-instance-error">{error}</span>
          ) : (
            <span className="friend-instance-skeleton" aria-busy="true" aria-label="Loading instance">
              <span className="skeleton-line" />
              <span className="skeleton-line is-short" />
            </span>
          )}
          {detail && error && <span className="friend-instance-error">{error}</span>}
        </div>
        <div className="friend-instance-thumbnail">
          {detail?.thumbnailUrl && <img src={toVrchatImageSrc(detail.thumbnailUrl)} alt="" loading="lazy" />}
        </div>
      </div>
    </div>
  );
}
