export interface GroupInstance {
  id: string;
  world: string;
  accessType: string;
  userCount: number;
  capacity: number;
  /** 表示用の作成時刻 (HH:mm) */
  createdLabel: string;
  /** ソート用の作成時刻 (0:00 起点の分。日付跨ぎを考慮して 24h を超える値を許容) */
  createdMinutes: number;
}

export interface Group {
  id: string;
  name: string;
  instances: GroupInstance[];
}

export type FriendStatus = "join" | "online" | "ask" | "busy" | "offline";

export interface Friend {
  id: string;
  status: FriendStatus;
  name: string;
  world?: string;
  subLabel?: string;
  userCount?: number;
  capacity?: number;
  isPrivate?: boolean;
  isOffline?: boolean;
}

export interface RecommendedGroup {
  name: string;
  members: string;
  joined: boolean;
  description: string;
}

export const MOCK_GROUPS: Group[] = [
  {
    id: "lib",
    name: "寝落ち図書館",
    instances: [
      { id: "i1", world: "静かな書庫", accessType: "Group Public", userCount: 6, capacity: 16, createdLabel: "23:12", createdMinutes: 312 },
      { id: "i2", world: "月明かりの閲覧室", accessType: "Group+", userCount: 2, capacity: 12, createdLabel: "00:40", createdMinutes: 400 },
    ],
  },
  {
    id: "yuru",
    name: "ゆるV睡の会",
    instances: [
      { id: "i3", world: "Cozy Futon Room", accessType: "Group Public", userCount: 11, capacity: 20, createdLabel: "22:05", createdMinutes: 245 },
      { id: "i4", world: "雨音の和室", accessType: "Group", userCount: 4, capacity: 8, createdLabel: "01:15", createdMinutes: 435 },
    ],
  },
  {
    id: "camp",
    name: "星空テント部",
    instances: [
      { id: "i5", world: "Starlit Camp", accessType: "Group Public", userCount: 8, capacity: 8, createdLabel: "21:30", createdMinutes: 210 },
    ],
  },
  { id: "work", name: "夜ふかし作業部屋", instances: [] },
  {
    id: "sea",
    name: "波の音クラブ",
    instances: [
      { id: "i6", world: "Quiet Shore", accessType: "Group Public", userCount: 9, capacity: 24, createdLabel: "22:48", createdMinutes: 288 },
      { id: "i7", world: "灯台の見える部屋", accessType: "Group+", userCount: 3, capacity: 6, createdLabel: "00:05", createdMinutes: 365 },
    ],
  },
  {
    id: "cat",
    name: "ねこと寝る部",
    instances: [
      { id: "i8", world: "Cat Nap Lounge", accessType: "Group Public", userCount: 15, capacity: 30, createdLabel: "21:50", createdMinutes: 230 },
    ],
  },
  {
    id: "rain",
    name: "雨の日同好会",
    instances: [
      { id: "i9", world: "Rainy Window", accessType: "Group", userCount: 5, capacity: 12, createdLabel: "23:40", createdMinutes: 340 },
      { id: "i10", world: "梅雨の縁側", accessType: "Group Public", userCount: 7, capacity: 16, createdLabel: "01:30", createdMinutes: 450 },
    ],
  },
  { id: "asmr", name: "ささやきASMR部", instances: [] },
];

export const MOCK_FRIENDS: Friend[] = [
  { id: "f1", status: "online", name: "ねこやなぎ", world: "静かな書庫", subLabel: "寝落ち図書館", userCount: 6, capacity: 16 },
  { id: "f2", status: "join", name: "しろくま", world: "Cozy Futon Room", subLabel: "ゆるV睡の会", userCount: 11, capacity: 20 },
  { id: "f3", status: "ask", name: "Kaito", isPrivate: true },
  { id: "f4", status: "busy", name: "ユズ", isPrivate: true },
  { id: "f5", status: "offline", name: "mint", isOffline: true },
  { id: "f6", status: "online", name: "Hana", world: "Midnight Café", subLabel: "Friends+", userCount: 3, capacity: 10 },
  { id: "f7", status: "offline", name: "たぬき", isOffline: true },
  { id: "f8", status: "ask", name: "Sora", isPrivate: true },
  { id: "f9", status: "join", name: "Mochi", world: "Rooftop Garden", subLabel: "Public", userCount: 14, capacity: 32 },
  { id: "f10", status: "online", name: "あおい", world: "Quiet Shore", subLabel: "波の音クラブ", userCount: 9, capacity: 24 },
  { id: "f11", status: "join", name: "Ren", world: "Cat Nap Lounge", subLabel: "ねこと寝る部", userCount: 15, capacity: 30 },
  { id: "f12", status: "busy", name: "ふわり", isPrivate: true },
  { id: "f13", status: "offline", name: "Kuro", isOffline: true },
  { id: "f14", status: "online", name: "こはる", world: "Rainy Window", subLabel: "雨の日同好会", userCount: 5, capacity: 12 },
  { id: "f15", status: "ask", name: "Lumi", isPrivate: true },
  { id: "f16", status: "offline", name: "はっか", isOffline: true },
  { id: "f17", status: "join", name: "Nagi", world: "灯台の見える部屋", subLabel: "波の音クラブ", userCount: 3, capacity: 6 },
  { id: "f18", status: "online", name: "つむぎ", world: "Starlit Camp", subLabel: "星空テント部", userCount: 8, capacity: 8 },
];

export const INITIAL_PINNED_FRIENDS: Record<string, boolean> = {
  f1: true,
  f2: true,
  f3: true,
  f4: true,
  f5: true,
  f10: true,
  f11: true,
  f12: true,
  f13: true,
  f14: true,
};

export const MOCK_RECOMMENDED_GROUPS: RecommendedGroup[] = [
  { name: "寝落ち図書館", members: "2,140", joined: true, description: "本を読みながらそのまま眠れる、静かな読書ワールドで毎晩開いています。" },
  { name: "ゆるV睡の会", members: "5,882", joined: true, description: "初めてのV睡でも入りやすい集まり。話し声は小さめ、途中参加も自由です。" },
  { name: "焚き火と朝まで", members: "930", joined: false, description: "焚き火の音だけが流れるキャンプワールド。朝まで開いていることが多いです。" },
  { name: "V睡はじめて案内所", members: "1,406", joined: false, description: "寝落ち用の設定やマナーを案内役が教えてくれる、入門向けグループです。" },
];

/** モック: 429 (Rate limited) 状態を再現する場合は true */
export const MOCK_RATE_LIMITED = false;
/** モック: 更新チェックで新しいバージョンが見つかるか */
export const MOCK_UPDATE_AVAILABLE = true;
export const MOCK_LATEST_VERSION = "0.2.0";
