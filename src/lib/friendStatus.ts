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
export const canOpenFriendLocation = (friend: Friend) => friend.locationKind === "world" && friend.worldName !== null;

/** 一覧での並び順: インスタンス内 → 非公開・移動中・Web → オフライン */
export function friendRank(friend: Friend): number {
  if (friend.locationKind === "offline") return 2;
  if (friend.locationKind === "world") return 0;
  return 1;
}

export const compareFriends = (a: Friend, b: Friend) => friendRank(a) - friendRank(b) || a.name.localeCompare(b.name);
