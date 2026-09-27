import type { Friend, InstanceDetail } from "../lib/social";

/** ダミー Friend の id 接頭辞。実データの id (`usr_…`) と衝突させず、ダミーかどうかの判定にも使う */
const DUMMY_ID_PREFIX = "dummy_";

interface DummyFriend {
  friend: Friend;
  /** Friends 欄に pin 済みとして表示するか */
  isPinned: boolean;
}

let nextId = 0;
const dummyId = () => `${DUMMY_ID_PREFIX}${String(++nextId).padStart(2, "0")}`;

/** 同じ World の同じインスタンスにいる Friend は location を共有させる */
const instance = (worldKey: string, instanceId: string, region: "jp" | "us" | "eu" = "jp") =>
  `wrld_${DUMMY_ID_PREFIX}${worldKey}:${instanceId}~region(${region})`;

/** インスタンス内の Friend。VRChat では Ask Me / Do Not Disturb だと location が非公開になるため Join Me / Online のみ */
const inWorld = (
  name: string,
  status: "join" | "online",
  location: string,
  worldName: string,
  isPinned = true,
  statusMessage = "",
): DummyFriend => ({
  friend: { id: dummyId(), name, status, statusMessage, locationKind: "world", location, worldName, isWorldLoading: false },
  isPinned,
});

/** location が分からない Friend (非公開インスタンス / 移動中 / Web / オフライン) */
const elsewhere = (
  name: string,
  status: Friend["status"],
  locationKind: Exclude<Friend["locationKind"], "world">,
  isPinned = true,
): DummyFriend => ({
  friend: {
    id: dummyId(),
    name,
    // オフライン中は選んでいる status に関わらず Offline 表示になる (Rust 側と同じ扱い)
    status: locationKind === "offline" ? "offline" : status,
    statusMessage: "",
    locationKind,
    location: null,
    worldName: null,
    isWorldLoading: false,
  },
  isPinned,
});

const ENGAWA = instance("engawa", "48213");
const ROOFTOP = instance("rooftop", "10977~friends(usr_dummy)");
const OBSERVATORY = instance("observatory", "63052");
const IZAKAYA = instance("izakaya", "25518~hidden(usr_dummy)");
const DRIVE = instance("drive", "71404", "us");
const CLASSROOM = instance("classroom", "39870");
const AQUARIUM = instance("aquarium", "80361~friends(usr_dummy)");

const DUMMY_FRIENDS: DummyFriend[] = [
  // インスタンス内 (同じワールドに複数人いるケースも含める)
  inWorld("あおい", "join", ENGAWA, "夜の縁側 - Night Engawa", true, "誰でもどうぞ〜"),
  inWorld("Haru_VR", "online", ENGAWA, "夜の縁側 - Night Engawa"),
  inWorld("みお*", "join", ENGAWA, "夜の縁側 - Night Engawa", false),
  inWorld("Tsubasa.exe", "online", ROOFTOP, "Cozy Rooftop Bar"),
  inWorld("りんりん", "online", ROOFTOP, "Cozy Rooftop Bar"),
  inWorld("Akari☆", "join", OBSERVATORY, "星見の丘の天文台", true, "今日は早めに寝ます"),
  inWorld("しゅん", "online", IZAKAYA, "居酒屋ぽんぽこ"),
  inWorld("Nagi", "online", IZAKAYA, "居酒屋ぽんぽこ", false),
  inWorld("Ryo_Ryo", "join", DRIVE, "Midnight Highway Drive"),
  inWorld("ゆきみだいふく", "online", CLASSROOM, "放課後の教室 v2.1"),
  inWorld("Miyu", "join", AQUARIUM, "Deep Blue Aquarium"),
  // 非公開インスタンス内
  elsewhere("れん", "ask", "private"),
  elsewhere("Sora_0412", "busy", "private"),
  elsewhere("だいち", "online", "private"),
  elsewhere("Emi", "ask", "private"),
  elsewhere("こうちゃ", "busy", "private", false),
  elsewhere("Kaito_JP", "join", "private"),
  // 移動中
  elsewhere("ひなた", "join", "traveling"),
  elsewhere("Touma", "online", "traveling"),
  // ゲーム外 (Web) でオンライン
  elsewhere("ななせ", "online", "website"),
  elsewhere("Hibiki", "ask", "website", false),
  // オフライン
  elsewhere("りく", "join", "offline"),
  elsewhere("Mai_Mai", "ask", "offline"),
  elsewhere("けんた", "online", "offline"),
  elsewhere("Yui", "busy", "offline"),
  elsewhere("さくら", "online", "offline"),
  elsewhere("Kotaro", "join", "offline", false),
  elsewhere("ちひろ", "online", "offline", false),
  elsewhere("Natsu_VRC", "ask", "offline", false),
];

/** サムネイル撮影用のダミー Friends (Thumbnail mode) */
export const THUMBNAIL_FRIENDS: Friend[] = DUMMY_FRIENDS.map((d) => d.friend);

/** Thumbnail mode 開始時に pin 済みとして扱うダミー Friends */
export const THUMBNAIL_PINNED_IDS: Record<string, boolean> = Object.fromEntries(
  DUMMY_FRIENDS.filter((d) => d.isPinned).map((d) => [d.friend.id, true]),
);

/** ダミーインスタンスの詳細 (Thumbnail mode で展開したときに API の代わりに使う) */
const DUMMY_INSTANCE_DETAILS: Record<string, Omit<InstanceDetail, "location" | "worldName" | "thumbnailUrl">> = {
  [ENGAWA]: { instanceType: "public", hostName: null, userCount: 9, capacity: 24 },
  [ROOFTOP]: { instanceType: "friends", hostName: "Tsubasa.exe", userCount: 6, capacity: 16 },
  [OBSERVATORY]: { instanceType: "public", hostName: null, userCount: 32, capacity: 32 },
  [IZAKAYA]: { instanceType: "friendsPlus", hostName: "しゅん", userCount: 11, capacity: 20 },
  [DRIVE]: { instanceType: "public", hostName: null, userCount: 4, capacity: 12 },
  [CLASSROOM]: { instanceType: "public", hostName: null, userCount: 15, capacity: 30 },
  [AQUARIUM]: { instanceType: "friends", hostName: "Miyu", userCount: 3, capacity: 16 },
};

/** ダミーインスタンスの詳細。ダミーでない location なら null */
export const getDummyInstanceDetail = (location: string): InstanceDetail | null => {
  const detail = DUMMY_INSTANCE_DETAILS[location];
  const friend = THUMBNAIL_FRIENDS.find((f) => f.location === location);
  if (!detail || !friend?.worldName) return null;
  return { ...detail, location, worldName: friend.worldName, thumbnailUrl: null };
};

export const isDummyLocation = (location: string) => location.startsWith(`wrld_${DUMMY_ID_PREFIX}`);
