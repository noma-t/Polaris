import { useEffect, useRef, useState } from "react";
import {
  getFriends,
  getGroups,
  getLastJoined,
  onFriendsUpdated,
  onGroupsUpdated,
  onLastJoinedUpdated,
  onSessionExpired,
  setPinnedFriends,
  type Friend,
  type Group,
  type LastJoinedAt,
} from "../lib/social";

interface UseSocialOptions {
  isSignedIn: boolean;
  pinnedFriendIds: Record<string, boolean>;
  onSessionExpired: () => void;
}

/**
 * Rust 側が保持する Friends / Groups の snapshot を購読する。
 * listen を登録してから現在値を取得し、登録前の emit を取りこぼさないようにする。
 */
export function useSocial({ isSignedIn, pinnedFriendIds, onSessionExpired: handleSessionExpired }: UseSocialOptions) {
  const [friends, setFriends] = useState<Friend[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [lastJoinedAt, setLastJoinedAt] = useState<LastJoinedAt>({});
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const sessionExpiredRef = useRef(handleSessionExpired);
  sessionExpiredRef.current = handleSessionExpired;

  useEffect(() => {
    if (!isSignedIn) {
      setFriends([]);
      setGroups([]);
      setLastJoinedAt({});
      setUpdatedAt(null);
      return;
    }

    let isCancelled = false;
    const applyFriends = (next: Friend[]) => {
      if (isCancelled) return;
      setFriends(next);
      setUpdatedAt(Date.now());
    };
    const applyGroups = (next: Group[]) => {
      if (isCancelled) return;
      setGroups(next);
      setUpdatedAt(Date.now());
    };

    const applyLastJoined = (next: LastJoinedAt) => {
      if (isCancelled) return;
      setLastJoinedAt(next);
    };

    const subscriptions = Promise.all([
      onFriendsUpdated(applyFriends),
      onGroupsUpdated(applyGroups),
      onLastJoinedUpdated(applyLastJoined),
      onSessionExpired(() => !isCancelled && sessionExpiredRef.current()),
    ]);
    subscriptions
      .then(() => Promise.all([getFriends(), getGroups(), getLastJoined()]))
      .then(([initialFriends, initialGroups, initialLastJoined]) => {
        // 初期同期の完了前は空なので、emit 済みの値を空で上書きしない
        if (initialFriends.length) applyFriends(initialFriends);
        if (initialGroups.length) applyGroups(initialGroups);
        if (Object.keys(initialLastJoined).length) applyLastJoined(initialLastJoined);
      })
      .catch(() => {
        // 取得に失敗しても以降の emit で更新される
      });

    return () => {
      isCancelled = true;
      subscriptions.then((unlisteners) => unlisteners.forEach((unlisten) => unlisten()));
    };
  }, [isSignedIn]);

  useEffect(() => {
    if (!isSignedIn) return;
    const ids = Object.keys(pinnedFriendIds).filter((id) => pinnedFriendIds[id]);
    setPinnedFriends(ids).catch(() => {
      // 次回の変更時に再送される
    });
  }, [isSignedIn, pinnedFriendIds]);

  return { friends, groups, lastJoinedAt, updatedAt };
}
