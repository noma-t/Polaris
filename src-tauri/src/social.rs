//! Friends / Groups の状態を Rust 側で保持し、REST の初期同期と pipeline イベントで更新する。
//! 状態が変わるたびに整形済みの snapshot をフロントエンドへ emit する。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::auth_log;
use crate::background;
use crate::instance_store;
use crate::pipeline::{self, PipelineEvent};
use crate::vrchat_client::{AuthError, AuthUserResponse, VrchatClient, FRIENDS_PAGE_SIZE};
use crate::vrchat_models::{ApiFriend, ApiGroupInstance, ApiInstance, ApiInstanceDetail};

pub const FRIENDS_UPDATED_EVENT: &str = "social://friends-updated";
pub const GROUPS_UPDATED_EVENT: &str = "social://groups-updated";
pub const SESSION_EXPIRED_EVENT: &str = "social://session-expired";

/// World 名の取得は 1 秒に 1 件まで
const WORLD_FETCH_INTERVAL: Duration = Duration::from_secs(1);
/// 取得に失敗した World を再試行するまでの間隔
const WORLD_RETRY_DELAY: Duration = Duration::from_secs(30);
/// 429 を受けたときに World 名の取得を止める時間
const RATE_LIMIT_PAUSE: Duration = Duration::from_secs(60);
/// fetchedAt から次にグループインスタンスを全体取得するまでの間隔 (フロントエンドの自動更新と揃える)
const GROUP_INSTANCES_REFRESH_INTERVAL: Duration = Duration::from_secs(90);
/// 端末と VRChat の時計のずれで fetchedAt + 間隔 が過ぎていても、連続取得しないよう最低限空ける時間
const GROUP_INSTANCES_MIN_REFRESH_DELAY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FriendStatus {
    Join,
    Online,
    Ask,
    Busy,
    Offline,
}

impl FriendStatus {
    fn from_api(status: &str) -> Self {
        match status {
            "join me" => Self::Join,
            "ask me" => Self::Ask,
            "busy" => Self::Busy,
            "offline" => Self::Offline,
            _ => Self::Online,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FriendLocation {
    /// `location` は `wrld_…:…` 形式の全体 (インスタンスを開くのに使う)
    World { world_id: String, location: String },
    Private,
    Traveling,
    /// ゲーム外 (Web サイト上) でオンライン
    Website,
    Offline,
}

impl FriendLocation {
    /// `wrld_…:…` / `private` / `traveling` / `offline` 形式の location を解釈する
    fn from_api(location: &str) -> Self {
        match location {
            "offline" => Self::Offline,
            "traveling" => Self::Traveling,
            _ => match location.split_once(':') {
                Some((world_id, _)) if world_id.starts_with("wrld_") => {
                    Self::World { world_id: world_id.to_owned(), location: location.to_owned() }
                }
                _ => Self::Private,
            },
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::World { .. } => "world",
            Self::Private => "private",
            Self::Traveling => "traveling",
            Self::Website => "website",
            Self::Offline => "offline",
        }
    }
}

#[derive(Debug, Clone)]
struct FriendEntry {
    id: String,
    name: String,
    /// ユーザーが選んでいる status。オフライン中も保持し、表示時に Offline へ置き換える
    status: FriendStatus,
    status_message: String,
    location: FriendLocation,
}

impl FriendEntry {
    fn from_api(user: &ApiFriend, location: FriendLocation) -> Self {
        Self {
            id: user.id.clone(),
            name: user.display_name.clone(),
            status: FriendStatus::from_api(&user.status),
            status_message: user.status_description.clone(),
            location,
        }
    }

    /// pipeline の `user` でプロフィールを更新する (location は含まれないので変えない)
    fn update_profile(&mut self, user: &ApiFriend) {
        if !user.display_name.is_empty() {
            self.name = user.display_name.clone();
        }
        // status を含む `user` はプロフィール全体なので、空のステータスメッセージも「未設定」として反映する
        if !user.status.is_empty() {
            self.status = FriendStatus::from_api(&user.status);
            self.status_message = user.status_description.clone();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendView {
    id: String,
    name: String,
    status: FriendStatus,
    status_message: String,
    location_kind: &'static str,
    /// locationKind が world のときのみ。`wrld_…:…` 形式
    location: Option<String>,
    world_name: Option<String>,
    /// pinned かつ World 名が未取得 (取得待ち・取得中)
    is_world_loading: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupView {
    id: String,
    name: String,
}

/// グループインスタンス 1 件分の表示用データ
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInstanceView {
    /// `wrld_…:…` 形式の location
    id: String,
    group_id: String,
    world_name: String,
    /// `"public"` / `"plus"` / `"members"`
    access_type: &'static str,
    user_count: u32,
    capacity: u32,
    /// API に作成時刻が無いため、Polaris が初めて観測した時刻 (epoch ms) を表示に使う
    first_seen_at: i64,
    /// Created ソート用の値。大きいほど新しい
    created_order: u32,
}

/// 全グループ分のグループインスタンス取得結果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInstanceListView {
    /// VRChat 側でこの一覧が取得された時刻 (epoch ms)。レスポンスから読めなければ Polaris が受け取った時刻
    fetched_at: i64,
    instances: Vec<GroupInstanceView>,
}

/// フレンドがいるインスタンス 1 件分の詳細表示用データ
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDetailView {
    /// `wrld_…:…` 形式の location
    location: String,
    world_name: String,
    thumbnail_url: Option<String>,
    /// `"public"` / `"friendsPlus"` / `"friends"` / `"invitePlus"` / `"invite"` / `"groupPublic"` / `"groupPlus"` / `"group"`
    instance_type: &'static str,
    /// インスタンスを立てた group / user の名前。Public や解決できなかったときは None
    host_name: Option<String>,
    user_count: u32,
    capacity: u32,
}

/// インスタンスを立てた側
#[derive(Debug, Clone, PartialEq, Eq)]
enum InstanceHost {
    Group(String),
    User(String),
}

/// location の `~…` 部分からインスタンス種別とホストを読み取る
fn instance_type_from_location(location: &str) -> (&'static str, Option<InstanceHost>) {
    if let Some(group_id) = location_param(location, "group") {
        let instance_type = match location_param(location, "groupAccessType") {
            Some("public") => "groupPublic",
            Some("plus") => "groupPlus",
            _ => "group",
        };
        return (instance_type, Some(InstanceHost::Group(group_id.to_owned())));
    }
    let user_host = |id: &str| Some(InstanceHost::User(id.to_owned()));
    if let Some(user_id) = location_param(location, "hidden") {
        return ("friendsPlus", user_host(user_id));
    }
    if let Some(user_id) = location_param(location, "friends") {
        return ("friends", user_host(user_id));
    }
    if let Some(user_id) = location_param(location, "private") {
        let can_request_invite = location.split('~').skip(1).any(|part| part == "canRequestInvite");
        return (if can_request_invite { "invitePlus" } else { "invite" }, user_host(user_id));
    }
    ("public", None)
}

/// 観測済みのグループインスタンスについて、取得をまたいで保持する値
/// アプリを終了しても引き継げるよう instance_store で保存する
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SeenInstance {
    /// 初めて観測した時刻 (epoch ms)
    pub(crate) first_seen_at: i64,
    /// 初めて観測したときに振った Created ソート用の値。以降の取得では変えない
    pub(crate) created_order: u32,
}

fn group_access_type_from_api(access_type: &str) -> &'static str {
    match access_type {
        "public" => "public",
        "plus" => "plus",
        _ => "members",
    }
}

/// location の `~key(value)` 部分から value を取り出す
fn location_param<'a>(location: &'a str, key: &str) -> Option<&'a str> {
    location.split('~').skip(1).find_map(|part| part.strip_prefix(key)?.strip_prefix('(')?.strip_suffix(')'))
}

/// 取得元 API ごとの形の違いを吸収した、グループインスタンス 1 件分の値
struct ObservedGroupInstance {
    location: String,
    group_id: String,
    world_name: String,
    access_type: &'static str,
    user_count: u32,
    capacity: u32,
}

fn now_epoch_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// RFC 3339 形式 (`2024-05-01T12:34:56.789Z` / `…+09:00`) の日時を epoch ms に変換する。
/// crate を増やさないよう、VRChat API が返す形式に必要な範囲だけ自前で parse する
fn parse_rfc3339_ms(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let digits = |s: &str| -> Option<i64> { s.bytes().all(|c| c.is_ascii_digit()).then(|| s.parse().ok()).flatten() };
    let number = |start: usize, len: usize| digits(text.get(start..start + len)?);
    let is_separator = |index: usize, expected: &[u8]| bytes.get(index).is_some_and(|c| expected.contains(c));
    if !(is_separator(4, b"-") && is_separator(7, b"-") && is_separator(10, b"Tt ") && is_separator(13, b":") && is_separator(16, b":")) {
        return None;
    }
    let (year, month, day) = (number(0, 4)?, number(5, 2)?, number(8, 2)?);
    let (hour, minute, second) = (number(11, 2)?, number(14, 2)?, number(17, 2)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let mut rest = text.get(19..)?;
    let mut millis = 0;
    if let Some(fraction) = rest.strip_prefix('.') {
        let len = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if len == 0 {
            return None;
        }
        // ms より細かい桁は切り捨てる
        let head = &fraction[..len.min(3)];
        millis = head.parse::<i64>().ok()? * 10_i64.pow(3 - head.len() as u32);
        rest = &fraction[len..];
    }
    let offset_minutes = match rest.as_bytes() {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), _, _, b':', _, _] => {
            let minutes = digits(&rest[1..3])? * 60 + digits(&rest[4..6])?;
            if *sign == b'+' { minutes } else { -minutes }
        }
        _ => return None,
    };

    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    Some(seconds * 1_000 + millis)
}

/// 1970-01-01 からの日数 (proleptic Gregorian calendar)
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// epoch ms をログ用に端末のタイムゾーンの時刻 (`2024-05-01 21:34:56.789 +09:00`) にする
fn format_local_time(epoch_ms: i64) -> String {
    chrono::Local
        .timestamp_millis_opt(epoch_ms)
        .single()
        .map_or_else(|| epoch_ms.to_string(), |time| time.format("%Y-%m-%d %H:%M:%S%.3f %:z").to_string())
}

/// fetchedAt から次にグループインスタンスを全体取得するまでの待ち時間。
/// 端末と VRChat の時計がずれていても連続取得や長すぎる待ちにならないよう、[最低限空ける時間, 取得間隔] に収める
fn group_instances_refresh_delay(fetched_at_ms: i64, now_ms: i64) -> Duration {
    let interval_ms = GROUP_INSTANCES_REFRESH_INTERVAL.as_millis() as i64;
    let min_ms = GROUP_INSTANCES_MIN_REFRESH_DELAY.as_millis() as i64;
    Duration::from_millis((fetched_at_ms + interval_ms - now_ms).clamp(min_ms, interval_ms) as u64)
}

#[derive(Default)]
struct SocialStore {
    /// start / stop のたびに進める。古いセッションのタスクによる書き込みを捨てるために使う
    generation: u64,
    friends: HashMap<String, FriendEntry>,
    pinned: HashSet<String>,
    world_names: HashMap<String, String>,
    world_retry_at: HashMap<String, Instant>,
    groups: Vec<GroupView>,
    /// インスタンスのホスト (group / user) の ID → 名前。所属グループ・friend 以外を API で引いた分
    host_names: HashMap<String, String>,
    /// グループインスタンスの location → 観測済みの値
    instance_seen: HashMap<String, SeenInstance>,
    /// 次に初めて見るグループインスタンスに振る Created ソート用の値
    next_created_order: u32,
}

impl SocialStore {
    fn friend_views(&self) -> Vec<FriendView> {
        self.friends
            .values()
            .map(|friend| {
                let (location, world_name) = match &friend.location {
                    FriendLocation::World { world_id, location } => (Some(location.clone()), self.world_names.get(world_id).cloned()),
                    _ => (None, None),
                };
                let is_in_world = matches!(friend.location, FriendLocation::World { .. });
                FriendView {
                    id: friend.id.clone(),
                    name: friend.name.clone(),
                    status: if friend.location == FriendLocation::Offline { FriendStatus::Offline } else { friend.status },
                    status_message: friend.status_message.clone(),
                    location_kind: friend.location.kind(),
                    location,
                    is_world_loading: is_in_world && world_name.is_none() && self.pinned.contains(&friend.id),
                    world_name,
                }
            })
            .collect()
    }

    /// pinned friend がいる World のうち、名前が未取得で再試行待ちでもない最初の 1 件
    fn next_world_to_fetch(&self, now: Instant) -> Option<String> {
        let mut candidates: Vec<&FriendEntry> = self.pinned.iter().filter_map(|id| self.friends.get(id)).collect();
        candidates.sort_by(|a, b| a.name.cmp(&b.name));
        candidates.into_iter().find_map(|friend| match &friend.location {
            FriendLocation::World { world_id, .. }
                if !self.world_names.contains_key(world_id)
                    && self.world_retry_at.get(world_id).is_none_or(|at| now >= *at) =>
            {
                Some(world_id.clone())
            }
            _ => None,
        })
    }

    fn replace_friends(&mut self, online: Vec<ApiFriend>, offline: Vec<ApiFriend>) {
        self.friends.clear();
        for user in &offline {
            self.friends.insert(user.id.clone(), FriendEntry::from_api(user, FriendLocation::Offline));
        }
        for user in &online {
            // online 一覧で location が offline なのはゲーム外 (Web) でオンラインの friend
            let location = match FriendLocation::from_api(&user.location) {
                FriendLocation::Offline => FriendLocation::Website,
                location => location,
            };
            self.friends.insert(user.id.clone(), FriendEntry::from_api(user, location));
        }
    }

    /// 既知の friend の location を更新する。未知なら `user` があれば追加する
    fn set_location(&mut self, user_id: &str, user: Option<&ApiFriend>, location: FriendLocation) {
        if let Some(friend) = self.friends.get_mut(user_id) {
            if let Some(user) = user {
                friend.update_profile(user);
            }
            friend.location = location;
        } else if let Some(user) = user.filter(|u| u.id == user_id) {
            self.friends.insert(user_id.to_owned(), FriendEntry::from_api(user, location));
        }
    }

    /// 所属グループ・friend・取得済みのキャッシュからホスト名を引く
    fn known_host_name(&self, host: &InstanceHost) -> Option<String> {
        match host {
            InstanceHost::Group(id) => self.groups.iter().find(|g| &g.id == id).map(|g| g.name.clone()),
            InstanceHost::User(id) => self.friends.get(id).map(|f| f.name.clone()),
        }
        .or_else(|| {
            let (InstanceHost::Group(id) | InstanceHost::User(id)) = host;
            self.host_names.get(id).cloned()
        })
    }

    /// 全グループ分のインスタンスから表示用データを作る。
    /// 初めて見る location は `now_ms` を記録し、今回含まれなかった (閉じた) location の記録は破棄する
    fn apply_group_instances(&mut self, instances: Vec<ApiInstance>, now_ms: i64) -> Vec<GroupInstanceView> {
        let observed = instances
            .into_iter()
            .filter(|inst| inst.owner_id.starts_with("grp_"))
            .map(|inst| ObservedGroupInstance {
                access_type: group_access_type_from_api(&inst.group_access_type),
                location: inst.location,
                group_id: inst.owner_id,
                world_name: inst.world.name,
                user_count: inst.user_count,
                capacity: inst.capacity,
            })
            .collect();
        let (views, seen) = self.group_instance_views(observed, now_ms);
        self.instance_seen = seen;
        views
    }

    /// 1 グループ分のインスタンスから表示用データを作る。
    /// 観測済みの記録はこのグループの分だけ置き換え、ほかのグループの記録は残す
    fn apply_instances_of_group(&mut self, group_id: &str, instances: Vec<ApiGroupInstance>, now_ms: i64) -> Vec<GroupInstanceView> {
        let observed = instances
            .into_iter()
            .map(|inst| ObservedGroupInstance {
                access_type: group_access_type_from_api(location_param(&inst.location, "groupAccessType").unwrap_or_default()),
                location: inst.location,
                group_id: group_id.to_owned(),
                world_name: inst.world.name,
                user_count: inst.member_count,
                capacity: inst.world.capacity,
            })
            .collect();
        let (views, seen) = self.group_instance_views(observed, now_ms);
        self.instance_seen.retain(|location, _| location_param(location, "group") != Some(group_id));
        self.instance_seen.extend(seen);
        views
    }

    /// 表示用データと、そのインスタンスの location → 観測済みの値を返す。
    /// 観測済みのインスタンスは記録済みの値を使い、Created 順を変えない。初めて見るインスタンスだけ既存より新しい値を振る
    fn group_instance_views(
        &mut self,
        instances: Vec<ObservedGroupInstance>,
        now_ms: i64,
    ) -> (Vec<GroupInstanceView>, HashMap<String, SeenInstance>) {
        let instances: Vec<_> = instances.into_iter().filter(|inst| !inst.location.is_empty() && !inst.world_name.is_empty()).collect();
        let mut seen = HashMap::new();
        // API はおおむね作成が古い順に返すので、初めて見るインスタンスには先頭から順に値を振り、末尾ほど新しくする
        for inst in &instances {
            if seen.contains_key(&inst.location) {
                continue;
            }
            let entry = match self.instance_seen.get(&inst.location) {
                Some(entry) => *entry,
                None => {
                    self.next_created_order += 1;
                    SeenInstance { first_seen_at: now_ms, created_order: self.next_created_order }
                }
            };
            seen.insert(inst.location.clone(), entry);
        }
        let views = instances
            .into_iter()
            .map(|inst| {
                let SeenInstance { first_seen_at, created_order } = seen[&inst.location];
                GroupInstanceView {
                    id: inst.location,
                    group_id: inst.group_id,
                    world_name: inst.world_name,
                    access_type: inst.access_type,
                    user_count: inst.user_count,
                    capacity: inst.capacity,
                    first_seen_at,
                    created_order,
                }
            })
            .collect();
        (views, seen)
    }

    /// pipeline イベントを適用する。Friends の snapshot を送り直す必要があれば true
    fn apply(&mut self, event: &PipelineEvent) -> bool {
        match event {
            PipelineEvent::FriendOnline { user_id, location, user } => {
                let location = match FriendLocation::from_api(location) {
                    FriendLocation::Offline => FriendLocation::Private,
                    location => location,
                };
                self.set_location(user_id, user.as_ref(), location);
            }
            PipelineEvent::FriendActive { user_id, user } => {
                self.set_location(user_id, user.as_ref(), FriendLocation::Website);
            }
            PipelineEvent::FriendOffline { user_id } => {
                self.set_location(user_id, None, FriendLocation::Offline);
            }
            PipelineEvent::FriendLocation { user_id, location, user, world } => {
                if let Some(world) = world {
                    self.world_names.insert(world.id.clone(), world.name.clone());
                }
                self.set_location(user_id, user.as_ref(), FriendLocation::from_api(location));
            }
            PipelineEvent::FriendUpdate { user_id, user } => match self.friends.get_mut(user_id) {
                Some(friend) => friend.update_profile(user),
                None => return false,
            },
            PipelineEvent::FriendAdd { user_id, user } => {
                let location = match user.location.as_str() {
                    "" => FriendLocation::Offline,
                    location => FriendLocation::from_api(location),
                };
                self.friends.insert(user_id.clone(), FriendEntry { id: user_id.clone(), ..FriendEntry::from_api(user, location) });
            }
            PipelineEvent::FriendDelete { user_id } => {
                self.friends.remove(user_id);
                self.pinned.remove(user_id);
            }
            PipelineEvent::Connected | PipelineEvent::GroupsChanged => return false,
        }
        true
    }
}

/// 401 のあとの `/auth/user` の結果から、セッションが失効したかを判断する。
/// 通信エラーなどで確認できなかったときは、失効扱いにしない
fn is_session_expired(verification: &Result<AuthUserResponse, AuthError>) -> bool {
    matches!(verification, Ok(AuthUserResponse::Unauthorized | AuthUserResponse::RequiresTwoFactor(_)))
}

/// 1 セッション (ログイン中) のタスクが共有するもの
#[derive(Clone)]
struct Session {
    app: AppHandle,
    client: VrchatClient,
    user_id: String,
    store: Arc<Mutex<SocialStore>>,
    generation: u64,
}

impl Session {
    /// 自分のセッションがまだ有効なときだけ store を変更する
    fn with_store<R>(&self, f: impl FnOnce(&mut SocialStore) -> R) -> Option<R> {
        let mut store = self.store.lock().expect("social store poisoned");
        (store.generation == self.generation).then(|| f(&mut store))
    }

    fn emit_friends(&self) {
        if let Some(views) = self.with_store(|store| store.friend_views()) {
            let _ = self.app.emit(FRIENDS_UPDATED_EVENT, views);
        }
    }

    fn emit_groups(&self) {
        if let Some(groups) = self.with_store(|store| store.groups.clone()) {
            let _ = self.app.emit(GROUPS_UPDATED_EVENT, groups);
        }
    }

    /// API のエラーを処理し、呼び出し元へ返すエラーを返す。
    /// 401 は一時的に返ることがあるので、`/auth/user` で失効を確かめてからサインアウトさせる。
    /// 失効していなければセッションを保ち、呼び出し元には (サインアウトを伴わない) 別のエラーを返す
    async fn handle_error(&self, error: AuthError) -> AuthError {
        if !matches!(error, AuthError::Unauthorized) {
            return error;
        }
        if self.with_store(|_| ()).is_none() {
            auth_log::log!("session: unauthorized after sign-out; ignored");
            return error;
        }
        let verification = self.client.get_current_user().await;
        if !is_session_expired(&verification) {
            auth_log::log!("session: unauthorized, but the session could not be confirmed as expired; keeping it");
            return AuthError::Unexpected("VRChat rejected the request, but the session is still valid. Please try again.".into());
        }
        // 確認している間にサインアウトされていたら通知しない
        if self.with_store(|_| ()).is_none() {
            auth_log::log!("session: unauthorized after sign-out; ignored");
            return error;
        }
        auth_log::log!("session: unauthorized; notifying session-expired");
        let _ = self.app.emit(SESSION_EXPIRED_EVENT, ());
        error
    }

    async fn fetch_all_friends(&self, offline: bool) -> Result<Vec<ApiFriend>, AuthError> {
        let mut friends = Vec::new();
        loop {
            let page = self.client.get_friends(offline, friends.len()).await?;
            let is_last = page.len() < FRIENDS_PAGE_SIZE;
            friends.extend(page);
            if is_last {
                return Ok(friends);
            }
        }
    }

    async fn sync_friends(&self) {
        let result = async { Ok::<_, AuthError>((self.fetch_all_friends(false).await?, self.fetch_all_friends(true).await?)) }.await;
        match result {
            Ok((online, offline)) => {
                self.with_store(|store| store.replace_friends(online, offline));
                self.emit_friends();
            }
            Err(error) => {
                self.handle_error(error).await;
            }
        }
    }

    async fn sync_groups(&self) {
        match self.client.get_user_groups(&self.user_id).await {
            Ok(groups) => {
                let mut groups: Vec<GroupView> = groups
                    .into_iter()
                    .filter(|g| !g.group_id.is_empty())
                    .map(|g| GroupView { id: g.group_id, name: g.name })
                    .collect();
                groups.sort_by(|a, b| a.name.cmp(&b.name));
                self.with_store(|store| store.groups = groups);
                self.emit_groups();
            }
            Err(error) => {
                self.handle_error(error).await;
            }
        }
    }

    /// pipeline の受信と store への適用を同じタスク内で回し、タスクの abort で両方止まるようにする
    async fn run_pipeline(self) {
        let (tx, rx) = mpsc::channel(64);
        tokio::select! {
            _ = pipeline::run(self.client.clone(), tx) => {}
            _ = self.consume_pipeline(rx) => {}
        }
    }

    async fn consume_pipeline(&self, mut rx: mpsc::Receiver<PipelineEvent>) {
        let mut has_connected = false;
        while let Some(event) = rx.recv().await {
            match &event {
                // 初回は start 時の同期で足りる。再接続時は切断中の変化を取り戻すため再同期する
                PipelineEvent::Connected => {
                    if has_connected {
                        self.sync_friends().await;
                        self.sync_groups().await;
                    }
                    has_connected = true;
                }
                PipelineEvent::GroupsChanged => self.sync_groups().await,
                _ => {
                    if self.with_store(|store| store.apply(&event)) == Some(true) {
                        self.emit_friends();
                    }
                }
            }
        }
    }

    /// 所属する全グループのインスタンスを取得する。ログアウト後に完了した結果は捨てる
    async fn fetch_group_instances(&self) -> Result<GroupInstanceListView, AuthError> {
        let list = match self.client.get_user_group_instances(&self.user_id).await {
            Ok(list) => list,
            Err(error) => return Err(self.handle_error(error).await),
        };
        let now_ms = now_epoch_ms();
        let parsed = parse_rfc3339_ms(&list.fetched_at);
        let fetched_at = parsed.unwrap_or(now_ms);
        let next_in = group_instances_refresh_delay(fetched_at, now_ms).as_secs_f64();
        match parsed {
            Some(ms) => eprintln!(
                "[group-instances] fetchedAt={} (raw={:?}, age={:.1}s, next in {next_in:.1}s)",
                format_local_time(ms),
                list.fetched_at,
                (now_ms - ms) as f64 / 1000.0,
            ),
            None => eprintln!("[group-instances] fetchedAt={:?} (unparsed; using local time, next in {next_in:.1}s)", list.fetched_at),
        }
        let view = self
            .with_store(|store| GroupInstanceListView { fetched_at, instances: store.apply_group_instances(list.instances, now_ms) })
            .ok_or(AuthError::Unauthorized)?;
        self.persist_instance_seen();
        Ok(view)
    }

    /// 観測済みのグループインスタンスの記録を保存する。ファイル書き込みは store のロック外で行う
    fn persist_instance_seen(&self) {
        if let Some((seen, next_created_order)) = self.with_store(|store| (store.instance_seen.clone(), store.next_created_order)) {
            instance_store::save(&self.app, &self.user_id, seen, next_created_order);
        }
    }

    /// フレンドがいるインスタンスの詳細を取得する。ホスト名は既知のデータに無ければ API で引く
    async fn fetch_instance_detail(&self, location: &str) -> Result<InstanceDetailView, AuthError> {
        let detail: ApiInstanceDetail = match self.client.get_instance(location).await {
            Ok(detail) => detail,
            Err(error) => return Err(self.handle_error(error).await),
        };
        let (instance_type, host) = instance_type_from_location(location);
        let host_name = match host {
            Some(host) => match self.with_store(|store| store.known_host_name(&host)).ok_or(AuthError::Unauthorized)? {
                Some(name) => Some(name),
                None => self.fetch_host_name(&host).await,
            },
            None => None,
        };
        let world = detail.world;
        Ok(InstanceDetailView {
            location: location.to_owned(),
            world_name: world.name,
            thumbnail_url: Some(world.thumbnail_image_url).filter(|url| !url.is_empty()),
            instance_type,
            host_name,
            user_count: detail.user_count,
            capacity: detail.capacity,
        })
    }

    /// ホスト名を API で引いてキャッシュする。取得できなくても詳細は表示したいので、失敗は None として扱う
    async fn fetch_host_name(&self, host: &InstanceHost) -> Option<String> {
        let (id, result) = match host {
            InstanceHost::Group(id) => (id, self.client.get_group(id).await.map(|g| g.name)),
            InstanceHost::User(id) => (id, self.client.get_user(id).await.map(|u| u.display_name)),
        };
        match result {
            Ok(name) if !name.is_empty() => {
                self.with_store(|store| store.host_names.insert(id.clone(), name.clone()));
                Some(name)
            }
            Ok(_) => None,
            Err(error) => {
                self.handle_error(error).await;
                None
            }
        }
    }

    /// Run in background でウィンドウが見えていない間、グループインスタンスを定期取得して初めて観測した時刻を記録し続ける
    /// 次の取得は fetchedAt + 取得間隔 に行い、取得できなかったときは取得間隔だけ待つ
    async fn run_background_group_poll(self) {
        let mut delay = GROUP_INSTANCES_REFRESH_INTERVAL;
        loop {
            tokio::time::sleep(delay).await;
            if !background::should_poll_in_background(&self.app) {
                delay = GROUP_INSTANCES_REFRESH_INTERVAL;
                continue;
            }
            delay = match self.fetch_group_instances().await {
                Ok(list) => group_instances_refresh_delay(list.fetched_at, now_epoch_ms()),
                Err(AuthError::RateLimited) => RATE_LIMIT_PAUSE + GROUP_INSTANCES_REFRESH_INTERVAL,
                Err(AuthError::Unauthorized) => {
                    auth_log::log!("background poll: stopped (unauthorized)");
                    return;
                }
                Err(_) => GROUP_INSTANCES_REFRESH_INTERVAL,
            };
        }
    }

    async fn run_world_queue(self) {
        let mut interval = tokio::time::interval(WORLD_FETCH_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let Some(Some(world_id)) = self.with_store(|store| store.next_world_to_fetch(Instant::now())) else {
                continue;
            };
            match self.client.get_world(&world_id).await {
                Ok(world) => {
                    self.with_store(|store| store.world_names.insert(world_id, world.name));
                    self.emit_friends();
                }
                Err(AuthError::RateLimited) => tokio::time::sleep(RATE_LIMIT_PAUSE).await,
                Err(error) => {
                    // 401 が失効ではなかったときは、他の失敗と同じく後で再試行する
                    if matches!(self.handle_error(error).await, AuthError::Unauthorized) {
                        return;
                    }
                    self.with_store(|store| store.world_retry_at.insert(world_id, Instant::now() + WORLD_RETRY_DELAY));
                }
            }
        }
    }
}

pub struct SocialState {
    store: Arc<Mutex<SocialStore>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    /// ログイン中のセッション。コマンドからの API 呼び出しに使う
    session: Mutex<Option<Session>>,
}

impl SocialState {
    pub fn new() -> Self {
        Self { store: Arc::default(), tasks: Mutex::default(), session: Mutex::default() }
    }

    /// ログイン完了時に呼ぶ。保存済みのグループインスタンスの観測記録を復元し、
    /// 初期同期・pipeline・World 名取得・バックグラウンド中の定期取得のタスクを起動する
    pub fn start(&self, app: AppHandle, client: VrchatClient, user_id: String) {
        self.stop();
        let (instance_seen, next_created_order) = instance_store::load(&app, &user_id);
        let generation = {
            let mut store = self.store.lock().expect("social store poisoned");
            store.instance_seen = instance_seen;
            store.next_created_order = next_created_order;
            store.generation
        };
        let session = Session { app, client, user_id, store: self.store.clone(), generation };

        let initial_sync = session.clone();
        let tasks = vec![
            tauri::async_runtime::spawn(async move {
                initial_sync.sync_friends().await;
                initial_sync.sync_groups().await;
            }),
            tauri::async_runtime::spawn(session.clone().run_pipeline()),
            tauri::async_runtime::spawn(session.clone().run_world_queue()),
            tauri::async_runtime::spawn(session.clone().run_background_group_poll()),
        ];
        *self.tasks.lock().expect("social tasks poisoned") = tasks;
        *self.session.lock().expect("social session poisoned") = Some(session);
    }

    /// ログアウト時に呼ぶ。全タスクを止めて状態を破棄する
    pub fn stop(&self) {
        for task in self.tasks.lock().expect("social tasks poisoned").drain(..) {
            task.abort();
        }
        *self.session.lock().expect("social session poisoned") = None;
        let mut store = self.store.lock().expect("social store poisoned");
        let generation = store.generation + 1;
        *store = SocialStore { generation, ..SocialStore::default() };
    }

    pub fn friend_views(&self) -> Vec<FriendView> {
        self.store.lock().expect("social store poisoned").friend_views()
    }

    pub fn group_views(&self) -> Vec<GroupView> {
        self.store.lock().expect("social store poisoned").groups.clone()
    }

    /// 所属する全グループのインスタンスを取得する。ログアウト後に完了した結果は捨てる
    pub async fn fetch_group_instances(&self) -> Result<GroupInstanceListView, AuthError> {
        let session = self.session.lock().expect("social session poisoned").clone().ok_or(AuthError::Unauthorized)?;
        session.fetch_group_instances().await
    }

    /// 指定した 1 グループのインスタンスを取得する。ログアウト後に完了した結果は捨てる
    pub async fn fetch_instances_of_group(&self, group_id: &str) -> Result<Vec<GroupInstanceView>, AuthError> {
        let session = self.session.lock().expect("social session poisoned").clone().ok_or(AuthError::Unauthorized)?;
        let instances = match session.client.get_group_instances(group_id).await {
            Ok(instances) => instances,
            Err(error) => return Err(session.handle_error(error).await),
        };
        let views = session
            .with_store(|store| store.apply_instances_of_group(group_id, instances, now_epoch_ms()))
            .ok_or(AuthError::Unauthorized)?;
        session.persist_instance_seen();
        Ok(views)
    }

    /// フレンドがいるインスタンスの詳細を取得する
    pub async fn fetch_instance_detail(&self, location: &str) -> Result<InstanceDetailView, AuthError> {
        let session = self.session.lock().expect("social session poisoned").clone().ok_or(AuthError::Unauthorized)?;
        session.fetch_instance_detail(location).await
    }

    /// pinned を置き換え、`is_world_loading` が変わるので snapshot を返す
    pub fn set_pinned(&self, ids: Vec<String>) -> Vec<FriendView> {
        let mut store = self.store.lock().expect("social store poisoned");
        store.pinned = ids.into_iter().collect();
        store.friend_views()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vrchat_client::CurrentUser;
    use crate::vrchat_models::{ApiGroupInstanceWorld, ApiWorld};

    fn api_friend(id: &str, status: &str, location: &str) -> ApiFriend {
        ApiFriend {
            id: id.into(),
            display_name: id.to_uppercase(),
            status: status.into(),
            location: location.into(),
            status_description: String::new(),
        }
    }

    fn view<'a>(views: &'a [FriendView], id: &str) -> &'a FriendView {
        views.iter().find(|v| v.id == id).expect("friend not found")
    }

    #[test]
    fn signs_out_only_when_auth_user_confirms_expiry() {
        let user = CurrentUser { id: "usr_1".into(), display_name: "A".into(), icon_url: None };
        assert!(is_session_expired(&Ok(AuthUserResponse::Unauthorized)));
        assert!(is_session_expired(&Ok(AuthUserResponse::RequiresTwoFactor(vec![]))));
        assert!(!is_session_expired(&Ok(AuthUserResponse::SignedIn(user))));
        // 確認できなかったときは、誤って失効扱いにしないようサインアウトさせない
        assert!(!is_session_expired(&Err(AuthError::Network("offline".into()))));
        assert!(!is_session_expired(&Err(AuthError::RateLimited)));
    }

    #[test]
    fn parses_locations() {
        assert_eq!(
            FriendLocation::from_api("wrld_abc:123~group(grp_1)"),
            FriendLocation::World { world_id: "wrld_abc".into(), location: "wrld_abc:123~group(grp_1)".into() }
        );
        assert_eq!(FriendLocation::from_api("private"), FriendLocation::Private);
        assert_eq!(FriendLocation::from_api("traveling"), FriendLocation::Traveling);
        assert_eq!(FriendLocation::from_api("offline"), FriendLocation::Offline);
    }

    #[test]
    fn builds_views_from_initial_sync() {
        let mut store = SocialStore::default();
        store.replace_friends(
            vec![api_friend("a", "join me", "wrld_x:1"), api_friend("b", "active", "offline")],
            vec![api_friend("c", "active", "offline")],
        );
        store.pinned.insert("a".into());
        let views = store.friend_views();
        assert_eq!(view(&views, "a").location_kind, "world");
        assert_eq!(view(&views, "a").location.as_deref(), Some("wrld_x:1"));
        assert!(view(&views, "a").is_world_loading);
        assert_eq!(view(&views, "b").location_kind, "website");
        assert_eq!(view(&views, "c").status, FriendStatus::Offline);
        assert_eq!(store.next_world_to_fetch(Instant::now()).as_deref(), Some("wrld_x"));
    }

    #[test]
    fn applies_pipeline_events() {
        let mut store = SocialStore::default();
        store.replace_friends(vec![], vec![api_friend("a", "ask me", "offline")]);
        store.pinned.insert("a".into());

        store.apply(&PipelineEvent::FriendLocation {
            user_id: "a".into(),
            location: "wrld_y:2".into(),
            user: None,
            world: Some(ApiWorld { id: "wrld_y".into(), name: "Rainy Window".into(), ..ApiWorld::default() }),
        });
        let views = store.friend_views();
        assert_eq!(view(&views, "a").world_name.as_deref(), Some("Rainy Window"));
        assert_eq!(view(&views, "a").status, FriendStatus::Ask);
        assert!(!view(&views, "a").is_world_loading);
        assert_eq!(store.next_world_to_fetch(Instant::now()), None);

        store.apply(&PipelineEvent::FriendOffline { user_id: "a".into() });
        assert_eq!(view(&store.friend_views(), "a").status, FriendStatus::Offline);

        store.apply(&PipelineEvent::FriendDelete { user_id: "a".into() });
        assert!(store.friend_views().is_empty());
        assert!(store.pinned.is_empty());
    }

    #[test]
    fn updates_status_message_from_full_profile_only() {
        let mut store = SocialStore::default();
        let mut user = api_friend("a", "active", "wrld_x:1");
        user.status_description = "おやすみ".into();
        store.replace_friends(vec![user.clone()], vec![]);
        assert_eq!(view(&store.friend_views(), "a").status_message, "おやすみ");

        // status を含まない `user` ではステータスメッセージを変えない
        let partial = ApiFriend { status: String::new(), ..api_friend("a", "", "") };
        store.apply(&PipelineEvent::FriendUpdate { user_id: "a".into(), user: partial });
        assert_eq!(view(&store.friend_views(), "a").status_message, "おやすみ");

        user.status_description = String::new();
        store.apply(&PipelineEvent::FriendUpdate { user_id: "a".into(), user });
        assert_eq!(view(&store.friend_views(), "a").status_message, "");
    }

    #[test]
    fn parses_instance_types() {
        let group = |id: &str| Some(InstanceHost::Group(id.into()));
        let user = |id: &str| Some(InstanceHost::User(id.into()));
        assert_eq!(instance_type_from_location("wrld_a:1~region(jp)"), ("public", None));
        assert_eq!(instance_type_from_location("wrld_a:1~hidden(usr_1)~region(jp)"), ("friendsPlus", user("usr_1")));
        assert_eq!(instance_type_from_location("wrld_a:1~friends(usr_1)~region(jp)"), ("friends", user("usr_1")));
        assert_eq!(
            instance_type_from_location("wrld_a:1~private(usr_1)~canRequestInvite~region(us)"),
            ("invitePlus", user("usr_1"))
        );
        assert_eq!(instance_type_from_location("wrld_a:1~private(usr_1)~region(us)"), ("invite", user("usr_1")));
        assert_eq!(
            instance_type_from_location("wrld_a:1~group(grp_1)~groupAccessType(public)~region(jp)"),
            ("groupPublic", group("grp_1"))
        );
        assert_eq!(
            instance_type_from_location("wrld_a:1~group(grp_1)~groupAccessType(plus)"),
            ("groupPlus", group("grp_1"))
        );
        assert_eq!(
            instance_type_from_location("wrld_a:1~group(grp_1)~groupAccessType(members)"),
            ("group", group("grp_1"))
        );
    }

    #[test]
    fn resolves_known_host_names() {
        let mut store = SocialStore::default();
        store.replace_friends(vec![api_friend("usr_1", "active", "wrld_x:1")], vec![]);
        store.groups = vec![GroupView { id: "grp_1".into(), name: "寝落ち図書館".into() }];
        store.host_names.insert("usr_2".into(), "あおい".into());
        assert_eq!(store.known_host_name(&InstanceHost::Group("grp_1".into())).as_deref(), Some("寝落ち図書館"));
        assert_eq!(store.known_host_name(&InstanceHost::User("usr_1".into())).as_deref(), Some("USR_1"));
        assert_eq!(store.known_host_name(&InstanceHost::User("usr_2".into())).as_deref(), Some("あおい"));
        assert_eq!(store.known_host_name(&InstanceHost::Group("grp_9".into())), None);
    }

    fn api_instance(location: &str, access_type: &str) -> ApiInstance {
        ApiInstance {
            location: location.into(),
            owner_id: "grp_1".into(),
            user_count: 3,
            capacity: 16,
            group_access_type: access_type.into(),
            world: ApiWorld { id: "wrld_a".into(), name: "Quiet Shore".into(), ..ApiWorld::default() },
        }
    }

    #[test]
    fn keeps_first_seen_time_of_group_instances() {
        let mut store = SocialStore::default();
        let views = store.apply_group_instances(vec![api_instance("wrld_a:1", "public"), api_instance("wrld_a:2", "plus")], 100);
        assert_eq!(views.iter().map(|v| v.first_seen_at).collect::<Vec<_>>(), [100, 100]);
        assert_eq!(views[0].access_type, "public");
        assert_eq!(views[1].access_type, "plus");

        let views = store.apply_group_instances(vec![api_instance("wrld_a:2", "plus"), api_instance("wrld_a:3", "members")], 200);
        assert_eq!(views[0].first_seen_at, 100);
        assert_eq!(views[1].first_seen_at, 200);
        assert_eq!(views[1].access_type, "members");
        // 既知の wrld_a:2 は初回の値のまま、初めて見る wrld_a:3 だけ既存より新しい値になる
        assert_eq!(views.iter().map(|v| v.created_order).collect::<Vec<_>>(), [2, 3]);
        assert!(!store.instance_seen.contains_key("wrld_a:1"));
    }

    #[test]
    fn keeps_restored_first_seen_time_after_restart() {
        // 前回起動時に保存した記録を復元した状態
        let mut store = SocialStore {
            instance_seen: HashMap::from([("wrld_a:1".to_owned(), SeenInstance { first_seen_at: 100, created_order: 5 })]),
            next_created_order: 5,
            ..SocialStore::default()
        };
        let views = store.apply_group_instances(vec![api_instance("wrld_a:1", "public"), api_instance("wrld_a:2", "public")], 900);
        assert_eq!((views[0].first_seen_at, views[0].created_order), (100, 5));
        assert_eq!((views[1].first_seen_at, views[1].created_order), (900, 6));
    }

    #[test]
    fn keeps_created_order_of_known_instances_on_group_refresh() {
        let mut store = SocialStore::default();
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public"), api_instance("wrld_a:2~group(grp_1)", "public")], 100);

        // 1 グループ分の API はレスポンス順が異なっても既知の順序を変えない
        let views = store.apply_instances_of_group(
            "grp_1",
            vec![
                api_group_instance("wrld_a:1~group(grp_1)", 7),
                api_group_instance("wrld_a:3~group(grp_1)", 1),
                api_group_instance("wrld_a:2~group(grp_1)", 9),
            ],
            200,
        );
        let order = |id: &str| views.iter().find(|v| v.id == id).unwrap().created_order;
        assert_eq!((order("wrld_a:1~group(grp_1)"), order("wrld_a:2~group(grp_1)"), order("wrld_a:3~group(grp_1)")), (1, 2, 3));
        assert_eq!(views.iter().find(|v| v.id == "wrld_a:1~group(grp_1)").unwrap().user_count, 7);
    }

    fn api_group_instance(location: &str, member_count: u32) -> ApiGroupInstance {
        ApiGroupInstance {
            location: location.into(),
            member_count,
            world: ApiGroupInstanceWorld { name: "Quiet Shore".into(), capacity: 60 },
        }
    }

    #[test]
    fn parses_location_params() {
        let location = "wrld_a:1~group(grp_1)~groupAccessType(plus)~region(us)";
        assert_eq!(location_param(location, "group"), Some("grp_1"));
        assert_eq!(location_param(location, "groupAccessType"), Some("plus"));
        assert_eq!(location_param(location, "private"), None);
    }

    #[test]
    fn replaces_instances_of_one_group_only() {
        let mut store = SocialStore::default();
        let mut other = api_instance("wrld_a:9~group(grp_2)", "public");
        other.owner_id = "grp_2".into();
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public"), other], 100);

        let views = store.apply_instances_of_group(
            "grp_1",
            vec![api_group_instance("wrld_a:2~group(grp_1)~groupAccessType(plus)", 5)],
            200,
        );
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].group_id, "grp_1");
        assert_eq!(views[0].access_type, "plus");
        assert_eq!((views[0].user_count, views[0].capacity), (5, 60));
        assert_eq!(views[0].first_seen_at, 200);
        // grp_1 の閉じたインスタンスの記録だけ消え、grp_2 の記録は残る
        assert!(!store.instance_seen.contains_key("wrld_a:1~group(grp_1)"));
        assert_eq!(store.instance_seen.get("wrld_a:9~group(grp_2)").map(|seen| seen.first_seen_at), Some(100));
    }

    #[test]
    fn drops_non_group_instances() {
        let mut store = SocialStore::default();
        let mut not_group = api_instance("wrld_a:1", "public");
        not_group.owner_id = "usr_1".into();
        assert!(store.apply_group_instances(vec![not_group], 100).is_empty());
    }

    #[test]
    fn parses_rfc3339_fetched_at() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_ms("2024-05-01T12:34:56.789Z"), Some(1_714_566_896_789));
        assert_eq!(parse_rfc3339_ms("2024-05-01T12:34:56Z"), Some(1_714_566_896_000));
        assert_eq!(parse_rfc3339_ms("2024-05-01T12:34:56.7Z"), Some(1_714_566_896_700));
        assert_eq!(parse_rfc3339_ms("2024-05-01T12:34:56.789123Z"), Some(1_714_566_896_789));
        assert_eq!(parse_rfc3339_ms("2024-05-01T21:34:56.789+09:00"), Some(1_714_566_896_789));
        assert_eq!(parse_rfc3339_ms("2024-05-01T07:04:56.789-05:30"), Some(1_714_566_896_789));
        assert_eq!(parse_rfc3339_ms("2024-02-29T00:00:00Z"), Some(1_709_164_800_000));
    }

    #[test]
    fn rejects_malformed_fetched_at() {
        for text in ["", "2024-05-01", "2024-05-01T12:34:56", "2024-13-01T00:00:00Z", "2024-05-01T12:34:56.Z", "2024-05-01T12:34:56+0900", "2024-05-01T12:34:5xZ"] {
            assert_eq!(parse_rfc3339_ms(text), None, "{text}");
        }
    }

    #[test]
    fn schedules_next_group_poll_from_fetched_at() {
        let now = 1_000_000;
        assert_eq!(group_instances_refresh_delay(now, now), Duration::from_secs(90));
        assert_eq!(group_instances_refresh_delay(now - 30_000, now), Duration::from_secs(60));
        // 時計のずれで予定が過ぎていても連続取得しない
        assert_eq!(group_instances_refresh_delay(now - 120_000, now), Duration::from_secs(10));
        // fetchedAt が未来でも取得間隔より長くは待たない
        assert_eq!(group_instances_refresh_delay(now + 60_000, now), Duration::from_secs(90));
    }

    #[test]
    fn skips_worlds_waiting_for_retry() {
        let mut store = SocialStore::default();
        store.replace_friends(vec![api_friend("a", "active", "wrld_z:3")], vec![]);
        store.pinned.insert("a".into());
        let now = Instant::now();
        store.world_retry_at.insert("wrld_z".into(), now + WORLD_RETRY_DELAY);
        assert_eq!(store.next_world_to_fetch(now), None);
        assert_eq!(store.next_world_to_fetch(now + WORLD_RETRY_DELAY).as_deref(), Some("wrld_z"));
    }
}
