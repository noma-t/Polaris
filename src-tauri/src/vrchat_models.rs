//! VRChat API / pipeline のレスポンスのうち Polaris が使うフィールドだけを持つ struct。

use serde::Deserialize;

/// `GET /auth/user/friends` の要素。pipeline イベントの `user` もこの形で受ける
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiFriend {
    pub id: String,
    pub display_name: String,
    /// `"join me"` / `"active"` / `"ask me"` / `"busy"` / `"offline"`
    pub status: String,
    /// `"offline"` / `"private"` / `"traveling"` / `"wrld_…:…"`。pipeline の `user` には含まれない
    pub location: String,
    /// ユーザーが設定しているステータスメッセージ
    pub status_description: String,
}

/// `GET /worlds/{id}` のレスポンス。pipeline `friend-location` の `world` もこの形で受ける
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiWorld {
    pub id: String,
    pub name: String,
    pub thumbnail_image_url: String,
}

/// `GET /instances/{worldId}:{instanceId}` のレスポンスのうち詳細表示に使うフィールド
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiInstanceDetail {
    pub location: String,
    pub user_count: u32,
    pub capacity: u32,
    pub world: ApiWorld,
}

/// `GET /users/{id}` のうちインスタンスのホスト名表示に使うフィールド
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiUserName {
    pub display_name: String,
}

/// `GET /groups/{id}` のうちインスタンスのホスト名表示に使うフィールド
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiGroupName {
    pub name: String,
}

/// `GET /users/{id}/groups` の要素
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiUserGroup {
    /// `grp_…` 形式の group ID (`id` は membership ID なので使わない)
    pub group_id: String,
    pub name: String,
}

/// `GET /users/{id}/instances/groups` のレスポンス
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiGroupInstanceList {
    /// VRChat 側でこの一覧が取得された時刻 (RFC 3339)
    pub fetched_at: String,
    pub instances: Vec<ApiInstance>,
}

/// Instance のうちグループインスタンスの表示に使うフィールド
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiInstance {
    /// `wrld_…:…` 形式。インスタンスの識別に使う
    pub location: String,
    /// グループインスタンスでは `grp_…` 形式の group ID
    pub owner_id: String,
    pub user_count: u32,
    pub capacity: u32,
    /// `"public"` / `"plus"` / `"members"`
    pub group_access_type: String,
    pub world: ApiWorld,
}

/// `GET /groups/{groupId}/instances` の要素。
/// `userCount` / `capacity` / `groupAccessType` / `ownerId` は含まれない
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiGroupInstance {
    /// `wrld_…:…~group(grp_…)~groupAccessType(…)~…` 形式
    pub location: String,
    /// インスタンス内の人数
    pub member_count: u32,
    pub world: ApiGroupInstanceWorld,
}

/// `ApiGroupInstance` の `world` のうち表示に使うフィールド
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiGroupInstanceWorld {
    pub name: String,
    pub capacity: u32,
}
