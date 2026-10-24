use crate::persistent_snapshot;
use async_std::io::ReadExt;
use partitioned_array_rust::PartitionedArray;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{io, path::PathBuf};
use tide::{Request, Response, StatusCode};

const MAX_BODY_BYTES: u64 = 4096;
const STALE_AFTER: Duration = Duration::from_secs(45);
const MAX_LIBRARY_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MusicEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    video_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    list: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    title: String,
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedQueue {
    name: String,
    items: Vec<MusicEntry>,
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct PlayerSettings {
    volume: u8,
    muted: bool,
    shuffle: bool,
    repeat: String,
    queue_index: i64,
    position_seconds: f64,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            volume: 70,
            muted: false,
            shuffle: false,
            repeat: "off".into(),
            queue_index: -1,
            position_seconds: 0.0,
        }
    }
}

#[derive(Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct MusicLibrary {
    queue: Vec<MusicEntry>,
    saved: Vec<SavedQueue>,
    history: Vec<MusicEntry>,
    settings: PlayerSettings,
}

fn valid_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

impl MusicLibrary {
    fn validate(&self) -> tide::Result<()> {
        let valid_entry = |entry: &MusicEntry| {
            let source = match (&entry.video_id, &entry.list) {
                (Some(id), None) => valid_video_id(id),
                (None, Some(list)) => {
                    (10..=100).contains(&list.len())
                        && list
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                }
                _ => false,
            };
            source && entry.title.chars().count() <= 160
        };
        let settings = &self.settings;
        if self.queue.len() > 100
            || self.history.len() > 50
            || self.saved.len() > 30
            || !self.queue.iter().all(valid_entry)
            || !self.history.iter().all(valid_entry)
            || self.saved.iter().any(|saved| {
                saved.name.trim().is_empty()
                    || saved.name.chars().count() > 60
                    || saved.items.len() > 100
                    || !saved.items.iter().all(valid_entry)
            })
            || settings.volume > 100
            || !["off", "track", "queue"].contains(&settings.repeat.as_str())
            || settings.queue_index < -1
            || settings.queue_index >= self.queue.len() as i64
            || !settings.position_seconds.is_finite()
            || !(0.0..=864000.0).contains(&settings.position_seconds)
        {
            return Err(tide::Error::from_str(
                StatusCode::BadRequest,
                "Invalid music library or player settings",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RememberedLogin {
    id: String,
    token: String,
    expires_at: i64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MusicRecord {
    username: String,
    revision: u64,
    library: MusicLibrary,
    playback: Playback,
    updated_at: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    logins: Vec<RememberedLogin>,
}

impl MusicRecord {
    fn public_response(&self) -> Response {
        let mut response = crate::json_response(serde_json::json!({
            "username": self.username, "revision": self.revision,
            "library": self.library, "playback": self.playback,
            "updated_at": self.updated_at,
        }));
        response.insert_header("Cache-Control", "no-store");
        response
    }
}

struct MusicStore {
    path: Option<PathBuf>,
    rows: PartitionedArray,
    live: HashMap<String, Instant>,
}

impl MusicStore {
    fn open(path: Option<PathBuf>) -> io::Result<Self> {
        let rows = match path
            .as_ref()
            .map(|path| persistent_snapshot::load::<PartitionedArray>(path))
            .transpose()?
            .flatten()
        {
            Some(rows) => {
                let mut names = std::collections::HashSet::new();
                for row in rows.data_arr.iter().filter(|row| !row.is_empty()) {
                    let record: MusicRecord =
                        serde_json::from_value(serde_json::Value::Object(row.clone()))
                            .map_err(io::Error::other)?;
                    if record.username.is_empty()
                        || record.username.len() > 40
                        || record.username != record.username.to_ascii_lowercase()
                        || !record
                            .username
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
                        || !names.insert(record.username.clone())
                    {
                        return Err(io::Error::other(
                            "Invalid or duplicate music username in snapshot",
                        ));
                    }
                    record.library.validate().map_err(io::Error::other)?;
                    record.playback.validate().map_err(io::Error::other)?;
                }
                rows
            }
            None => {
                let mut rows = PartitionedArray::new(1, 32, 1, true);
                rows.allocate(false);
                rows
            }
        };
        Ok(Self {
            path,
            rows,
            live: HashMap::new(),
        })
    }

    fn record(&self, username: &str) -> MusicRecord {
        self.rows
            .data_arr
            .iter()
            .find(|row| row.get("username").and_then(serde_json::Value::as_str) == Some(username))
            .map(|row| serde_json::from_value(serde_json::Value::Object(row.clone())).unwrap())
            .unwrap_or_else(|| MusicRecord {
                username: username.into(),
                revision: 0,
                library: MusicLibrary::default(),
                playback: Playback::default(),
                updated_at: 0,
                logins: Vec::new(),
            })
    }

    fn remembered_login(&self, id: &str, token: &str) -> Option<(String, String)> {
        for row in self.rows.data_arr.iter().filter(|row| !row.is_empty()) {
            let record: MusicRecord =
                serde_json::from_value(serde_json::Value::Object(row.clone())).ok()?;
            if let Some(login) = record.logins.iter().find(|login| {
                login.id == id
                    && login.token == token
                    && login.expires_at > chrono::Utc::now().timestamp()
            }) {
                return Some((record.username, login.token.clone()));
            }
        }
        None
    }

    fn remember_login(
        &mut self,
        username: &str,
        token: String,
        previous: &str,
    ) -> tide::Result<String> {
        let mut record = self.record(username);
        let now = chrono::Utc::now().timestamp();
        record
            .logins
            .retain(|login| login.expires_at > now && login.id != previous && login.token == token);
        if record.logins.len() >= 20 {
            record.logins.remove(0);
        }
        let id = uuid::Uuid::new_v4().to_string();
        record.logins.push(RememberedLogin {
            id: id.clone(),
            token,
            expires_at: now + 30 * 86400,
        });
        self.commit(record)?;
        Ok(id)
    }

    fn forget_login(&mut self, username: &str, id: &str) -> tide::Result<()> {
        let mut record = self.record(username);
        record.logins.retain(|login| login.id != id);
        self.commit(record)?;
        Ok(())
    }

    fn commit(&mut self, record: MusicRecord) -> tide::Result<MusicRecord> {
        let mut next = self.rows.clone();
        let row = serde_json::to_value(&record)?.as_object().unwrap().clone();
        if let Some(slot) = next.data_arr.iter_mut().find(|row| {
            row.get("username").and_then(serde_json::Value::as_str)
                == Some(record.username.as_str())
        }) {
            *slot = row;
        } else {
            next.add(|slot| *slot = row.clone()).ok_or_else(|| {
                tide::Error::from_str(StatusCode::InsufficientStorage, "Music database is full")
            })?;
        }
        if let Some(path) = &self.path {
            persistent_snapshot::save(path, &next)?;
        }
        self.rows = next;
        Ok(record)
    }

    fn save_library(
        &mut self,
        username: &str,
        revision: u64,
        library: MusicLibrary,
    ) -> tide::Result<MusicRecord> {
        library.validate()?;
        let mut record = self.record(username);
        if revision != record.revision {
            return Err(tide::Error::from_str(
                StatusCode::Conflict,
                "Music library changed in another tab. Reconnect before editing.",
            ));
        }
        if record.library == library {
            return Ok(record);
        }
        record.library = library;
        record.revision += 1;
        self.commit(record)
    }

    fn save_playback(&mut self, username: &str, playback: Playback) -> tide::Result<()> {
        let mut record = self.record(username);
        record.playback = playback;
        record.updated_at = chrono::Utc::now().timestamp();
        self.commit(record)?;
        self.live
            .retain(|_, received| received.elapsed() < STALE_AFTER);
        self.live.insert(username.into(), Instant::now());
        Ok(())
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Playback {
    title: String,
    artist: String,
    video_id: String,
    is_playing: bool,
}

impl Playback {
    fn validate(&self) -> tide::Result<()> {
        if self.title.chars().count() > 160
            || self.artist.chars().count() > 100
            || (!self.video_id.is_empty() && !valid_video_id(&self.video_id))
            || (self.is_playing
                && (self.title.trim().is_empty() || !valid_video_id(&self.video_id)))
        {
            return Err(tide::Error::from_str(
                StatusCode::BadRequest,
                "Invalid playback metadata",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct PlaybackState {
    token: Option<String>,
    store: Arc<Mutex<MusicStore>>,
}

impl PlaybackState {
    fn authorize<State>(&self, req: &Request<State>) -> tide::Result<()> {
        let token = self
            .token
            .as_deref()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                tide::Error::from_str(
                    StatusCode::ServiceUnavailable,
                    "YOUTUBEMUSIC_TOKEN is not configured",
                )
            })?;
        if req
            .header("X-YouTubeMusic-Token")
            .map(|values| values.last().as_str())
            != Some(token)
        {
            return Err(tide::Error::from_str(
                StatusCode::Unauthorized,
                "Invalid playback token",
            ));
        }
        Ok(())
    }

    fn snapshot(&self, username: &str) -> Response {
        let store = self.store.lock().unwrap();
        let fresh = store
            .live
            .get(username)
            .is_some_and(|received| received.elapsed() < STALE_AFTER);
        let playback = if fresh {
            store.record(username).playback
        } else {
            Playback::default()
        };
        let url = if playback.video_id.is_empty() {
            String::new()
        } else {
            format!("https://music.youtube.com/watch?v={}", playback.video_id)
        };
        let mut response = crate::json_response(serde_json::json!({
            "username": username,
            "title": playback.title,
            "artist": playback.artist,
            "is_playing": playback.is_playing,
            "url": url,
            "stale": !fresh,
        }));
        response.insert_header("Cache-Control", "no-store");
        response
    }
}

fn username<State>(req: &Request<State>) -> tide::Result<String> {
    let username = req
        .header("X-YouTubeMusic-Username")
        .map(|values| values.last().as_str())
        .unwrap_or("default")
        .trim()
        .to_ascii_lowercase();
    if username.is_empty()
        || username.len() > 40
        || !username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return Err(tide::Error::from_str(
            StatusCode::BadRequest,
            "Use a music username of 1-40 letters, numbers, dots, underscores or hyphens",
        ));
    }
    Ok(username)
}

pub fn mount<State: Clone + Send + Sync + 'static>(
    app: &mut tide::Server<State>,
) -> io::Result<()> {
    let path = std::env::var_os("YOUTUBEMUSIC_DATA_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("youtube_music_data/music.json")
        });
    mount_with_store(
        app,
        std::env::var("YOUTUBEMUSIC_TOKEN").ok(),
        MusicStore::open(Some(path))?,
    );
    Ok(())
}

#[cfg(test)]
fn mount_with_token<State: Clone + Send + Sync + 'static>(
    app: &mut tide::Server<State>,
    token: Option<String>,
) {
    mount_with_store(app, token, MusicStore::open(None).unwrap());
}

fn mount_with_store<State: Clone + Send + Sync + 'static>(
    app: &mut tide::Server<State>,
    token: Option<String>,
    store: MusicStore,
) {
    let state = PlaybackState {
        token,
        store: Arc::new(Mutex::new(store)),
    };
    let library_reader = state.clone();
    let library_writer = state.clone();
    let session_reader = state.clone();
    let session_writer = state.clone();
    let session_remover = state.clone();
    app.at("/youtubemusic/session")
        .get(move |req: Request<State>| {
            let state = session_reader.clone();
            async move {
                let cookie = req.cookie("ytm_session");
                let login = cookie.as_ref().and_then(|cookie| {
                    state.token.as_deref().filter(|token| !token.is_empty()).and_then(|token| {
                        state.store.lock().unwrap().remembered_login(cookie.value(), token)
                    })
                });
                let mut response = crate::json_response(login.map(|(username, token)| {
                    serde_json::json!({"username": username, "token": token})
                }));
                response.insert_header("Cache-Control", "no-store");
                response.insert_header("Vary", "Cookie");
                Ok(response)
            }
        })
        .post(move |req: Request<State>| {
            let state = session_writer.clone();
            async move {
                state.authorize(&req)?;
                let username = username(&req)?;
                let previous = req.cookie("ytm_session").map(|cookie| cookie.value().to_owned()).unwrap_or_default();
                let token = state.token.clone().unwrap();
                let id = async_std::task::spawn_blocking(move || {
                    state.store.lock().unwrap().remember_login(&username, token, &previous)
                }).await?;
                let mut response = crate::json_response(serde_json::json!({"remembered": true}));
                response.insert_header("Cache-Control", "no-store");
                response.insert_header("Set-Cookie", format!("ytm_session={id}; Path=/youtubemusic; Max-Age=2592000; Secure; HttpOnly; SameSite=Strict"));
                Ok(response)
            }
        })
        .delete(move |req: Request<State>| {
            let state = session_remover.clone();
            async move {
                state.authorize(&req)?;
                let username = username(&req)?;
                if let Some(cookie) = req.cookie("ytm_session") {
                    let id = cookie.value().to_owned();
                    async_std::task::spawn_blocking(move || {
                        state.store.lock().unwrap().forget_login(&username, &id)
                    }).await?;
                }
                let mut response = crate::json_response(serde_json::json!({"remembered": false}));
                response.insert_header("Cache-Control", "no-store");
                response.insert_header("Set-Cookie", "ytm_session=; Path=/youtubemusic; Max-Age=0; Secure; HttpOnly; SameSite=Strict");
                Ok(response)
            }
        });
    app.at("/youtubemusic/library")
        .get(move |req: Request<State>| {
            let state = library_reader.clone();
            async move {
                state.authorize(&req)?;
                let record = state.store.lock().unwrap().record(&username(&req)?);
                Ok(record.public_response())
            }
        })
        .put(move |mut req: Request<State>| {
            let state = library_writer.clone();
            async move {
                state.authorize(&req)?;
                let username = username(&req)?;
                let mut body = Vec::new();
                req.take_body()
                    .into_reader()
                    .take(MAX_LIBRARY_BYTES + 1)
                    .read_to_end(&mut body)
                    .await?;
                if body.len() > MAX_LIBRARY_BYTES as usize {
                    return Err(tide::Error::from_str(
                        StatusCode::PayloadTooLarge,
                        "Music library exceeds 1 MB",
                    ));
                }
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Update {
                    revision: u64,
                    library: MusicLibrary,
                }
                let update: Update = serde_json::from_slice(&body).map_err(|_| {
                    tide::Error::from_str(StatusCode::BadRequest, "Invalid music library JSON")
                })?;
                let record = async_std::task::spawn_blocking(move || {
                    state.store.lock().unwrap().save_library(
                        &username,
                        update.revision,
                        update.library,
                    )
                })
                .await?;
                Ok(record.public_response())
            }
        });
    app.at("/youtubemusic/player").get(|_| async {
        let mut response = Response::new(StatusCode::Ok);
        response.set_content_type(tide::http::mime::HTML);
        response.set_body(include_str!("youtube_music_player.html"));
        response.insert_header("Cache-Control", "no-store");
        response.insert_header("Referrer-Policy", "strict-origin-when-cross-origin");
        Ok(response)
    });
    let metadata_state = state.clone();
    app.at("/youtubemusic/metadata").get(move |req: Request<State>| {
        let state = metadata_state.clone();
        async move {
            state.authorize(&req)?;
            #[derive(Deserialize)]
            struct Query { video_id: String }
            let query: Query = req.query()?;
            if query.video_id.len() != 11 || !query.video_id.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte)) {
                return Err(tide::Error::from_str(StatusCode::BadRequest, "Invalid YouTube video ID"));
            }
            let endpoint = format!("https://www.youtube.com/oembed?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3D{}", query.video_id);
            let metadata = async_std::future::timeout(Duration::from_secs(8), async {
                let mut upstream = surf::get(endpoint).await
                    .map_err(|_| tide::Error::from_str(StatusCode::BadGateway, "YouTube metadata request failed"))?;
                if !upstream.status().is_success() {
                    return Err(tide::Error::from_str(StatusCode::BadGateway, "YouTube could not provide this video's metadata"));
                }
                #[derive(Deserialize)]
                struct Metadata { title: String, author_name: String }
                let metadata: Metadata = upstream.body_json().await
                    .map_err(|_| tide::Error::from_str(StatusCode::BadGateway, "Invalid YouTube metadata"))?;
                Ok(crate::json_response(serde_json::json!({
                    "title": metadata.title.chars().take(160).collect::<String>(),
                    "artist": metadata.author_name.chars().take(100).collect::<String>(),
                })))
            }).await.map_err(|_| tide::Error::from_str(StatusCode::GatewayTimeout, "YouTube metadata timed out"))?;
            metadata
        }
    });
    let reader = state.clone();
    app.at("/youtubemusic")
        .get(move |req: Request<State>| {
            let state = reader.clone();
            async move {
                state.authorize(&req)?;
                Ok(state.snapshot(&username(&req)?))
            }
        })
        .post(move |mut req: Request<State>| {
            let state = state.clone();
            async move {
                state.authorize(&req)?;
                let username = username(&req)?;
                let mut body = Vec::new();
                req.take_body()
                    .into_reader()
                    .take(MAX_BODY_BYTES + 1)
                    .read_to_end(&mut body)
                    .await?;
                if body.len() > MAX_BODY_BYTES as usize {
                    return Err(tide::Error::from_str(
                        StatusCode::PayloadTooLarge,
                        "Playback update is too large",
                    ));
                }
                let playback: Playback = serde_json::from_slice(&body).map_err(|_| {
                    tide::Error::from_str(StatusCode::BadRequest, "Invalid playback JSON")
                })?;
                playback.validate()?;
                let store = state.store.clone();
                let owner = username.clone();
                async_std::task::spawn_blocking(move || {
                    store.lock().unwrap().save_playback(&owner, playback)
                })
                .await?;
                Ok(state.snapshot(&username))
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tide::http::{Method, Request as HttpRequest, Url};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("music-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn library() -> MusicLibrary {
        let entry = MusicEntry {
            video_id: Some("abcdefghijk".into()),
            list: None,
            title: "Test song".into(),
        };
        MusicLibrary {
            queue: vec![entry.clone()],
            saved: vec![SavedQueue {
                name: "Favorites".into(),
                items: vec![entry.clone()],
            }],
            history: vec![entry],
            settings: PlayerSettings {
                volume: 32,
                muted: true,
                shuffle: true,
                repeat: "queue".into(),
                queue_index: 0,
                position_seconds: 78.0,
            },
        }
    }

    #[test]
    fn remembered_login_survives_restart_without_leaking_into_library() {
        let directory = TestDirectory::new();
        let path = directory.0.join("music.json");
        let mut store = MusicStore::open(Some(path.clone())).unwrap();
        let id = store
            .remember_login("alice", "private-token".into(), "")
            .unwrap();
        let mut restored = MusicStore::open(Some(path)).unwrap();
        assert_eq!(
            restored.remembered_login(&id, "private-token"),
            Some(("alice".into(), "private-token".into()))
        );
        assert!(restored.remembered_login(&id, "rotated-token").is_none());
        assert!(restored
            .remembered_login("invalid", "private-token")
            .is_none());
        let public: serde_json::Value = async_std::task::block_on(
            restored
                .record("alice")
                .public_response()
                .take_body()
                .into_json(),
        )
        .unwrap();
        assert!(!public.to_string().contains("private-token"));
        assert!(public.get("logins").is_none());
        let mut expired = restored.record("alice");
        expired.logins[0].expires_at = 0;
        restored.commit(expired).unwrap();
        assert!(restored.remembered_login(&id, "private-token").is_none());
        restored.forget_login("alice", &id).unwrap();
        assert!(restored.remembered_login(&id, "private-token").is_none());
    }

    #[async_std::test]
    async fn remembered_login_routes_require_credentials_and_restore_only_with_cookie() {
        let mut app = tide::new();
        mount_with_token(&mut app, Some("test-token".into()));
        let url = Url::parse("https://localhost/youtubemusic/session").unwrap();
        let request = HttpRequest::new(Method::Post, url.clone());
        let response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Unauthorized);
        let mut request = HttpRequest::new(Method::Post, url.clone());
        request.insert_header("X-YouTubeMusic-Token", "test-token");
        request.insert_header("X-YouTubeMusic-Username", "alice");
        let response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        let cookie = response.header("Set-Cookie").unwrap().last().as_str();
        for attribute in ["Secure", "HttpOnly", "SameSite=Strict", "Max-Age=2592000"] {
            assert!(cookie.contains(attribute));
        }
        let cookie = cookie.split(';').next().unwrap().to_owned();
        for value in [None, Some("ytm_session=invalid"), Some(cookie.as_str())] {
            let mut request = HttpRequest::new(Method::Get, url.clone());
            if let Some(value) = value {
                request.insert_header("Cookie", value);
            }
            let mut response: Response = app.respond(request).await.unwrap();
            assert_eq!(
                response.header("Cache-Control").unwrap().last().as_str(),
                "no-store"
            );
            let body: serde_json::Value = response.take_body().into_json().await.unwrap();
            if value == Some(cookie.as_str()) {
                assert_eq!(body["username"], "alice");
                assert_eq!(body["token"], "test-token");
            } else {
                assert!(body.is_null());
            }
        }
        let mut request = HttpRequest::new(Method::Delete, url.clone());
        request.insert_header("Cookie", cookie.as_str());
        request.insert_header("X-YouTubeMusic-Token", "test-token");
        request.insert_header("X-YouTubeMusic-Username", "alice");
        let response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        let mut request = HttpRequest::new(Method::Get, url);
        request.insert_header("Cookie", cookie.as_str());
        let mut response: Response = app.respond(request).await.unwrap();
        let body: serde_json::Value = response.take_body().into_json().await.unwrap();
        assert!(body.is_null());
    }

    #[test]
    fn partitioned_music_records_survive_restart_without_reviving_live_playback() {
        let directory = TestDirectory::new();
        let path = directory.0.join("music.json");
        let mut store = MusicStore::open(Some(path.clone())).unwrap();
        let first = store.save_library("alice", 0, library()).unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(
            store
                .save_library("alice", 0, library())
                .err()
                .unwrap()
                .status(),
            StatusCode::Conflict
        );
        store
            .save_playback(
                "alice",
                Playback {
                    title: "Test song".into(),
                    artist: "Test artist".into(),
                    video_id: "abcdefghijk".into(),
                    is_playing: true,
                },
            )
            .unwrap();
        let rows: PartitionedArray = persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(rows.non_empty_ids().len(), 1);
        let restored = MusicStore::open(Some(path)).unwrap();
        assert!(restored.record("alice").library == library());
        assert_eq!(restored.record("alice").playback.title, "Test song");
        assert_eq!(restored.record("alice").revision, 1);
        assert!(restored.record("bob").library.queue.is_empty());
        assert!(restored.live.is_empty());
        let state = PlaybackState {
            token: None,
            store: Arc::new(Mutex::new(restored)),
        };
        let result: serde_json::Value =
            async_std::task::block_on(state.snapshot("alice").take_body().into_json()).unwrap();
        assert_eq!(result["stale"], true);
        assert_eq!(result["is_playing"], false);
    }

    #[test]
    fn failed_music_save_and_corrupt_snapshot_do_not_reset_or_mutate_data() {
        let directory = TestDirectory::new();
        let path = directory.0.join("music.json");
        let mut store = MusicStore::open(Some(path.clone())).unwrap();
        store.save_library("alice", 0, library()).unwrap();
        let before = std::fs::read(&path).unwrap();
        store.path = Some(directory.0.clone());
        let mut changed = library();
        changed.settings.volume = 55;
        assert!(store.save_library("alice", 1, changed).is_err());
        assert_eq!(store.record("alice").library.settings.volume, 32);
        assert_eq!(store.record("alice").revision, 1);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(&path, "invalid snapshot").unwrap();
        assert!(MusicStore::open(Some(path)).is_err());
    }

    #[async_std::test]
    async fn library_routes_require_tokens_isolate_names_and_reject_conflicting_or_invalid_writes()
    {
        let mut app = tide::new();
        mount_with_token(&mut app, Some("test-token".into()));
        for (method, owner, token, body, expected) in [
            (
                Method::Get,
                "alice",
                None,
                String::new(),
                StatusCode::Unauthorized,
            ),
            (
                Method::Put,
                "alice",
                None,
                "{}".into(),
                StatusCode::Unauthorized,
            ),
            (
                Method::Put,
                "alice",
                Some("test-token"),
                serde_json::json!({"revision":0,"library":library()}).to_string(),
                StatusCode::Ok,
            ),
            (
                Method::Get,
                "alice",
                Some("test-token"),
                String::new(),
                StatusCode::Ok,
            ),
            (
                Method::Get,
                "bob",
                Some("test-token"),
                String::new(),
                StatusCode::Ok,
            ),
            (
                Method::Put,
                "alice",
                Some("test-token"),
                serde_json::json!({"revision":0,"library":library()}).to_string(),
                StatusCode::Conflict,
            ),
            (
                Method::Put,
                "alice",
                Some("test-token"),
                r#"{"revision":1,"library":{"queue":[{"videoId":"bad"}]}}"#.into(),
                StatusCode::BadRequest,
            ),
            (
                Method::Put,
                "alice",
                Some("test-token"),
                "x".repeat(MAX_LIBRARY_BYTES as usize + 1),
                StatusCode::PayloadTooLarge,
            ),
        ] {
            let mut req = HttpRequest::new(
                method,
                Url::parse("http://localhost/youtubemusic/library").unwrap(),
            );
            if let Some(token) = token {
                req.insert_header("X-YouTubeMusic-Token", token);
            }
            req.insert_header("X-YouTubeMusic-Username", owner);
            req.set_body(body);
            let mut response: Response = app.respond(req).await.unwrap();
            assert_eq!(response.status(), expected);
            if method == Method::Get && expected == StatusCode::Ok {
                assert_eq!(
                    response.header("Cache-Control").unwrap().last().as_str(),
                    "no-store"
                );
                let result: serde_json::Value = response.take_body().into_json().await.unwrap();
                assert_eq!(result["revision"], if owner == "alice" { 1 } else { 0 });
                assert_eq!(
                    result["library"]["queue"].as_array().unwrap().len(),
                    if owner == "alice" { 1 } else { 0 }
                );
                assert!(!result.to_string().contains("test-token"));
            }
        }
    }

    async fn request(
        app: &tide::Server<()>,
        method: Method,
        token: Option<&str>,
        body: &str,
    ) -> Response {
        let mut request =
            HttpRequest::new(method, Url::parse("http://localhost/youtubemusic").unwrap());
        if let Some(token) = token {
            request.insert_header("X-YouTubeMusic-Token", token);
        }
        request.set_body(body.to_owned());
        app.respond(request).await.unwrap()
    }

    #[async_std::test]
    async fn serves_player_without_secrets_and_validates_metadata_requests() {
        let mut app = tide::new();
        mount_with_token(&mut app, Some("private-test-token".into()));
        let mut response: Response = app
            .respond(HttpRequest::new(
                Method::Get,
                Url::parse("http://localhost/youtubemusic/player").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        let body = response.take_body().into_string().await.unwrap();
        assert!(body.contains("YouTube Music"));
        assert!(!body.contains("private-test-token"));
        for (token, expected) in [
            (None, StatusCode::Unauthorized),
            (Some("private-test-token"), StatusCode::BadRequest),
        ] {
            let mut request = HttpRequest::new(
                Method::Get,
                Url::parse("http://localhost/youtubemusic/metadata?video_id=invalid").unwrap(),
            );
            if let Some(token) = token {
                request.insert_header("X-YouTubeMusic-Token", token);
            }
            let response: Response = app.respond(request).await.unwrap();
            assert_eq!(response.status(), expected);
        }
    }

    #[async_std::test]
    async fn authenticated_playback_round_trip_and_pause() {
        let mut app = tide::new();
        mount_with_token(&mut app, Some("test-token".into()));
        let mut empty = request(&app, Method::Get, Some("test-token"), "").await;
        let empty: serde_json::Value = empty.take_body().into_json().await.unwrap();
        assert_eq!(empty["stale"], true);
        assert_eq!(empty["is_playing"], false);
        for playing in [true, false] {
            let body = serde_json::json!({"title": "Test song", "artist": "Test artist",
                "video_id": "abcdefghijk", "is_playing": playing})
            .to_string();
            assert_eq!(
                request(&app, Method::Post, Some("test-token"), &body)
                    .await
                    .status(),
                StatusCode::Ok
            );
            let mut response = request(&app, Method::Get, Some("test-token"), "").await;
            assert_eq!(
                response.header("Cache-Control").unwrap().last().as_str(),
                "no-store"
            );
            let result: serde_json::Value = response.take_body().into_json().await.unwrap();
            assert_eq!(result["title"], "Test song");
            assert_eq!(result["artist"], "Test artist");
            assert_eq!(result["is_playing"], playing);
            assert_eq!(result["stale"], false);
            assert_eq!(
                result["url"],
                "https://music.youtube.com/watch?v=abcdefghijk"
            );
        }
    }

    #[async_std::test]
    async fn rejects_unauthorized_unconfigured_and_invalid_updates() {
        let mut app = tide::new();
        mount_with_token(&mut app, Some("test-token".into()));
        for method in [Method::Get, Method::Post] {
            assert_eq!(
                request(&app, method, None, "{}").await.status(),
                StatusCode::Unauthorized
            );
            assert_eq!(
                request(&app, method, Some("wrong"), "{}").await.status(),
                StatusCode::Unauthorized
            );
        }
        for body in ["{}".to_owned(), "not json".to_owned(),
            serde_json::json!({"title": "Song", "artist": "Artist", "video_id": "invalid", "is_playing": true}).to_string()] {
            assert_eq!(request(&app, Method::Post, Some("test-token"), &body).await.status(), StatusCode::BadRequest);
        }
        assert_eq!(
            request(&app, Method::Post, Some("test-token"), &"x".repeat(4097))
                .await
                .status(),
            StatusCode::PayloadTooLarge
        );
        let mut unconfigured = tide::new();
        mount_with_token(&mut unconfigured, None);
        assert_eq!(
            request(&unconfigured, Method::Get, None, "").await.status(),
            StatusCode::ServiceUnavailable
        );
    }

    #[test]
    fn expires_old_playback() {
        let mut store = MusicStore::open(None).unwrap();
        store
            .save_playback(
                "default",
                Playback {
                    title: "Old song".into(),
                    artist: "Artist".into(),
                    video_id: "abcdefghijk".into(),
                    is_playing: true,
                },
            )
            .unwrap();
        store
            .live
            .insert("default".into(), Instant::now() - STALE_AFTER);
        let state = PlaybackState {
            token: Some("test-token".into()),
            store: Arc::new(Mutex::new(store)),
        };
        let mut response = state.snapshot("default");
        let result: serde_json::Value =
            async_std::task::block_on(response.take_body().into_json()).unwrap();
        assert_eq!(result["stale"], true);
        assert_eq!(result["is_playing"], false);
        assert_eq!(result["title"], "");
    }

    #[async_std::test]
    async fn usernames_select_separate_feeds_and_are_validated() {
        let mut app = tide::new();
        mount_with_token(&mut app, Some("test-token".into()));
        for (name, method, status) in [
            ("Alice", Method::Post, StatusCode::Ok),
            ("alice", Method::Get, StatusCode::Ok),
            ("bob", Method::Get, StatusCode::Ok),
            ("bad name", Method::Get, StatusCode::BadRequest),
            ("", Method::Post, StatusCode::BadRequest),
        ] {
            let mut request =
                HttpRequest::new(method, Url::parse("http://localhost/youtubemusic").unwrap());
            request.insert_header("X-YouTubeMusic-Token", "test-token");
            request.insert_header("X-YouTubeMusic-Username", name);
            request.set_body(r#"{"title":"Alice song","artist":"Artist","video_id":"abcdefghijk","is_playing":true}"#);
            let mut response: Response = app.respond(request).await.unwrap();
            assert_eq!(response.status(), status);
            if status == StatusCode::Ok {
                let result: serde_json::Value = response.take_body().into_json().await.unwrap();
                assert_eq!(result["username"], name.to_ascii_lowercase());
                assert_eq!(result["stale"], name == "bob");
                assert_eq!(
                    result["title"],
                    if name == "bob" { "" } else { "Alice song" }
                );
            }
        }
    }
}
