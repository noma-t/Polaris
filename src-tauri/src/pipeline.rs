//! VRChat pipeline (WebSocket) への接続と受信イベントの parse。
//! 切断時は exponential backoff で再接続し続ける。

use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::header, Message};

use crate::auth_log;
use crate::vrchat_client::{VrchatClient, USER_AGENT};
use crate::vrchat_models::{ApiFriend, ApiWorld};

const PIPELINE_URL: &str = "wss://pipeline.vrchat.cloud/";
const RECONNECT_BASE_DELAY: Duration = Duration::from_secs(1);
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(60);

/// Polaris が扱う pipeline イベント
#[derive(Debug, Clone)]
pub enum PipelineEvent {
    /// 接続 (再接続を含む) が確立した。切断中の変化を取りこぼしているので REST で再同期する
    Connected,
    /// ゲーム内でオンラインになった
    FriendOnline { user_id: String, location: String, user: Option<ApiFriend> },
    /// Web サイト上でオンラインになった
    FriendActive { user_id: String, user: Option<ApiFriend> },
    FriendOffline { user_id: String },
    FriendLocation { user_id: String, location: String, user: Option<ApiFriend>, world: Option<ApiWorld> },
    /// プロフィール (表示名・status 等) の更新
    FriendUpdate { user_id: String, user: ApiFriend },
    FriendAdd { user_id: String, user: ApiFriend },
    FriendDelete { user_id: String },
    /// group の参加 / 脱退
    GroupsChanged,
    /// 自分がインスタンスに入った (到着後の `wrld_…:…` 形式の location のみ)
    UserLocation { location: String },
}

#[derive(Deserialize)]
struct RawMessage {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: Value,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct FriendContent {
    user_id: String,
    location: String,
    user: Option<ApiFriend>,
    world: Option<ApiWorld>,
}

/// pipeline のテキストメッセージを parse する。`content` は JSON 文字列として二重エンコードされている
pub fn parse_message(text: &str) -> Option<PipelineEvent> {
    let raw: RawMessage = serde_json::from_str(text).ok()?;
    let content = match raw.content {
        Value::String(inner) => serde_json::from_str(&inner).unwrap_or(Value::Null),
        other => other,
    };
    let friend = || serde_json::from_value::<FriendContent>(content.clone()).ok();

    let event = match raw.kind.as_str() {
        "friend-online" => {
            let c = friend()?;
            PipelineEvent::FriendOnline { user_id: c.user_id, location: c.location, user: c.user }
        }
        "friend-active" => {
            let c = friend()?;
            PipelineEvent::FriendActive { user_id: c.user_id, user: c.user }
        }
        "friend-offline" => PipelineEvent::FriendOffline { user_id: friend()?.user_id },
        "friend-location" => {
            let c = friend()?;
            // private 等では world が空オブジェクトで届くため、ID のあるものだけ採用する
            let world = c.world.filter(|w| !w.id.is_empty() && !w.name.is_empty());
            PipelineEvent::FriendLocation { user_id: c.user_id, location: c.location, user: c.user, world }
        }
        "friend-update" => {
            let c = friend()?;
            PipelineEvent::FriendUpdate { user_id: c.user_id, user: c.user? }
        }
        "friend-add" => {
            let c = friend()?;
            PipelineEvent::FriendAdd { user_id: c.user_id, user: c.user? }
        }
        "friend-delete" => PipelineEvent::FriendDelete { user_id: friend()?.user_id },
        "group-joined" | "group-left" => PipelineEvent::GroupsChanged,
        // 自分宛てのイベントなので userId は見ない。移動中 (`traveling`) や private は到着後の通知を待つ
        "user-location" => {
            let location = friend()?.location;
            location.starts_with("wrld_").then_some(PipelineEvent::UserLocation { location })?
        }
        _ => return None,
    };
    let has_user_id = match &event {
        PipelineEvent::Connected | PipelineEvent::GroupsChanged | PipelineEvent::UserLocation { .. } => true,
        PipelineEvent::FriendOnline { user_id, .. }
        | PipelineEvent::FriendActive { user_id, .. }
        | PipelineEvent::FriendOffline { user_id }
        | PipelineEvent::FriendLocation { user_id, .. }
        | PipelineEvent::FriendUpdate { user_id, .. }
        | PipelineEvent::FriendAdd { user_id, .. }
        | PipelineEvent::FriendDelete { user_id } => !user_id.is_empty(),
    };
    has_user_id.then_some(event)
}

/// pipeline に接続し、受信したイベントを `tx` に流し続ける。受信側が閉じたら終了する
pub async fn run(client: VrchatClient, tx: mpsc::Sender<PipelineEvent>) {
    let mut delay = RECONNECT_BASE_DELAY;
    loop {
        match connect_and_listen(&client, &tx).await {
            ListenOutcome::ReceiverClosed => return,
            ListenOutcome::Disconnected { was_connected } => {
                if was_connected {
                    delay = RECONNECT_BASE_DELAY;
                }
            }
        }
        auth_log::log!("pipeline: reconnecting in {}s", delay.as_secs());
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(RECONNECT_MAX_DELAY);
    }
}

enum ListenOutcome {
    ReceiverClosed,
    Disconnected { was_connected: bool },
}

async fn connect_and_listen(client: &VrchatClient, tx: &mpsc::Sender<PipelineEvent>) -> ListenOutcome {
    let disconnected = ListenOutcome::Disconnected { was_connected: false };
    let Some(token) = client.auth_token() else {
        auth_log::log!("pipeline: no auth cookie cookies=[{}]", client.cookie_names().join(", "));
        return disconnected;
    };
    let url = format!("{PIPELINE_URL}?authToken={}", urlencoding::encode(&token));
    let Ok(mut request) = url.into_client_request() else {
        return disconnected;
    };
    request.headers_mut().insert(header::USER_AGENT, header::HeaderValue::from_static(USER_AGENT));

    // URL は認証トークンを含むので記録しない
    let mut stream = match tokio_tungstenite::connect_async(request).await {
        Ok((stream, _)) => stream,
        Err(err) => {
            auth_log::log!("pipeline: failed to connect: {err}");
            return disconnected;
        }
    };
    auth_log::log!("pipeline: connected");
    if tx.send(PipelineEvent::Connected).await.is_err() {
        return ListenOutcome::ReceiverClosed;
    }

    // Ping への Pong 応答は tungstenite が自動で返す
    while let Some(message) = stream.next().await {
        let text = match message {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(frame)) => {
                auth_log::log!("pipeline: closed by server {frame:?}");
                break;
            }
            Err(err) => {
                auth_log::log!("pipeline: connection error: {err}");
                break;
            }
            Ok(_) => continue,
        };
        if let Some(event) = parse_message(&text) {
            if tx.send(event).await.is_err() {
                return ListenOutcome::ReceiverClosed;
            }
        }
    }
    auth_log::log!("pipeline: disconnected");
    ListenOutcome::Disconnected { was_connected: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(kind: &str, content: Value) -> String {
        serde_json::json!({ "type": kind, "content": content.to_string() }).to_string()
    }

    #[test]
    fn parses_friend_location_with_world() {
        let text = wrap(
            "friend-location",
            serde_json::json!({
                "userId": "usr_1",
                "location": "wrld_abc:12345~private(usr_1)",
                "user": { "id": "usr_1", "displayName": "Alice", "status": "join me" },
                "world": { "id": "wrld_abc", "name": "Quiet Shore", "capacity": 16 }
            }),
        );
        let Some(PipelineEvent::FriendLocation { user_id, location, user, world }) = parse_message(&text) else {
            panic!("expected FriendLocation");
        };
        assert_eq!(user_id, "usr_1");
        assert_eq!(location, "wrld_abc:12345~private(usr_1)");
        assert_eq!(user.unwrap().display_name, "Alice");
        assert_eq!(world.unwrap().name, "Quiet Shore");
    }

    #[test]
    fn drops_empty_world_on_private_location() {
        let text = wrap(
            "friend-location",
            serde_json::json!({ "userId": "usr_1", "location": "private", "world": {} }),
        );
        let Some(PipelineEvent::FriendLocation { world, .. }) = parse_message(&text) else {
            panic!("expected FriendLocation");
        };
        assert!(world.is_none());
    }

    #[test]
    fn parses_friend_offline_and_delete() {
        let offline = wrap("friend-offline", serde_json::json!({ "userId": "usr_2", "platform": "standalonewindows" }));
        assert!(matches!(parse_message(&offline), Some(PipelineEvent::FriendOffline { user_id }) if user_id == "usr_2"));
        let delete = wrap("friend-delete", serde_json::json!({ "userId": "usr_3" }));
        assert!(matches!(parse_message(&delete), Some(PipelineEvent::FriendDelete { user_id }) if user_id == "usr_3"));
    }

    #[test]
    fn parses_group_events_and_ignores_unknown() {
        let joined = wrap("group-joined", serde_json::json!({ "groupId": "grp_1" }));
        assert!(matches!(parse_message(&joined), Some(PipelineEvent::GroupsChanged)));
        let unknown = wrap("notification", serde_json::json!({ "id": "not_1" }));
        assert!(parse_message(&unknown).is_none());
        assert!(parse_message("not json").is_none());
    }

    #[test]
    fn parses_own_location_without_user_id() {
        let text = wrap("user-location", serde_json::json!({ "location": "wrld_abc:123~group(grp_1)" }));
        assert!(matches!(
            parse_message(&text),
            Some(PipelineEvent::UserLocation { location }) if location == "wrld_abc:123~group(grp_1)"
        ));
    }

    #[test]
    fn ignores_own_location_while_traveling_or_private() {
        for location in ["traveling", "private", "offline", ""] {
            let text = wrap("user-location", serde_json::json!({ "userId": "usr_1", "location": location }));
            assert!(parse_message(&text).is_none(), "{location:?} should be ignored");
        }
    }

    #[test]
    fn rejects_friend_update_without_user() {
        let text = wrap("friend-update", serde_json::json!({ "userId": "usr_1" }));
        assert!(parse_message(&text).is_none());
    }
}
