//! Chrome Client's cosmetics and friends, the same way the Chrome Client mod does it.
//!
//! Everything goes through the Chrome Client service (the `baksyaaaaa/Cosmetics` repo, server.js), never Polyfrost's servers:
//! - cosmetics: `POST /set` with your picks, `GET /get?u=` for other players (the mod draws them in game)
//! - friends and messages: sign in with Mojang's own check (`/chat/nonce` -> Mojang `join` -> `/chat/login`), then the
//!   `/social/...` calls with `Authorization: Bearer <token>`
//!
//! The wardrobe picks are kept by the launcher and written into each game's `config/chrome-cosmetics.json` (the file the
//! mod reads) before it starts; if you changed them in game since, the game's choice wins and comes back to the launcher.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::state::LauncherState;

/// The Chrome Client service. Fixed, like in the mod.
pub const SERVICE_URL: &str = "https://cosmetics-md7h.onrender.com";
const MOJANG_JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------- cosmetics

/// The categories and items exactly as the Chrome Client mod has them (`com.chromeclient.Cosmetics.ITEMS`);
/// an item's number is its place in the list, so the order must never change.
pub const CATEGORIES: [&str; 3] = ["Hats", "Capes", "Wings"];
pub const HATS: &[&str] = &["None", "Halo", "Cat ears", "Bunny ears", "Void ring"];
pub const CAPES: &[&str] = &[
    "None", "Midnight", "Snow", "Twisted", "Cherry",
    // CapeArt.NAMES
    "Cobalt", "dannyBstyle", "JulianClark", "Mapmaker", "Migrator", "Millionth Sale", "Minecon 2011", "Minecon 2012",
    "Minecon 2015", "Minecon 2016", "Moderator", "Mojang", "Mojang Studios", "Mojang (old)", "MrMessiah", "Prismarine",
    "Scrolls Champ", "Turtle", "Valentine",
    // added later, after the others so saved picks keep their number
    "Aurora", "Ember", "Abyss", "Galaxy", "Web",
];
pub const WINGS: &[&str] = &["None", "Angel", "Void bat", "Butterfly", "Crystal", "Phoenix"];

pub fn items(category: usize) -> &'static [&'static str] {
    match category {
        0 => HATS,
        1 => CAPES,
        _ => WINGS,
    }
}

/// The JSON keys the mod uses in chrome-cosmetics.json (its category names, lower case)
const KEYS: [&str; 3] = ["hats", "capes", "wings"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picks {
    pub hats: usize,
    pub capes: usize,
    pub wings: usize,
    /// ms since 1970 when these were last changed (in the launcher or, when imported, in game)
    #[serde(default)]
    pub changed: u64,
}

impl Picks {
    pub fn get(&self, category: usize) -> usize {
        match category {
            0 => self.hats,
            1 => self.capes,
            _ => self.wings,
        }
    }

    /// These picks with item `index` of `category` equipped
    #[must_use]
    pub fn with(mut self, category: usize, index: usize) -> Self {
        self.set(category.min(2), index);
        self
    }

    fn set(&mut self, category: usize, index: usize) {
        let index = index.min(items(category).len() - 1);
        match category {
            0 => self.hats = index,
            1 => self.capes = index,
            _ => self.wings = index,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn picks_file() -> Option<PathBuf> {
    scclient_common::paths::data_dir().ok().map(|d| d.join("chrome").join("cosmetics.json"))
}

pub async fn load_picks() -> Picks {
    let Some(path) = picks_file() else { return Picks::default() };
    match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Picks::default(),
    }
}

async fn save_picks(picks: &Picks) -> std::io::Result<()> {
    let Some(path) = picks_file() else { return Ok(()) };
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("json.part");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(picks).unwrap_or_default()).await?;
    tokio::fs::rename(&tmp, &path).await
}

/// The newest picks still waiting to be saved; quick clicks in a row end up as one save
static PENDING: std::sync::Mutex<Option<Picks>> = std::sync::Mutex::new(None);

/// Saves `picks` and tells the service, in the background: the wardrobe shows the change straight away and never
/// waits for the network. Must be called from inside the launcher's async runtime.
pub fn apply_picks(state: Arc<LauncherState>, mut picks: Picks) {
    picks.changed = now_ms();
    if let Ok(mut pending) = PENDING.lock() {
        *pending = Some(picks);
    }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        let Some(picks) = PENDING.lock().ok().and_then(|mut p| p.take()) else { return };
        if let Err(err) = save_picks(&picks).await {
            tracing::warn!(error = %err, "couldn't save the cosmetics");
        }
        publish(&state, &picks).await;
    });
}

/// `POST /set` like the mod's CosmeticSync.publish (best effort: the mod sends it again in game anyway)
async fn publish(state: &Arc<LauncherState>, picks: &Picks) {
    if scclient_common::consent::declined() {
        return; // the SC Client terms were declined
    }
    let Ok(Some(account)) = state.auth.default_account().await else { return };
    let body = json!({
        "u": account.id.hyphenated().to_string(),
        "h": picks.hats, "c": picks.capes, "w": picks.wings,
        "e": 0, "ew": 0, "ea": 0.5, "eo": true,
    });
    let sent = state.services.requester.http().post(format!("{SERVICE_URL}/set")).timeout(TIMEOUT).json(&body).send().await;
    if let Err(err) = sent {
        tracing::debug!(error = %err, "couldn't send cosmetics to the Chrome Client service");
    }
}

fn modified_ms(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// Before a game starts: puts the launcher's picks into the game's chrome-cosmetics.json, or - when the game's file is
/// newer (you changed them in game) - takes the game's picks back into the launcher. Other settings in the file stay.
pub async fn sync_to_game(game_dir: &Path) {
    let file = game_dir.join("config").join("chrome-cosmetics.json");
    let mut picks = load_picks().await;
    let mut config: serde_json::Map<String, Value> = tokio::fs::read(&file)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let file_time = modified_ms(&file);

    if let Some(file_time) = file_time
        && file_time > picks.changed
        && KEYS.iter().any(|k| config.contains_key(*k))
    {
        for (i, key) in KEYS.iter().enumerate() {
            if let Some(n) = config.get(*key).and_then(Value::as_u64) {
                picks.set(i, n as usize);
            }
        }
        picks.changed = file_time;
        if let Err(err) = save_picks(&picks).await {
            tracing::warn!(error = %err, "couldn't keep the in-game cosmetics");
        }
        return;
    }
    if picks.changed == 0 {
        return; // nothing chosen in the launcher yet: leave the game's file alone
    }
    for (i, key) in KEYS.iter().enumerate() {
        config.insert((*key).to_string(), json!(picks.get(i)));
    }
    let write = async {
        tokio::fs::create_dir_all(file.parent().unwrap_or(game_dir)).await?;
        tokio::fs::write(&file, serde_json::to_vec(&Value::Object(config)).unwrap_or_default()).await
    };
    if let Err(err) = write.await {
        tracing::warn!(error = %err, "couldn't write the cosmetics for the game");
    }
}

// ---------------------------------------------------------------- friends and messages

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Friend {
    #[serde(rename = "u")]
    pub uuid: String,
    #[serde(rename = "n")]
    pub name: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub unread: u32,
    /// player UUIDs whose faces to show for this entry (the SC Client chat shows the team's)
    #[serde(default)]
    pub faces: Vec<String>,
    /// the SC Client team chat (one message every 14 days)
    #[serde(default)]
    pub team: bool,
    /// in the SC Client chat: this player can send one message every 14 days (the team can't be limited)
    #[serde(default)]
    pub limited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Person {
    #[serde(rename = "u")]
    pub uuid: String,
    #[serde(rename = "n")]
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SocialState {
    #[serde(default)]
    pub friends: Vec<Friend>,
    #[serde(default)]
    pub incoming: Vec<Person>,
    #[serde(default)]
    pub outgoing: Vec<Person>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Message {
    pub id: u64,
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "m")]
    pub text: String,
    #[serde(default)]
    pub mine: bool,
}

#[derive(Deserialize)]
struct Messages {
    #[serde(default)]
    messages: Vec<Message>,
}

/// (account the token belongs to, token)
static SESSION: tokio::sync::Mutex<Option<(Uuid, String)>> = tokio::sync::Mutex::const_new(None);

async fn send(
    state: &Arc<LauncherState>,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> Result<(u16, Value), String> {
    let mut req = state.services.requester.http().request(method, format!("{SERVICE_URL}{path}")).timeout(TIMEOUT);
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    if let Some(body) = body {
        req = req.json(&body);
    }
    let res = req.send().await.map_err(|_| "Can't reach the SC Client service right now.".to_string())?;
    let status = res.status().as_u16();
    let value = res.json::<Value>().await.unwrap_or(Value::Null);
    Ok((status, value))
}

fn error_of(value: &Value, fallback: &str) -> String {
    value.get("error").and_then(Value::as_str).unwrap_or(fallback).to_string()
}

/// Signs in to the service with the launcher's Minecraft account (Mojang confirms it), like ChromeChat.logIn
async fn log_in(state: &Arc<LauncherState>) -> Result<(Uuid, String), String> {
    let account = state
        .auth
        .default_account_for_launch()
        .await
        .map_err(|e| format!("Couldn't use your account: {e}"))?
        .filter(|a| !a.access_token.is_empty())
        .ok_or_else(|| "Friends need a signed-in Microsoft account.".to_string())?;

    let (_, nonce) = send(state, reqwest::Method::POST, "/chat/nonce", None, None).await?;
    let nonce = nonce
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(|| "The SC Client service didn't answer. Try again in a moment.".to_string())?
        .to_string();

    let join = state
        .services
        .requester
        .http()
        .post(MOJANG_JOIN_URL)
        .timeout(TIMEOUT)
        .json(&json!({
            "accessToken": account.access_token,
            "selectedProfile": account.id.simple().to_string(),
            "serverId": nonce,
        }))
        .send()
        .await
        .map_err(|_| "Can't reach Mojang right now.".to_string())?;
    if !join.status().is_success() {
        return Err("Mojang didn't accept your session. Sign in again in the launcher.".into());
    }

    let (status, login) = send(
        state,
        reqwest::Method::POST,
        "/chat/login",
        Some(json!({ "name": account.username, "nonce": nonce })),
        None,
    )
    .await?;
    match login.get("token").and_then(Value::as_str) {
        Some(token) if status == 200 => Ok((account.id, token.to_string())),
        _ => Err(format!("Friends sign-in failed: {}", error_of(&login, "unknown error"))),
    }
}

/// A call that needs the sign-in; signs in (again) when there is no token or it ran out
async fn authed(
    state: &Arc<LauncherState>,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    if scclient_common::consent::declined() {
        return Err("Friends need the SC Client terms accepted (Settings > Launcher).".into());
    }
    let current = state.auth.default_account().await.ok().flatten().map(|a| a.id);
    for attempt in 0..2 {
        let token = {
            let mut session = SESSION.lock().await;
            match session.as_ref() {
                Some((id, token)) if Some(*id) == current => token.clone(),
                _ => {
                    let fresh = log_in(state).await?;
                    let token = fresh.1.clone();
                    *session = Some(fresh);
                    token
                }
            }
        };
        let (status, value) = send(state, method.clone(), path, body.clone(), Some(&token)).await?;
        match status {
            200 => return Ok(value),
            401 if attempt == 0 => {
                *SESSION.lock().await = None;
            }
            429 => return Err("Slow down a little.".into()),
            _ => return Err(error_of(&value, "That didn't work.")),
        }
    }
    Err("Friends sign-in failed.".into())
}

pub async fn social_state(state: &Arc<LauncherState>) -> Result<SocialState, String> {
    let value = authed(state, reqwest::Method::GET, "/social/state", None).await?;
    serde_json::from_value(value).map_err(|_| "The SC Client service sent something unexpected.".into())
}

pub async fn add_friend(state: &Arc<LauncherState>, name: &str) -> Result<(), String> {
    authed(state, reqwest::Method::POST, "/social/request", Some(json!({ "name": name.trim() }))).await.map(|_| ())
}

/// `action` is one of accept, decline, cancel, remove
pub async fn friend_action(state: &Arc<LauncherState>, action: &str, uuid: &str) -> Result<(), String> {
    authed(state, reqwest::Method::POST, &format!("/social/{action}"), Some(json!({ "u": uuid }))).await.map(|_| ())
}

pub async fn send_message(state: &Arc<LauncherState>, uuid: &str, text: &str) -> Result<(), String> {
    authed(state, reqwest::Method::POST, "/social/dm/send", Some(json!({ "u": uuid, "text": text }))).await.map(|_| ())
}

pub async fn messages(state: &Arc<LauncherState>, uuid: &str, after: u64) -> Result<Vec<Message>, String> {
    let path = format!("/social/dm/get?u={}&after={after}", urlencoding_component(uuid));
    let value = authed(state, reqwest::Method::GET, &path, None).await?;
    Ok(serde_json::from_value::<Messages>(value).map(|m| m.messages).unwrap_or_default())
}

fn urlencoding_component(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_hexdigit() || *c == '-').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_lists_match_the_mod() {
        assert_eq!(HATS.len(), 5);
        assert_eq!(CAPES.len(), 29);
        assert_eq!(WINGS.len(), 6);
        assert_eq!(CAPES[5], "Cobalt");
        assert_eq!(CAPES[24], "Aurora");
    }

    #[test]
    fn picks_are_clamped() {
        let mut p = Picks::default();
        p.set(0, 99);
        assert_eq!(p.hats, HATS.len() - 1);
    }

    #[test]
    fn parses_the_service_state() {
        let s: SocialState = serde_json::from_value(json!({
            "friends": [{"u": "a-b", "n": "Steve", "online": true, "unread": 2}],
            "incoming": [{"u": "c", "n": "Alex"}],
            "outgoing": []
        }))
        .unwrap();
        assert_eq!(s.friends[0].name, "Steve");
        assert_eq!(s.friends[0].unread, 2);
        assert_eq!(s.incoming[0].name, "Alex");
    }
}
