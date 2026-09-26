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

/** public: Group Public / plus: Group+ / members: Group (メンバー限定) */
export type GroupAccessType = "public" | "plus" | "members";

export interface GroupInstance {
  /** `wrld_…:…` 形式の location */
  id: string;
  groupId: string;
  worldName: string;
  accessType: GroupAccessType;
  userCount: number;
  capacity: number;
  /** Polaris が初めて観測した時刻 (epoch ms)。API に作成時刻が無いため代用する */
  firstSeenAt: number;
  /** Created ソート用の値。大きいほど新しい (API のレスポンス順から算出) */
  createdOrder: number;
}

export const getFriends = () => invoke<Friend[]>("social_get_friends");
export const getGroups = () => invoke<Group[]>("social_get_groups");
export const getGroupInstances = () => invoke<GroupInstance[]>("social_get_group_instances");
export const setPinnedFriends = (ids: string[]) => invoke<void>("social_set_pinned_friends", { ids });

export const onFriendsUpdated = (handler: (friends: Friend[]) => void): Promise<UnlistenFn> =>
  listen<Friend[]>("social://friends-updated", (event) => handler(event.payload));
export const onGroupsUpdated = (handler: (groups: Group[]) => void): Promise<UnlistenFn> =>
  listen<Group[]>("social://groups-updated", (event) => handler(event.payload));
export const onSessionExpired = (handler: () => void): Promise<UnlistenFn> =>
  listen("social://session-expired", () => handler());
