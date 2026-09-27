import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type FriendStatus = "join" | "online" | "ask" | "busy" | "offline";
/** world: インスタンス内 / website: ゲーム外 (Web) でオンライン */
export type FriendLocationKind = "world" | "private" | "traveling" | "website" | "offline";

export interface Friend {
  id: string;
  name: string;
  status: FriendStatus;
  /** ユーザーが設定しているステータスメッセージ。未設定なら空文字 */
  statusMessage: string;
  locationKind: FriendLocationKind;
  /** locationKind が world のときのみ。`wrld_…:…` 形式 */
  location: string | null;
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
  /** Created ソート用の値。大きいほど新しい (初めて観測したときに振り、以降の取得では変わらない) */
  createdOrder: number;
}

export interface GroupInstanceList {
  /** VRChat 側でこの一覧が取得された時刻 (epoch ms)。レスポンスから読めなければ Polaris が受け取った時刻 */
  fetchedAt: number;
  instances: GroupInstance[];
}

export type InstanceType = "public" | "friendsPlus" | "friends" | "invitePlus" | "invite" | "groupPublic" | "groupPlus" | "group";

export const INSTANCE_TYPE_LABELS: Record<InstanceType, string> = {
  public: "Public",
  friendsPlus: "Friends+",
  friends: "Friends",
  invitePlus: "Invite+",
  invite: "Invite",
  groupPublic: "Group Public",
  groupPlus: "Group+",
  group: "Group",
};

/** フレンドがいるインスタンス 1 件分の詳細 */
export interface InstanceDetail {
  /** `wrld_…:…` 形式の location */
  location: string;
  worldName: string;
  /** World のサムネイル画像の元 URL (VRChat 側)。無ければ null */
  thumbnailUrl: string | null;
  instanceType: InstanceType;
  /** インスタンスを立てた group / user の名前。Public や解決できなかったときは null */
  hostName: string | null;
  userCount: number;
  capacity: number;
}

export const getFriends = () => invoke<Friend[]>("social_get_friends");
export const getGroups = () => invoke<Group[]>("social_get_groups");
export const getGroupInstances = () => invoke<GroupInstanceList>("social_get_group_instances");
export const getInstancesOfGroup = (groupId: string) => invoke<GroupInstance[]>("social_get_instances_of_group", { groupId });
export const getInstanceDetail = (location: string) => invoke<InstanceDetail>("social_get_instance_detail", { location });
export const setPinnedFriends = (ids: string[]) => invoke<void>("social_set_pinned_friends", { ids });

export const onFriendsUpdated = (handler: (friends: Friend[]) => void): Promise<UnlistenFn> =>
  listen<Friend[]>("social://friends-updated", (event) => handler(event.payload));
export const onGroupsUpdated = (handler: (groups: Group[]) => void): Promise<UnlistenFn> =>
  listen<Group[]>("social://groups-updated", (event) => handler(event.payload));
export const onSessionExpired = (handler: () => void): Promise<UnlistenFn> =>
  listen("social://session-expired", () => handler());
