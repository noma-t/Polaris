import type { Friend, FriendStatus } from "../data/mock";

export const FRIEND_STATUS_META: Record<FriendStatus, { color: string; label: string }> = {
  join: { color: "oklch(0.72 0.13 245)", label: "Join Me" },
  online: { color: "oklch(0.76 0.15 150)", label: "Online" },
  ask: { color: "oklch(0.79 0.14 65)", label: "Ask Me" },
  busy: { color: "oklch(0.66 0.19 25)", label: "Do Not Disturb" },
  offline: { color: "oklch(0.5 0.01 250)", label: "Offline" },
};

/** 現在地の表示文字列 (オフライン / 非公開 / ワールド名) */
export function friendLocationLabel(friend: Friend): string {
  if (friend.isOffline) return "Offline";
  if (friend.isPrivate) return "Private";
  return friend.world ?? "";
}

/** 一覧での並び順: 公開インスタンス → 非公開 → オフライン */
export function friendRank(friend: Friend): number {
  if (friend.isOffline) return 2;
  if (friend.isPrivate) return 1;
  return 0;
}
