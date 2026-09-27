import type { Friend, FriendStatus } from "./social";

export const FRIEND_STATUS_META: Record<FriendStatus, { color: string; label: string }> = {
  join: { color: "oklch(0.72 0.13 245)", label: "Join Me" },
  online: { color: "oklch(0.76 0.15 150)", label: "Online" },
  ask: { color: "oklch(0.79 0.14 65)", label: "Ask Me" },
  busy: { color: "oklch(0.66 0.19 25)", label: "Do Not Disturb" },
  offline: { color: "oklch(0.5 0.01 250)", label: "Offline" },
};

/** 現在地の表示文字列 (オフライン / 非公開 / 移動中 / Web / ワールド名) */
export function friendLocationLabel(friend: Friend): string {
  switch (friend.locationKind) {
    case "offline":
      return "Offline";
    case "private":
      return "Private";
    case "traveling":
      return "Traveling";
    case "website":
      return "Website";
    case "world":
      return friend.worldName ?? "In world";
  }
}

/** Open できる (World 名が判明しているインスタンス内にいる) か */
export const canOpenFriendLocation = (friend: Friend): friend is Friend & { location: string; worldName: string } =>
  friend.locationKind === "world" && friend.location !== null && friend.worldName !== null;

/**
 * 一覧での並び順:
 * インスタンス内 → 非公開・移動中 (Join Me / Online → Ask Me → Do Not Disturb) → Web → オフライン
 * (同じ順位の中はステータス順 → 名前順。compareFriends を参照)
 */
export function friendRank(friend: Friend): number {
  switch (friend.locationKind) {
    case "world":
      return 0;
    case "private":
    case "traveling":
      if (friend.status === "ask") return 2;
      if (friend.status === "busy") return 3;
      return 1;
    case "website":
      return 4;
    case "offline":
      return 5;
  }
}

/** 同じ friendRank 内での並び順: Join Me → Online → Ask Me → Do Not Disturb → Offline */
const STATUS_ORDER: Record<FriendStatus, number> = { join: 0, online: 1, ask: 2, busy: 3, offline: 4 };

export const compareFriends = (a: Friend, b: Friend) =>
  friendRank(a) - friendRank(b) || STATUS_ORDER[a.status] - STATUS_ORDER[b.status] || a.name.localeCompare(b.name);
