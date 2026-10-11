//! Friends / Groups の状態を Rust 側で保持し、REST の初期同期と pipeline イベントで更新する。
//! 状態が変わるたびに整形済みの snapshot をフロントエンドへ emit する。

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Notify};

use crate::auth_log;
use crate::background;
use crate::instance_store::{self, StoredInstances};
use crate::pipeline::{self, PipelineEvent};
use crate::vrchat_client::{AuthError, AuthUserResponse, VrchatClient, FRIENDS_PAGE_SIZE};
use crate::vrchat_models::{ApiFriend, ApiGroupInstance, ApiInstance, ApiInstanceDetail};

pub const FRIENDS_UPDATED_EVENT: &str = "social://friends-updated";
pub const GROUPS_UPDATED_EVENT: &str = "social://groups-updated";
pub const SESSION_EXPIRED_EVENT: &str = "social://session-expired";
pub const LAST_JOINED_UPDATED_EVENT: &str = "social://last-joined-updated";
pub const GROUP_INSTANCES_UPDATED_EVENT: &str = "social://group-instances-updated";

/// World 名の取得は 1 秒に 1 件まで
const WORLD_FETCH_INTERVAL: Duration = Duration::from_secs(1);
/// 取得に失敗した World を再試行するまでの間隔
const WORLD_RETRY_DELAY: Duration = Duration::from_secs(30);
/// 429 を受けたときに World 名の取得を止める時間
const RATE_LIMIT_PAUSE: Duration = Duration::from_secs(60);
/// fetchedAt から次にグループインスタンスを全体取得するまでの間隔
const GROUP_INSTANCES_REFRESH_INTERVAL: Duration = Duration::from_secs(120);
/// 端末と VRChat の時計のずれで fetchedAt + 間隔 が過ぎていても、連続取得しないよう最低限空ける時間
const GROUP_INSTANCES_MIN_REFRESH_DELAY: Duration = Duration::from_secs(10);
/// 取得予定を過ぎても取得条件がそろわない間、条件を確かめ直す間隔 (ウィンドウの再表示に追従するため)
const GROUP_POLL_RECHECK_INTERVAL: Duration = Duration::from_secs(1);
/// 自分が最後にインスタンスに入った時刻を残す期間。経過がこれ以上になった記録は破棄する (表示は最大 23h59m)
const LAST_JOINED_TTL_MS: i64 = 24 * 60 * 60 * 1000;

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

/// グループインスタンス一覧の snapshot。状態が変わるたびにフロントエンドへ emit する
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInstancesView {
    /// emit のたびに進める。フロントエンドが古い snapshot で上書きしないために使う
    revision: u64,
    /// 全グループ分の取得が 1 度も成功していない間は None
    instances: Option<Vec<GroupInstanceView>>,
    is_loading: bool,
    /// 全グループ分が未取得のまま取得に失敗した
    has_load_failed: bool,
    /// 最後に取得結果を反映した時刻 (epoch ms)
    updated_at: Option<i64>,
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone)]
struct ObservedGroupInstance {
    location: String,
    group_id: String,
    world_name: String,
    access_type: &'static str,
    user_count: u32,
    capacity: u32,
}

impl ObservedGroupInstance {
    /// location や World 名が欠けたものは表示できないので捨てる
    fn is_displayable(&self) -> bool {
        !self.location.is_empty() && !self.world_name.is_empty()
    }
}

/// 全グループ分の取得 (`/users/{id}/instances/groups`) の結果
struct FullGroupInstances {
    /// VRChat 側でこの一覧が取得された時刻 (epoch ms)
    fetched_at: i64,
    /// group ID → インスタンス (API の返却順)
    by_group: HashMap<String, Vec<ObservedGroupInstance>>,
}

/// 1 グループ分の取得 (`/groups/{id}/instances`) の結果
struct GroupRefresh {
    /// レスポンスを受け取った時刻 (epoch ms)。この API には取得時刻が無いので、受け取った時点を取得時刻とする。
    /// 全グループ分の `fetched_at` と比べるため、VRChat の時計に換算した値を持つ
    fetched_at: i64,
    instances: Vec<ObservedGroupInstance>,
}

/// グループインスタンスを定期取得するか。
/// ウィンドウ表示中は展開中のグループがあるときだけ、ウィンドウが見えていない間は Run in background が有効なときだけ取得する
fn should_fetch_group_instances(has_open_group: bool, conditions: &background::PollConditions) -> bool {
    (has_open_group && conditions.is_window_shown()) || conditions.should_poll()
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

/// フロントエンドから呼ばれた取得コマンドの結果と所要時間を記録する
fn log_command_result(name: &str, started_at: Instant, result: Result<usize, &AuthError>) {
    let elapsed_ms = started_at.elapsed().as_millis();
    match result {
        Ok(count) => auth_log::log!("command: {name} -> ok, {count} items ({elapsed_ms} ms)"),
        Err(error) => auth_log::log!("command: {name} -> failed: {error} ({elapsed_ms} ms)"),
    }
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
    /// グループインスタンスの location → 自分が最後に入った時刻 (epoch ms)
    last_joined: HashMap<String, i64>,
    /// 全グループ分の取得結果。未取得なら None
    full_list: Option<FullGroupInstances>,
    /// group ID → 1 グループ分の取得結果。全グループ分の取得より新しいものだけ表示に使う
    group_refreshes: HashMap<String, GroupRefresh>,
    /// フロントエンドで表示中かつ展開中のグループ
    open_group_ids: HashSet<String>,
    /// グループインスタンスを取得中。取得は同時に 1 つまで
    is_fetching_group_instances: bool,
    /// 全グループ分が未取得のまま取得に失敗した
    has_group_instances_load_failed: bool,
    /// 最後に取得結果を反映した時刻 (epoch ms)
    group_instances_updated_at: Option<i64>,
    /// 端末の時計 − VRChat の時計 (ms)。全グループ分の取得で観測した「受け取った時刻 − fetchedAt」の最小値。
    /// キャッシュの古さや通信時間は大きくする方向にしか乗らないので、最小値を時計のずれとみなす
    vrchat_clock_offset_ms: Option<i64>,
    /// 次に全グループ分を定期取得する時刻。None はすぐ取得する
    next_group_poll_at: Option<Instant>,
    /// グループインスタンスの snapshot を emit するたびに進める
    group_instances_revision: u64,
}

impl SocialStore {
    /// 自分がインスタンスに入ったことを記録する。グループインスタンス以外は記録せず、記録したときだけ true
    fn record_joined(&mut self, location: &str, now_ms: i64) -> bool {
        if location_param(location, "group").is_none() {
            return false;
        }
        self.last_joined.insert(location.to_owned(), now_ms);
        self.prune_last_joined(now_ms);
        true
    }

    /// 入ってから 24 時間以上経った記録を破棄する
    fn prune_last_joined(&mut self, now_ms: i64) {
        self.last_joined.retain(|_, joined_at| now_ms - *joined_at < LAST_JOINED_TTL_MS);
    }


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

    /// 全グループ分の取得結果を反映する。`fetched_at` は VRChat 側の取得時刻で、読み取れなければ None (受け取った時刻で代用する)。
    /// これより古い 1 グループ分の取得結果は不要になるので破棄する
    fn apply_group_instances(&mut self, instances: Vec<ApiInstance>, fetched_at: Option<i64>, now_ms: i64) {
        if let Some(fetched_at) = fetched_at {
            let offset = now_ms - fetched_at;
            self.vrchat_clock_offset_ms = Some(self.vrchat_clock_offset_ms.map_or(offset, |current| current.min(offset)));
        }
        let fetched_at = fetched_at.unwrap_or(now_ms);
        let observed: Vec<_> = instances
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
            .filter(ObservedGroupInstance::is_displayable)
            .collect();
        self.record_first_seen(&observed, now_ms);
        let mut by_group: HashMap<String, Vec<ObservedGroupInstance>> = HashMap::new();
        for inst in observed {
            by_group.entry(inst.group_id.clone()).or_default().push(inst);
        }
        self.group_refreshes.retain(|_, refresh| refresh.fetched_at > fetched_at);
        self.full_list = Some(FullGroupInstances { fetched_at, by_group });
        self.has_group_instances_load_failed = false;
        self.group_instances_updated_at = Some(now_ms);
        self.prune_instance_seen();
    }

    /// 1 グループ分の取得結果を反映する。受け取った時刻 `now_ms` を VRChat の時計に換算して取得時刻とし、
    /// 全グループ分の取得結果より新しい間はこのグループの表示に使う
    fn apply_instances_of_group(&mut self, group_id: &str, instances: Vec<ApiGroupInstance>, now_ms: i64) {
        let observed: Vec<_> = instances
            .into_iter()
            .map(|inst| ObservedGroupInstance {
                access_type: group_access_type_from_api(location_param(&inst.location, "groupAccessType").unwrap_or_default()),
                location: inst.location,
                group_id: group_id.to_owned(),
                world_name: inst.world.name,
                user_count: inst.member_count,
                capacity: inst.world.capacity,
            })
            .filter(ObservedGroupInstance::is_displayable)
            .collect();
        self.record_first_seen(&observed, now_ms);
        let fetched_at = now_ms - self.vrchat_clock_offset_ms.unwrap_or(0);
        self.group_refreshes.insert(group_id.to_owned(), GroupRefresh { fetched_at, instances: observed });
        self.group_instances_updated_at = Some(now_ms);
        self.prune_instance_seen();
    }

    /// 初めて見るインスタンスに観測時刻と Created ソート用の値を振る。観測済みのインスタンスは記録済みの値を変えない。
    /// API はおおむね作成が古い順に返すので、先頭から順に値を振り、末尾ほど新しくする
    fn record_first_seen(&mut self, instances: &[ObservedGroupInstance], now_ms: i64) {
        for inst in instances {
            if !self.instance_seen.contains_key(&inst.location) {
                self.next_created_order += 1;
                let seen = SeenInstance { first_seen_at: now_ms, created_order: self.next_created_order };
                self.instance_seen.insert(inst.location.clone(), seen);
            }
        }
    }

    /// 表示中でなくなった (閉じた・古い取得結果にしか無い) インスタンスの観測記録を破棄する。
    /// 全グループ分が未取得の間は、前回起動時から引き継いだ記録を残す
    fn prune_instance_seen(&mut self) {
        let Some(displayed) = self.displayed_group_instances() else {
            return;
        };
        let locations: HashSet<String> = displayed.into_iter().map(|inst| inst.location.clone()).collect();
        self.instance_seen.retain(|location, _| locations.contains(location));
    }

    /// 表示するインスタンス。グループごとに、全グループ分と 1 グループ分の取得結果のうち新しいほうを使う。
    /// 全グループ分が未取得なら None
    fn displayed_group_instances(&self) -> Option<Vec<&ObservedGroupInstance>> {
        let full = self.full_list.as_ref()?;
        let mut group_ids: Vec<&String> = full.by_group.keys().chain(self.group_refreshes.keys()).collect();
        group_ids.sort();
        group_ids.dedup();
        let instances = group_ids
            .into_iter()
            .flat_map(|group_id| match self.group_refreshes.get(group_id).filter(|refresh| refresh.fetched_at > full.fetched_at) {
                Some(refresh) => refresh.instances.as_slice(),
                None => full.by_group.get(group_id).map_or(&[][..], Vec::as_slice),
            })
            .collect();
        Some(instances)
    }

    fn group_instances_view(&self) -> GroupInstancesView {
        let instances = self.displayed_group_instances().map(|instances| {
            instances
                .into_iter()
                .map(|inst| {
                    let seen = self.instance_seen.get(&inst.location).copied().unwrap_or_default();
                    GroupInstanceView {
                        id: inst.location.clone(),
                        group_id: inst.group_id.clone(),
                        world_name: inst.world_name.clone(),
                        access_type: inst.access_type,
                        user_count: inst.user_count,
                        capacity: inst.capacity,
                        first_seen_at: seen.first_seen_at,
                        created_order: seen.created_order,
                    }
                })
                .collect()
        });
        GroupInstancesView {
            revision: self.group_instances_revision,
            instances,
            is_loading: self.is_fetching_group_instances,
            has_load_failed: self.has_group_instances_load_failed,
            updated_at: self.group_instances_updated_at,
        }
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
            // 入室の記録は Friends の snapshot に関係しないので、Session 側で処理する
            PipelineEvent::Connected | PipelineEvent::GroupsChanged | PipelineEvent::UserLocation { .. } => return false,
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
    /// 展開中のグループが変わったときに、グループインスタンスの定期取得を起こす
    group_poll_notify: Arc<Notify>,
    /// 観測記録の snapshot 作成から保存までを直列化し、古い snapshot が後から書かれないようにする
    persist_lock: Arc<Mutex<()>>,
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

    fn emit_last_joined(&self) {
        if let Some(last_joined) = self.with_store(|store| store.last_joined.clone()) {
            let _ = self.app.emit(LAST_JOINED_UPDATED_EVENT, last_joined);
        }
    }

    fn emit_group_instances(&self) {
        let view = self.with_store(|store| {
            store.group_instances_revision += 1;
            store.group_instances_view()
        });
        if let Some(view) = view {
            let _ = self.app.emit(GROUP_INSTANCES_UPDATED_EVENT, view);
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
                auth_log::log!("sync: friends online={} offline={}", online.len(), offline.len());
                self.with_store(|store| store.replace_friends(online, offline));
                self.emit_friends();
            }
            Err(error) => {
                auth_log::log!("sync: friends failed: {error}");
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
                auth_log::log!("sync: groups count={}", groups.len());
                self.with_store(|store| store.groups = groups);
                self.emit_groups();
            }
            Err(error) => {
                auth_log::log!("sync: groups failed: {error}");
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
                        auth_log::log!("pipeline: reconnected; resyncing friends and groups");
                        self.sync_friends().await;
                        self.sync_groups().await;
                    }
                    has_connected = true;
                }
                PipelineEvent::GroupsChanged => {
                    auth_log::log!("pipeline: groups changed; resyncing groups");
                    self.sync_groups().await;
                }
                PipelineEvent::UserLocation { location } => {
                    let is_recorded = self.with_store(|store| store.record_joined(location, now_epoch_ms())) == Some(true);
                    auth_log::log!("pipeline: own location changed to {location}; recorded as a group instance join={is_recorded}");
                    if is_recorded {
                        self.emit_last_joined();
                        self.persist_instance_seen();
                    }
                }
                _ => {
                    if self.with_store(|store| store.apply(&event)) == Some(true) {
                        self.emit_friends();
                    }
                }
            }
        }
    }

    /// グループインスタンスの取得を 1 つずつ実行する。取得の前後で読み込み中の状態を emit する。
    /// ほかの取得が実行中 (またはサインアウト後) なら実行せず None を返す
    async fn run_group_instances_fetch<T>(&self, fetch: impl Future<Output = Result<T, AuthError>>) -> Option<Result<T, AuthError>> {
        if self.with_store(|store| std::mem::replace(&mut store.is_fetching_group_instances, true)) != Some(false) {
            return None;
        }
        self.emit_group_instances();
        let result = fetch.await;
        self.with_store(|store| store.is_fetching_group_instances = false);
        self.emit_group_instances();
        Some(result)
    }

    /// 所属する全グループのインスタンスを取得して反映し、VRChat 側の取得時刻 (fetchedAt) と表示する件数を返す。
    /// 成功したら次の定期取得の予定もここで決める。ログアウト後に完了した結果は捨てる
    async fn fetch_group_instances(&self) -> Result<(i64, usize), AuthError> {
        let list = match self.client.get_user_group_instances(&self.user_id).await {
            Ok(list) => list,
            Err(error) => {
                self.with_store(|store| store.has_group_instances_load_failed = store.full_list.is_none());
                return Err(self.handle_error(error).await);
            }
        };
        let now_ms = now_epoch_ms();
        let parsed = parse_rfc3339_ms(&list.fetched_at);
        let fetched_at = parsed.unwrap_or(now_ms);
        let next_poll_in = group_instances_refresh_delay(fetched_at, now_ms);
        let next_in = next_poll_in.as_secs_f64();
        let message = match parsed {
            Some(ms) => format!(
                "[group-instances] fetchedAt={} (raw={:?}, age={:.1}s, next in {next_in:.1}s)",
                format_local_time(ms),
                list.fetched_at,
                (now_ms - ms) as f64 / 1000.0,
            ),
            None => format!("[group-instances] fetchedAt={:?} (unparsed; using local time, next in {next_in:.1}s)", list.fetched_at),
        };
        eprintln!("{message}");
        auth_log::log!("{message}");
        let received = list.instances.len();
        let applied = self.with_store(|store| {
            let previous_offset = store.vrchat_clock_offset_ms;
            store.apply_group_instances(list.instances, parsed, now_ms);
            store.next_group_poll_at = Some(Instant::now() + next_poll_in);
            let offset = store.vrchat_clock_offset_ms.filter(|offset| previous_offset != Some(*offset));
            (store.displayed_group_instances().map_or(0, |instances| instances.len()), offset)
        });
        let Some((applied, updated_offset)) = applied else {
            auth_log::log!("group instances: result discarded (signed out while fetching)");
            return Err(AuthError::Unauthorized);
        };
        if let Some(offset) = updated_offset {
            auth_log::log!("group instances: clock offset (local - VRChat) updated to {:.1}s", offset as f64 / 1000.0);
        }
        auth_log::log!("group instances: {received} received, {applied} displayed");
        self.persist_instance_seen();
        Ok((fetched_at, applied))
    }

    /// 指定した 1 グループのインスタンスを取得して反映し、受け取った件数を返す。ログアウト後に完了した結果は捨てる
    async fn fetch_instances_of_group(&self, group_id: &str) -> Result<usize, AuthError> {
        let instances = match self.client.get_group_instances(group_id).await {
            Ok(instances) => instances,
            Err(error) => return Err(self.handle_error(error).await),
        };
        let received = instances.len();
        let offset = self
            .with_store(|store| {
                store.apply_instances_of_group(group_id, instances, now_epoch_ms());
                store.vrchat_clock_offset_ms
            })
            .ok_or(AuthError::Unauthorized)?;
        auth_log::log!(
            "group instances: {group_id} fetched, compared with all groups using clock offset {:.1}s",
            offset.unwrap_or(0) as f64 / 1000.0
        );
        self.persist_instance_seen();
        Ok(received)
    }

    /// 観測済みのグループインスタンスの記録を保存する。
    /// snapshot の作成から書き込みまでを persist_lock で直列化し、作った順に保存する (store のロックは snapshot 作成中だけ取る)
    fn persist_instance_seen(&self) {
        let _guard = self.persist_lock.lock().expect("persist lock poisoned");
        let stored = self.with_store(|store| StoredInstances {
            seen: store.instance_seen.clone(),
            next_created_order: store.next_created_order,
            last_joined: store.last_joined.clone(),
        });
        if let Some(stored) = stored {
            instance_store::save(&self.app, &self.user_id, stored);
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

    /// 全グループ分のグループインスタンスを定期取得する。取得するのは、ウィンドウ表示中で展開中のグループがあるときと、
    /// Run in background でウィンドウが見えていない間 (初めて観測した時刻を記録し続けるため)。
    /// 次の取得は fetchedAt + 取得間隔 に行い、取得できなかったときは取得間隔だけ待つ
    async fn run_group_instance_poll(self) {
        // 取得予定を過ぎたのに取得条件がそろわない状態か。変化したときだけログに残す
        let mut is_paused = false;
        loop {
            let Some((next_at, has_open_group)) = self.with_store(|store| (store.next_group_poll_at, !store.open_group_ids.is_empty())) else {
                return;
            };
            let now = Instant::now();
            let wait = match next_at {
                Some(at) if at > now => at - now,
                _ => {
                    let conditions = background::poll_conditions(&self.app);
                    if should_fetch_group_instances(has_open_group, &conditions) {
                        if is_paused {
                            auth_log::log!("group poll: resumed ({conditions:?}, open groups: {has_open_group})");
                            is_paused = false;
                        }
                        if !self.poll_group_instances(next_at.map(|at| now - at)).await {
                            return;
                        }
                        continue;
                    }
                    if !is_paused {
                        auth_log::log!("group poll: paused ({conditions:?}, open groups: {has_open_group})");
                        is_paused = true;
                    }
                    GROUP_POLL_RECHECK_INTERVAL
                }
            };
            let slept_from = Instant::now();
            let wall_from = now_epoch_ms();
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = self.group_poll_notify.notified() => {}
            }
            // 単調時計と壁時計の進みが大きく違うときは、スリープからの復帰や時計の変更を疑えるよう記録する
            let clock_skew_ms = (now_epoch_ms() - wall_from) - slept_from.elapsed().as_millis() as i64;
            if clock_skew_ms.abs() > 5_000 {
                auth_log::log!(
                    "group poll: wall clock advanced {:+.1}s vs monotonic while waiting (system sleep or clock change?)",
                    clock_skew_ms as f64 / 1000.0
                );
            }
        }
    }

    /// 定期取得の 1 回分。結果に応じて次の取得予定を決める (成功時は fetch_group_instances が決める)。
    /// セッションが失効して定期取得を止めるときは false
    async fn poll_group_instances(&self, overdue: Option<Duration>) -> bool {
        let overdue = overdue.map_or_else(|| "first fetch".to_owned(), |d| format!("overdue by {:.1}s", d.as_secs_f64()));
        auth_log::log!("group poll: fetching ({overdue})");
        let fetch_started = Instant::now();
        let Some(result) = self.run_group_instances_fetch(self.fetch_group_instances()).await else {
            // 手動更新の取得中。終わったあとに予定を確かめ直す
            self.with_store(|store| {
                store.next_group_poll_at.get_or_insert(Instant::now() + GROUP_POLL_RECHECK_INTERVAL);
            });
            auth_log::log!("group poll: skipped (another fetch is running)");
            return true;
        };
        let fetch_ms = fetch_started.elapsed().as_millis();
        let retry_in = match result {
            Ok(_) => {
                auth_log::log!("group poll: fetched in {fetch_ms} ms");
                return true;
            }
            Err(AuthError::RateLimited) => {
                auth_log::log!("group poll: rate limited after {fetch_ms} ms; pausing");
                RATE_LIMIT_PAUSE + GROUP_INSTANCES_REFRESH_INTERVAL
            }
            Err(AuthError::Unauthorized) => {
                auth_log::log!("group poll: stopped (unauthorized) after {fetch_ms} ms");
                return false;
            }
            Err(error) => {
                auth_log::log!("group poll: failed after {fetch_ms} ms: {error}; retrying later");
                GROUP_INSTANCES_REFRESH_INTERVAL
            }
        };
        self.with_store(|store| store.next_group_poll_at = Some(Instant::now() + retry_in));
        true
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
                Err(AuthError::RateLimited) => {
                    auth_log::log!("world queue: rate limited; pausing {}s", RATE_LIMIT_PAUSE.as_secs());
                    tokio::time::sleep(RATE_LIMIT_PAUSE).await;
                }
                Err(error) => {
                    auth_log::log!("world queue: failed to fetch {world_id}: {error}");
                    // 401 が失効ではなかったときは、他の失敗と同じく後で再試行する
                    if matches!(self.handle_error(error).await, AuthError::Unauthorized) {
                        auth_log::log!("world queue: stopped (unauthorized)");
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
    /// 観測記録の保存を直列化する。セッションをまたいでも共有する
    persist_lock: Arc<Mutex<()>>,
}

impl SocialState {
    pub fn new() -> Self {
        Self { store: Arc::default(), tasks: Mutex::default(), session: Mutex::default(), persist_lock: Arc::default() }
    }

    /// ログイン完了時に呼ぶ。保存済みのグループインスタンスの観測記録を復元し、
    /// 初期同期・pipeline・World 名取得・グループインスタンスの定期取得のタスクを起動する
    pub fn start(&self, app: AppHandle, client: VrchatClient, user_id: String) {
        self.stop();
        let stored = instance_store::load(&app, &user_id);
        let generation = {
            let mut store = self.store.lock().expect("social store poisoned");
            store.instance_seen = stored.seen;
            store.next_created_order = stored.next_created_order;
            store.last_joined = stored.last_joined;
            store.prune_last_joined(now_epoch_ms());
            store.generation
        };
        let session = Session {
            app,
            client,
            user_id,
            store: self.store.clone(),
            generation,
            group_poll_notify: Arc::default(),
            persist_lock: self.persist_lock.clone(),
        };

        auth_log::log!("social: started user={} generation={generation}", session.user_id);
        let initial_sync = session.clone();
        let tasks = vec![
            auth_log::spawn_logged("initial sync", async move {
                initial_sync.sync_friends().await;
                initial_sync.sync_groups().await;
            }),
            auth_log::spawn_logged("pipeline", session.clone().run_pipeline()),
            auth_log::spawn_logged("world queue", session.clone().run_world_queue()),
            auth_log::spawn_logged("group instance poll", session.clone().run_group_instance_poll()),
        ];
        *self.tasks.lock().expect("social tasks poisoned") = tasks;
        *self.session.lock().expect("social session poisoned") = Some(session);
    }

    /// ログアウト時に呼ぶ。全タスクを止めて状態を破棄する
    pub fn stop(&self) {
        let mut tasks = self.tasks.lock().expect("social tasks poisoned");
        if !tasks.is_empty() {
            auth_log::log!("social: stopping {} tasks", tasks.len());
        }
        for task in tasks.drain(..) {
            task.abort();
        }
        drop(tasks);
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

    /// 自分が最後にグループインスタンスに入った時刻 (location → epoch ms)。24 時間以上前の記録は返さない
    pub fn last_joined_views(&self) -> HashMap<String, i64> {
        let mut store = self.store.lock().expect("social store poisoned");
        store.prune_last_joined(now_epoch_ms());
        store.last_joined.clone()
    }

    /// 現在のグループインスタンスの snapshot。listen 開始前に emit された分を取りこぼさないよう、初期表示時に使う
    pub fn group_instances_view(&self) -> GroupInstancesView {
        self.store.lock().expect("social store poisoned").group_instances_view()
    }

    /// フロントエンドで表示中かつ展開中のグループを置き換え、グループインスタンスの定期取得に条件を確かめ直させる
    pub fn set_open_groups(&self, group_ids: Vec<String>) {
        let Some(session) = self.session.lock().expect("social session poisoned").clone() else {
            return;
        };
        let group_ids: HashSet<String> = group_ids.into_iter().collect();
        let count = group_ids.len();
        let is_changed = session.with_store(|store| std::mem::replace(&mut store.open_group_ids, group_ids) != store.open_group_ids);
        if is_changed == Some(true) {
            auth_log::log!("group poll: open groups changed ({count} open)");
            session.group_poll_notify.notify_one();
        }
    }

    /// 押されたグループだけ再取得する。全グループ分が未取得なら全体取得にフォールバックする。
    /// ほかの取得が実行中なら何もしない。ログアウト後に完了した結果は捨てる
    pub async fn refresh_group(&self, group_id: &str) -> Result<(), AuthError> {
        let name = format!("refresh group {group_id}");
        let Some(session) = self.session.lock().expect("social session poisoned").clone() else {
            auth_log::log!("command: {name} -> no session");
            return Err(AuthError::Unauthorized);
        };
        let has_full_list = session.with_store(|store| store.full_list.is_some()).ok_or(AuthError::Unauthorized)?;
        let target = if has_full_list { group_id } else { "all groups (nothing loaded yet)" };
        auth_log::log!("command: {name} (manual refresh from the UI), fetching {target}");
        let started_at = Instant::now();
        let result = if has_full_list {
            session.run_group_instances_fetch(session.fetch_instances_of_group(group_id)).await
        } else {
            session.run_group_instances_fetch(async { session.fetch_group_instances().await.map(|(_, count)| count) }).await
        };
        let Some(result) = result else {
            auth_log::log!("command: {name} -> skipped (another fetch is running)");
            return Ok(());
        };
        log_command_result(&name, started_at, result.as_ref().map(|count| *count));
        result.map(|_| ())
    }

    /// フレンドがいるインスタンスの詳細を取得する
    pub async fn fetch_instance_detail(&self, location: &str) -> Result<InstanceDetailView, AuthError> {
        let name = format!("fetch instance detail {location}");
        let Some(session) = self.session.lock().expect("social session poisoned").clone() else {
            auth_log::log!("command: {name} -> no session");
            return Err(AuthError::Unauthorized);
        };
        let started_at = Instant::now();
        let result = session.fetch_instance_detail(location).await;
        log_command_result(&name, started_at, result.as_ref().map(|_| 1));
        result
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

    fn api_group_instance(location: &str, member_count: u32) -> ApiGroupInstance {
        ApiGroupInstance {
            location: location.into(),
            member_count,
            world: ApiGroupInstanceWorld { name: "Quiet Shore".into(), capacity: 60 },
        }
    }

    fn group_views(store: &SocialStore) -> Vec<GroupInstanceView> {
        store.group_instances_view().instances.expect("group instances not loaded")
    }

    fn group_view<'a>(views: &'a [GroupInstanceView], id: &str) -> &'a GroupInstanceView {
        views.iter().find(|v| v.id == id).expect("group instance not found")
    }

    #[test]
    fn keeps_first_seen_time_of_group_instances() {
        let mut store = SocialStore::default();
        store.apply_group_instances(vec![api_instance("wrld_a:1", "public"), api_instance("wrld_a:2", "plus")], Some(100), 100);
        let views = group_views(&store);
        assert_eq!(views.iter().map(|v| v.first_seen_at).collect::<Vec<_>>(), [100, 100]);
        assert_eq!(views[0].access_type, "public");
        assert_eq!(views[1].access_type, "plus");

        store.apply_group_instances(vec![api_instance("wrld_a:2", "plus"), api_instance("wrld_a:3", "members")], Some(200), 200);
        let views = group_views(&store);
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
        store.apply_group_instances(vec![api_instance("wrld_a:1", "public"), api_instance("wrld_a:2", "public")], Some(900), 900);
        let views = group_views(&store);
        assert_eq!((views[0].first_seen_at, views[0].created_order), (100, 5));
        assert_eq!((views[1].first_seen_at, views[1].created_order), (900, 6));
    }

    #[test]
    fn has_no_instances_until_all_groups_are_loaded() {
        let mut store = SocialStore {
            instance_seen: HashMap::from([("wrld_a:1".to_owned(), SeenInstance { first_seen_at: 100, created_order: 1 })]),
            ..SocialStore::default()
        };
        assert!(store.group_instances_view().instances.is_none());

        // 全グループ分が未取得の間は、1 グループ分を取得しても一覧は出さず、引き継いだ記録も消さない
        store.apply_instances_of_group("grp_1", vec![api_group_instance("wrld_a:2~group(grp_1)", 1)], 200);
        assert!(store.group_instances_view().instances.is_none());
        assert!(store.instance_seen.contains_key("wrld_a:1"));
    }

    #[test]
    fn keeps_created_order_of_known_instances_on_group_refresh() {
        let mut store = SocialStore::default();
        store.apply_group_instances(
            vec![api_instance("wrld_a:1~group(grp_1)", "public"), api_instance("wrld_a:2~group(grp_1)", "public")],
            Some(100),
            100,
        );

        // 1 グループ分の API はレスポンス順が異なっても既知の順序を変えない
        store.apply_instances_of_group(
            "grp_1",
            vec![
                api_group_instance("wrld_a:1~group(grp_1)", 7),
                api_group_instance("wrld_a:3~group(grp_1)", 1),
                api_group_instance("wrld_a:2~group(grp_1)", 9),
            ],
            200,
        );
        let views = group_views(&store);
        let order = |id: &str| group_view(&views, id).created_order;
        assert_eq!((order("wrld_a:1~group(grp_1)"), order("wrld_a:2~group(grp_1)"), order("wrld_a:3~group(grp_1)")), (1, 2, 3));
        assert_eq!(group_view(&views, "wrld_a:1~group(grp_1)").user_count, 7);
    }

    #[test]
    fn shows_newer_of_full_list_and_group_refresh_per_group() {
        let mut store = SocialStore::default();
        let mut other = api_instance("wrld_a:9~group(grp_2)", "public");
        other.owner_id = "grp_2".into();
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public"), other.clone()], Some(100), 100);

        store.apply_instances_of_group("grp_1", vec![api_group_instance("wrld_a:2~group(grp_1)~groupAccessType(plus)", 5)], 200);
        let views = group_views(&store);
        // grp_1 は新しい 1 グループ分の結果、grp_2 は全グループ分の結果を表示する
        assert_eq!(views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["wrld_a:2~group(grp_1)~groupAccessType(plus)", "wrld_a:9~group(grp_2)"]);
        assert_eq!(views[0].access_type, "plus");
        assert_eq!((views[0].user_count, views[0].capacity), (5, 60));
        assert_eq!(views[0].first_seen_at, 200);
        // grp_1 の閉じたインスタンスの記録だけ消え、grp_2 の記録は残る
        assert!(!store.instance_seen.contains_key("wrld_a:1~group(grp_1)"));
        assert_eq!(store.instance_seen.get("wrld_a:9~group(grp_2)").map(|seen| seen.first_seen_at), Some(100));

        // それより新しい全グループ分の結果が届いたら、1 グループ分の結果は破棄する
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public"), other], Some(300), 300);
        assert!(store.group_refreshes.is_empty());
        let views = group_views(&store);
        assert_eq!(views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["wrld_a:1~group(grp_1)", "wrld_a:9~group(grp_2)"]);
        assert_eq!(views[0].first_seen_at, 300);
    }

    #[test]
    fn keeps_group_refresh_when_full_list_is_older() {
        let mut store = SocialStore::default();
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public")], Some(100), 100);
        store.apply_instances_of_group(
            "grp_1",
            vec![api_group_instance("wrld_a:1~group(grp_1)", 3), api_group_instance("wrld_a:2~group(grp_1)", 1)],
            200,
        );
        let seen = store.instance_seen["wrld_a:2~group(grp_1)"];

        // 手動更新より前に VRChat 側で取得された全グループ分の結果には wrld_a:2 が無いが、表示も記録も変えない
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public")], Some(150), 250);
        let views = group_views(&store);
        assert_eq!(views.len(), 2);
        let x = group_view(&views, "wrld_a:2~group(grp_1)");
        assert_eq!((x.first_seen_at, x.created_order), (seen.first_seen_at, seen.created_order));

        // 手動更新より新しい全グループ分の結果に切り替わっても、観測記録は引き継ぐ
        store.apply_group_instances(
            vec![api_instance("wrld_a:1~group(grp_1)", "public"), api_instance("wrld_a:2~group(grp_1)", "public")],
            Some(300),
            300,
        );
        let views = group_views(&store);
        let x = group_view(&views, "wrld_a:2~group(grp_1)");
        assert_eq!((x.first_seen_at, x.created_order), (seen.first_seen_at, seen.created_order));
    }

    #[test]
    fn compares_group_refresh_on_vrchat_clock() {
        let mut store = SocialStore::default();
        // 端末の時計が VRChat より 30 遅れている
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public")], Some(1000), 970);
        store.apply_instances_of_group(
            "grp_1",
            vec![api_group_instance("wrld_a:1~group(grp_1)", 3), api_group_instance("wrld_a:2~group(grp_1)", 1)],
            1000,
        );
        assert_eq!(store.group_refreshes["grp_1"].fetched_at, 1030);
        let seen = store.instance_seen["wrld_a:2~group(grp_1)"];

        // VRChat の時計で手動更新より前に取得された全グループ分の結果では、手動更新の表示を置き換えない
        store.apply_group_instances(vec![api_instance("wrld_a:1~group(grp_1)", "public")], Some(1010), 990);
        let views = group_views(&store);
        let x = group_view(&views, "wrld_a:2~group(grp_1)");
        assert_eq!((x.first_seen_at, x.created_order), (seen.first_seen_at, seen.created_order));
    }

    #[test]
    fn uses_smallest_observed_clock_offset() {
        let mut store = SocialStore::default();
        store.apply_group_instances(vec![], Some(1000), 1050);
        store.apply_group_instances(vec![], Some(2000), 2010);
        store.apply_group_instances(vec![], Some(3000), 3080);
        assert_eq!(store.vrchat_clock_offset_ms, Some(10));
        // fetchedAt を読み取れなかった回は時計のずれの計算に使わない
        store.apply_group_instances(vec![], None, 4000);
        assert_eq!(store.vrchat_clock_offset_ms, Some(10));
    }

    #[test]
    fn parses_location_params() {
        let location = "wrld_a:1~group(grp_1)~groupAccessType(plus)~region(us)";
        assert_eq!(location_param(location, "group"), Some("grp_1"));
        assert_eq!(location_param(location, "groupAccessType"), Some("plus"));
        assert_eq!(location_param(location, "private"), None);
    }

    #[test]
    fn drops_non_group_instances() {
        let mut store = SocialStore::default();
        let mut not_group = api_instance("wrld_a:1", "public");
        not_group.owner_id = "usr_1".into();
        store.apply_group_instances(vec![not_group], Some(100), 100);
        assert!(group_views(&store).is_empty());
    }

    #[test]
    fn fetches_group_instances_only_when_needed() {
        let conditions = |is_enabled: bool, is_visible: bool, is_minimized: bool| background::PollConditions {
            is_enabled,
            has_window: true,
            is_visible: Some(is_visible),
            is_minimized: Some(is_minimized),
        };
        // ウィンドウ表示中は、展開中のグループがあるときだけ取得する
        assert!(should_fetch_group_instances(true, &conditions(false, true, false)));
        assert!(!should_fetch_group_instances(false, &conditions(true, true, false)));
        // 非表示・最小化中は Run in background が有効なときだけ取得する
        assert!(should_fetch_group_instances(false, &conditions(true, false, false)));
        assert!(should_fetch_group_instances(true, &conditions(true, true, true)));
        assert!(!should_fetch_group_instances(true, &conditions(false, false, false)));
        assert!(!should_fetch_group_instances(true, &conditions(false, true, true)));
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
        assert_eq!(group_instances_refresh_delay(now, now), Duration::from_secs(120));
        assert_eq!(group_instances_refresh_delay(now - 30_000, now), Duration::from_secs(90));
        // 時計のずれで予定が過ぎていても連続取得しない
        assert_eq!(group_instances_refresh_delay(now - 150_000, now), Duration::from_secs(10));
        // fetchedAt が未来でも取得間隔より長くは待たない
        assert_eq!(group_instances_refresh_delay(now + 60_000, now), Duration::from_secs(120));
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

    #[test]
    fn records_last_joined_only_for_group_instances() {
        let mut store = SocialStore::default();
        assert!(store.record_joined("wrld_a:1~group(grp_1)~groupAccessType(plus)", 1_000));
        assert!(!store.record_joined("wrld_a:2", 1_000));
        assert!(!store.record_joined("wrld_a:3~private(usr_1)", 1_000));
        assert_eq!(store.last_joined.len(), 1);
    }

    #[test]
    fn overwrites_last_joined_when_entering_the_same_instance_again() {
        let mut store = SocialStore::default();
        store.record_joined("wrld_a:1~group(grp_1)", 1_000);
        store.record_joined("wrld_a:1~group(grp_1)", 5_000);
        assert_eq!(store.last_joined["wrld_a:1~group(grp_1)"], 5_000);
    }

    #[test]
    fn drops_last_joined_after_24_hours() {
        let mut store = SocialStore::default();
        store.last_joined.insert("wrld_a:1~group(grp_1)".into(), 0);
        store.last_joined.insert("wrld_b:2~group(grp_1)".into(), 1);

        // 23h59m59.999s では残り、24h ちょうどで消える
        store.prune_last_joined(LAST_JOINED_TTL_MS - 1);
        assert_eq!(store.last_joined.len(), 2);
        store.prune_last_joined(LAST_JOINED_TTL_MS);
        assert_eq!(store.last_joined.len(), 1);
        assert!(store.last_joined.contains_key("wrld_b:2~group(grp_1)"));
    }

    #[test]
    fn prunes_expired_records_when_recording_a_new_join() {
        let mut store = SocialStore::default();
        store.last_joined.insert("wrld_old:1~group(grp_1)".into(), 0);
        store.record_joined("wrld_new:1~group(grp_1)", LAST_JOINED_TTL_MS + 10);
        assert!(!store.last_joined.contains_key("wrld_old:1~group(grp_1)"));
        assert!(store.last_joined.contains_key("wrld_new:1~group(grp_1)"));
    }
}
