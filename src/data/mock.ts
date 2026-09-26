export interface RecommendedGroup {
  name: string;
  members: string;
  joined: boolean;
  description: string;
}

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
