/** V-Sui Groups 画面に表示するグループ。値は VRChat API `GET /groups/{groupId}` の内容を手動で転記したもの */
export interface VsuiGroup {
  /** `grp_…` 形式の group ID */
  groupId: string;
  name: string;
  members: string;
  joined: boolean;
  description: string;
}

export const VSUI_GROUPS: VsuiGroup[] = [
  { groupId: "grp_4294b2d6-d3f7-495b-8523-a9899f767a3f", name: "V睡ラウンジ -もうふもふもふ-", members: "6,605", joined: true, description: "VR睡眠マッチングワールド『V睡ラウンジ-もうふもふもふ-』 の公式グループです。" },
  { groupId: "grp_9125235e-9a58-475c-90c9-6b6880dc8168", name: "いっしょに寝ようよ", members: "9,417", joined: true, description: "「いっしょに寝ようよ」のVR睡眠グループです。添い寝したりされたり" },
  { groupId: "grp_6c42182e-600c-4d67-9e91-3bc80beff3df", name: "ガチ恋距離睡眠", members: "4,423", joined: true, description: "くっついて寝ることが好きな人のためのグループです！ 抱きついて寝たり 顔を近づけて寝たり とにかく近くで寝たいっ！という人の集まりです！" },
  { groupId: "grp_98e35acd-839e-4fec-913a-250e0bd262ed", name: "ミルフィぶいすい部", members: "413", joined: true, description: "ミルフィと一緒に寝たい！そんな気持ちで作った、ミルフィ限定のV睡グループです～！" },
];
