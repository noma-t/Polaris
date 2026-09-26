import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type FriendStatus = "join" | "online" | "ask" | "busy" | "offline";
/** world: インスタンス内 / website: ゲーム外 (Web) でオンライン */
export type FriendLocationKind = "world" | "private" | "traveling" | "website" | "offline";

export interface Friend {
  id: string;
  name: string;
  status: FriendStatus;
  locationKind: FriendLocationKind;
  /** locationKind が world で、名前を取得済みのときのみ */
  worldName: string | null;
  /** pinned かつ World 名の取得待ち・取得中 */
  isWorldLoading: boolean;
}

export interface Group {
  id: string;
  name: string;
}

export const getFriends = () => invoke<Friend[]>("social_get_friends");
export const getGroups = () => invoke<Group[]>("social_get_groups");
export const setPinnedFriends = (ids: string[]) => invoke<void>("social_set_pinned_friends", { ids });

export const onFriendsUpdated = (handler: (friends: Friend[]) => void): Promise<UnlistenFn> =>
  listen<Friend[]>("social://friends-updated", (event) => handler(event.payload));
export const onGroupsUpdated = (handler: (groups: Group[]) => void): Promise<UnlistenFn> =>
  listen<Group[]>("social://groups-updated", (event) => handler(event.payload));
export const onSessionExpired = (handler: () => void): Promise<UnlistenFn> =>
  listen("social://session-expired", () => handler());
