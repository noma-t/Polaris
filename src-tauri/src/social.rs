//! Friends / Groups の状態を Rust 側で保持し、REST の初期同期と pipeline イベントで更新する。
//! 状態が変わるたびに整形済みの snapshot をフロントエンドへ emit する。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::pipeline::{self, PipelineEvent};
use crate::vrchat_client::{AuthError, VrchatClient, FRIENDS_PAGE_SIZE};
use crate::vrchat_models::ApiFriend;

pub const FRIENDS_UPDATED_EVENT: &str = "social://friends-updated";
pub const GROUPS_UPDATED_EVENT: &str = "social://groups-updated";
pub const SESSION_EXPIRED_EVENT: &str = "social://session-expired";

/// World 名の取得は 1 秒に 1 件まで
const WORLD_FETCH_INTERVAL: Duration = Duration::from_secs(1);
/// 取得に失敗した World を再試行するまでの間隔
const WORLD_RETRY_DELAY: Duration = Duration::from_secs(30);
/// 429 を受けたときに World 名の取得を止める時間
const RATE_LIMIT_PAUSE: Duration = Duration::from_secs(60);

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
    World { world_id: String },
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
                Some((world_id, _)) if world_id.starts_with("wrld_") => Self::World { world_id: world_id.to_owned() },
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
    location: FriendLocation,
}

impl FriendEntry {
    fn from_api(user: &ApiFriend, location: FriendLocation) -> Self {
        Self {
            id: user.id.clone(),
            name: user.display_name.clone(),
            status: FriendStatus::from_api(&user.status),
            location,
        }
    }

    /// pipeline の `user` でプロフィールを更新する (location は含まれないので変えない)
    fn update_profile(&mut self, user: &ApiFriend) {
        if !user.display_name.is_empty() {
            self.name = user.display_name.clone();
        }
        if !user.status.is_empty() {
            self.status = FriendStatus::from_api(&user.status);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendView {
    id: String,
    name: String,
    status: FriendStatus,
    location_kind: &'static str,
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

#[derive(Default)]
struct SocialStore {
    /// start / stop のたびに進める。古いセッションのタスクによる書き込みを捨てるために使う
    generation: u64,
    friends: HashMap<String, FriendEntry>,
    pinned: HashSet<String>,
    world_names: HashMap<String, String>,
    world_retry_at: HashMap<String, Instant>,
    groups: Vec<GroupView>,
}

impl SocialStore {
    fn friend_views(&self) -> Vec<FriendView> {
        self.friends
            .values()
            .map(|friend| {
                let world_name = match &friend.location {
                    FriendLocation::World { world_id } => self.world_names.get(world_id).cloned(),
                    _ => None,
                };
                let is_in_world = matches!(friend.location, FriendLocation::World { .. });
                FriendView {
                    id: friend.id.clone(),
                    name: friend.name.clone(),
                    status: if friend.location == FriendLocation::Offline { FriendStatus::Offline } else { friend.status },
                    location_kind: friend.location.kind(),
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
            FriendLocation::World { world_id }
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

    fn handle_error(&self, error: &AuthError) {
        if matches!(error, AuthError::Unauthorized) && self.with_store(|_| ()).is_some() {
            let _ = self.app.emit(SESSION_EXPIRED_EVENT, ());
        }
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
            Err(error) => self.handle_error(&error),
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
            Err(error) => self.handle_error(&error),
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
                Err(error @ AuthError::Unauthorized) => return self.handle_error(&error),
                Err(_) => {
                    self.with_store(|store| store.world_retry_at.insert(world_id, Instant::now() + WORLD_RETRY_DELAY));
                }
            }
        }
    }
}

pub struct SocialState {
    store: Arc<Mutex<SocialStore>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}

impl SocialState {
    pub fn new() -> Self {
        Self { store: Arc::default(), tasks: Mutex::default() }
    }

    /// ログイン完了時に呼ぶ。初期同期・pipeline・World 名取得のタスクを起動する
    pub fn start(&self, app: AppHandle, client: VrchatClient, user_id: String) {
        self.stop();
        let generation = self.store.lock().expect("social store poisoned").generation;
        let session = Session { app, client, user_id, store: self.store.clone(), generation };

        let initial_sync = session.clone();
        let tasks = vec![
            tauri::async_runtime::spawn(async move {
                initial_sync.sync_friends().await;
                initial_sync.sync_groups().await;
            }),
            tauri::async_runtime::spawn(session.clone().run_pipeline()),
            tauri::async_runtime::spawn(session.run_world_queue()),
        ];
        *self.tasks.lock().expect("social tasks poisoned") = tasks;
    }

    /// ログアウト時に呼ぶ。全タスクを止めて状態を破棄する
    pub fn stop(&self) {
        for task in self.tasks.lock().expect("social tasks poisoned").drain(..) {
            task.abort();
        }
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
    use crate::vrchat_models::ApiWorld;

    fn api_friend(id: &str, status: &str, location: &str) -> ApiFriend {
        ApiFriend { id: id.into(), display_name: id.to_uppercase(), status: status.into(), location: location.into() }
    }

    fn view<'a>(views: &'a [FriendView], id: &str) -> &'a FriendView {
        views.iter().find(|v| v.id == id).expect("friend not found")
    }

    #[test]
    fn parses_locations() {
        assert_eq!(
            FriendLocation::from_api("wrld_abc:123~group(grp_1)"),
            FriendLocation::World { world_id: "wrld_abc".into() }
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
            world: Some(ApiWorld { id: "wrld_y".into(), name: "Rainy Window".into() }),
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
