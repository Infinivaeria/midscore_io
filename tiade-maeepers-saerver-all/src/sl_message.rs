use async_std::io::ReadExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tide::{Request, Response, StatusCode};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
struct Display {
    text: String,
    color: [f32; 3],
    alpha: f32,
}

impl Display {
    fn validate(&self) -> tide::Result<()> {
        if self.text.len() > 254
            || !self.color.iter().chain(std::iter::once(&self.alpha))
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        {
            return Err(error(StatusCode::BadRequest, "Text must fit 254 UTF-8 bytes; color and alpha must be between 0 and 1"));
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Object {
    id: String,
    secret: String,
    object_key: String,
    name: String,
    region: String,
    desired: Display,
    revision: i32,
    applied_revision: i32,
    reported_text: String,
    message: String,
    message_at: i64,
    #[serde(default)]
    last_seen: i64,
    #[serde(default = "initial_program_state")]
    state: serde_json::Value,
    #[serde(default)]
    runtime: Runtime,
}

impl Object {
    fn online(&self, now: i64) -> bool {
        self.last_seen > 0 && now - self.last_seen < 60
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
struct Runtime {
    paused: bool,
    poll_seconds: u32,
    version: u64,
    runs: u64,
    last_run_at: i64,
    wake_at: i64,
    logs: Vec<String>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self { paused: false, poll_seconds: 15, version: 0, runs: 0,
            last_run_at: 0, wake_at: 0, logs: Vec::new() }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeUpdate {
    expected_version: u64,
    paused: Option<bool>,
    poll_seconds: Option<u32>,
    state: Option<serde_json::Value>,
    #[serde(default)]
    reset_state: bool,
    #[serde(default)]
    clear_logs: bool,
    #[serde(default)]
    wake_now: bool,
}

fn initial_program_state() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandlerResult {
    state: serde_json::Value,
    display: Option<Display>,
    wake_at: Option<i64>,
    #[serde(default)]
    logs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Poll {
    id: String,
    secret: String,
    object_key: String,
    name: String,
    region: String,
    applied_revision: i32,
    reported_text: String,
    color: [f32; 3],
    alpha: f32,
    message: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct RubyRows {
    backend: String,
    db_size: usize,
    partition_amount_and_offset: usize,
    data_arr: Vec<serde_json::Map<String, serde_json::Value>>,
}

fn ruby_store_call(args: Vec<String>) -> tide::Result<String> {
    crate::ruby_vm::start().map_err(|_| error(StatusCode::InternalServerError, "Ruby storage VM unavailable"))?;
    async_std::task::block_on(crate::ruby_vm::call_with_strings(
        include_str!("../ruby_client/sl_message_partitioned_array.rb"), args, crate::ruby_vm::DEFAULT_TIMEOUT,
    )).map_err(|failure| {
        eprintln!("SL Message Ruby operation failed: {failure}");
        error(StatusCode::InternalServerError, "Ruby storage operation failed")
    })
}

fn ruby_array_path() -> String {
    std::env::var("SL_MESSAGE_RUBY_ARRAY").unwrap_or_else(|_|
        concat!(env!("CARGO_MANIFEST_DIR"), "/../lib/partitioned_array/lib/partitioned_array.rb").into())
}

struct Store {
    path: Option<PathBuf>,
    rows: RubyRows,
    objects: BTreeMap<String, Object>,
    handler: Option<String>,
}

impl Store {
    fn new(path: Option<PathBuf>) -> Self {
        let rows = RubyRows { backend: "ruby-partitioned-array-v1".into(), db_size: 1,
            partition_amount_and_offset: 32, data_arr: Vec::new() };
        Self { path, rows, objects: BTreeMap::new(), handler: None }
    }

    fn open(path: PathBuf) -> tide::Result<Self> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Snapshot {
            Ruby(RubyRows),
            Partitioned(partitioned_array_rust::PartitionedArray),
            Legacy(BTreeMap<String, Object>),
        }

        let snapshot = crate::persistent_snapshot::load::<Snapshot>(&path)?;
        let mut store = Self::new(Some(path));
        match snapshot {
            Some(Snapshot::Ruby(rows)) => {
                if rows.backend != "ruby-partitioned-array-v1" {
                    return Err(error(StatusCode::InternalServerError, "Unknown Ruby registry format"));
                }
                for row in rows.data_arr.iter().filter(|row| !row.is_empty()) {
                    let object: Object = serde_json::from_value(serde_json::Value::Object(row.clone()))?;
                    if store.objects.insert(object.id.clone(), object).is_some() {
                        return Err(error(StatusCode::InternalServerError, "Duplicate prim ID in message storage"));
                    }
                }
                store.rows = rows;
            }
            Some(Snapshot::Partitioned(rows)) => {
                for row in rows.data_arr.iter().filter(|row| !row.is_empty()) {
                    let object: Object = serde_json::from_value(serde_json::Value::Object(row.clone()))?;
                    let id = object.id.clone();
                    if store.objects.insert(id, object).is_some() {
                        return Err(error(StatusCode::InternalServerError, "Duplicate prim ID in message storage"));
                    }
                }
                store.commit(store.objects.clone())?;
            }
            Some(Snapshot::Legacy(objects)) => {
                if objects.iter().any(|(id, object)| id != &object.id) {
                    return Err(error(StatusCode::InternalServerError, "Invalid prim ID in legacy message storage"));
                }
                store.commit(objects)?;
            }
            None => {}
        }
        Ok(store)
    }

    fn commit(&mut self, objects: BTreeMap<String, Object>) -> tide::Result<()> {
        let records = serde_json::to_string(&objects.values().collect::<Vec<_>>())?;
        let rows: RubyRows = serde_json::from_str(&ruby_store_call(vec![ruby_array_path(), records])?)?;
        if let Some(path) = &self.path {
            crate::persistent_snapshot::save(path, &rows)?;
        }
        self.rows = rows;
        self.objects = objects;
        Ok(())
    }

    fn poll(&mut self, poll: Poll) -> tide::Result<serde_json::Value> {
        for value in [&poll.id, &poll.secret, &poll.object_key] {
            if uuid::Uuid::parse_str(value).is_err() || value.len() != 36 {
                return Err(error(StatusCode::BadRequest, "Invalid object identity"));
            }
        }
        let display = Display { text: poll.reported_text.clone(), color: poll.color, alpha: poll.alpha };
        display.validate()?;
        if poll.name.len() > 128 || poll.region.len() > 128 || poll.message.len() > 512 || poll.applied_revision < -1 {
            return Err(error(StatusCode::BadRequest, "Invalid object report"));
        }
        let mut objects = self.objects.clone();
        let is_new = !objects.contains_key(&poll.id);
        if !objects.contains_key(&poll.id) {
            if poll.applied_revision != -1 {
                return Err(error(StatusCode::Conflict, "Register again with revision -1"));
            }
            objects.insert(poll.id.clone(), Object {
                id: poll.id.clone(), secret: poll.secret.clone(), object_key: poll.object_key.clone(),
                name: String::new(), region: String::new(), desired: display,
                revision: 0, applied_revision: -1, reported_text: String::new(),
                message: String::new(), message_at: 0, last_seen: 0,
                state: initial_program_state(),
                runtime: Runtime::default(),
            });
        }
        let object = objects.get_mut(&poll.id).unwrap();
        if object.secret != poll.secret || object.object_key != poll.object_key {
            return Err(error(StatusCode::Unauthorized, "Object credentials do not match"));
        }
        if poll.applied_revision > object.revision {
            return Err(error(StatusCode::Conflict, "Register again with revision -1"));
        }
        let now = chrono::Utc::now().timestamp();
        let previous_message = object.message.clone();
        object.name = poll.name;
        object.region = poll.region;
        object.applied_revision = poll.applied_revision;
        object.reported_text = poll.reported_text;
        if object.message != poll.message {
            object.message_at = now;
            object.message = poll.message;
        }
        object.last_seen = now;
        if let Some(handler) = self.handler.as_ref().filter(|_| !object.runtime.paused && now >= object.runtime.wake_at) {
            let mut input = serde_json::to_value(&*object)?;
            input.as_object_mut().unwrap().remove("secret");
            input["event"] = serde_json::json!(if is_new { "register" } else if previous_message != object.message { "message" } else { "poll" });
            input["previous_message"] = serde_json::json!(previous_message);
            input["now"] = serde_json::json!(now);
            let result: HandlerResult = serde_json::from_str(&ruby_store_call(vec![
                ruby_array_path(), input.to_string(), "handler".into(), handler.clone(),
            ])?).map_err(|_| error(StatusCode::InternalServerError, "Invalid Ruby handler result"))?;
            if result.logs.len() > 16 || result.logs.iter().any(|entry| entry.len() > 512)
                || result.wake_at.is_some_and(|wake_at| wake_at < 0)
            {
                return Err(error(StatusCode::InternalServerError, "Invalid Ruby handler logs or wake time"));
            }
            if let Some(display) = result.display {
                display.validate()?;
                if display != object.desired {
                    object.revision = object.revision.checked_add(1)
                        .ok_or_else(|| error(StatusCode::Conflict, "Revision limit reached"))?;
                    object.desired = display;
                }
            }
            object.state = result.state;
            object.runtime.version = object.runtime.version.checked_add(1)
                .ok_or_else(|| error(StatusCode::Conflict, "Runtime version limit reached"))?;
            object.runtime.runs = object.runtime.runs.checked_add(1)
                .ok_or_else(|| error(StatusCode::Conflict, "Runtime run limit reached"))?;
            object.runtime.last_run_at = now;
            object.runtime.wake_at = result.wake_at.unwrap_or(0);
            object.runtime.logs.extend(result.logs);
            let excess = object.runtime.logs.len().saturating_sub(64);
            object.runtime.logs.drain(..excess);
        }
        let response = serde_json::json!({"revision": object.revision, "text": object.desired.text,
            "color": object.desired.color, "alpha": object.desired.alpha, "poll_seconds": object.runtime.poll_seconds});
        self.commit(objects)?;
        Ok(response)
    }

    fn update_runtime(&mut self, id: &str, update: RuntimeUpdate) -> tide::Result<serde_json::Value> {
        if update.poll_seconds.is_some_and(|seconds| !(15..=120).contains(&seconds))
            || (update.reset_state && update.state.is_some())
        {
            return Err(error(StatusCode::BadRequest, "Polling must be 15-120 seconds; choose state or reset_state"));
        }
        let mut objects = self.objects.clone();
        let object = objects.get_mut(id).ok_or_else(|| error(StatusCode::NotFound, "Unknown object"))?;
        if update.expected_version != object.runtime.version {
            return Err(error(StatusCode::Conflict, "Runtime changed; reload before updating"));
        }
        if let Some(paused) = update.paused { object.runtime.paused = paused; }
        if let Some(seconds) = update.poll_seconds { object.runtime.poll_seconds = seconds; }
        if let Some(state) = update.state { object.state = state; }
        if update.reset_state { object.state = initial_program_state(); }
        if update.clear_logs { object.runtime.logs.clear(); }
        if update.wake_now || update.reset_state { object.runtime.wake_at = 0; }
        object.runtime.version = object.runtime.version.checked_add(1)
            .ok_or_else(|| error(StatusCode::Conflict, "Runtime version limit reached"))?;
        let response = serde_json::json!({"state": object.state, "runtime": object.runtime});
        self.commit(objects)?;
        Ok(response)
    }

    fn delete_offline(&mut self, id: &str) -> tide::Result<()> {
        let object = self.objects.get(id).ok_or_else(|| error(StatusCode::NotFound, "Unknown object"))?;
        if object.online(chrono::Utc::now().timestamp()) {
            return Err(error(StatusCode::Conflict, "Object is online; only offline objects can be deleted"));
        }
        let mut objects = self.objects.clone();
        objects.remove(id);
        self.commit(objects)
    }

    fn set_display(&mut self, id: &str, display: Display) -> tide::Result<i32> {
        display.validate()?;
        let mut objects = self.objects.clone();
        let object = objects.get_mut(id).ok_or_else(|| error(StatusCode::NotFound, "Unknown object"))?;
        object.revision = object.revision.checked_add(1)
            .ok_or_else(|| error(StatusCode::Conflict, "Revision limit reached"))?;
        object.desired = display;
        let revision = object.revision;
        self.commit(objects)?;
        Ok(revision)
    }
}

#[derive(Clone)]
struct Bridge {
    store: Arc<Mutex<Store>>,
    admin_token: Option<String>,
    device_token: Option<String>,
}

fn error(status: StatusCode, message: &str) -> tide::Error {
    tide::Error::from_str(status, message.to_owned())
}

fn authorize(req: &Request<Bridge>, token: &Option<String>) -> tide::Result<()> {
    let token = token.as_ref().filter(|value| !value.trim().is_empty())
        .ok_or_else(|| error(StatusCode::ServiceUnavailable, "Configure SL_MESSAGE_ADMIN_TOKEN and SL_MESSAGE_DEVICE_TOKEN on the server"))?;
    if req.header("Authorization").map(|value| value.last().as_str()) != Some(format!("Bearer {token}").as_str()) {
        return Err(error(StatusCode::Unauthorized, "Invalid access token"));
    }
    Ok(())
}

async fn read_json<T: serde::de::DeserializeOwned>(req: &mut Request<Bridge>) -> tide::Result<T> {
    let mut bytes = Vec::new();
    req.take_body().take(4097).read_to_end(&mut bytes).await?;
    if bytes.len() > 4096 {
        return Err(error(StatusCode::PayloadTooLarge, "Request exceeds 4096 bytes"));
    }
    serde_json::from_slice(&bytes).map_err(|_| error(StatusCode::BadRequest, "Invalid JSON request"))
}

fn json(value: impl Serialize) -> Response {
    let mut response = crate::json_response(value);
    response.insert_header("Cache-Control", "no-store");
    response
}

fn control_page() -> Response {
    let mut response = Response::new(StatusCode::Ok);
    response.set_content_type(tide::http::mime::HTML);
    response.insert_header("Cache-Control", "no-store");
    response.insert_header("X-Content-Type-Options", "nosniff");
    response.set_body(include_str!("sl_message.html"));
    response
}

fn mount_bridge<State: Clone + Send + Sync + 'static>(app: &mut tide::Server<State>, bridge: Bridge) {
    app.at("/slmessage/").get(|_| async { Ok(control_page()) });
    app.at("/slmessage").nest(service(bridge));
}

fn service(bridge: Bridge) -> tide::Server<Bridge> {
    let mut app = tide::with_state(bridge);
    app.at("/").get(|_| async { Ok(control_page()) });
    app.at("/connect").post(|req: Request<Bridge>| async move {
        authorize(&req, &req.state().admin_token)?;
        let store = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?;
        if let Some(path) = &store.path {
            persisted_admin_token(&path.with_extension("admin.json"), req.state().admin_token.clone())?;
        }
        Ok(json(serde_json::json!({"connected": true})))
    });
    app.at("/script.lsl").get(|_| async {
        let mut response = Response::new(StatusCode::Ok);
        response.insert_header("Content-Type", "text/plain; charset=utf-8");
        response.set_body(include_str!("../second_life/sl_message.lsl"));
        Ok(response)
    });
    app.at("/objects").get(|req: Request<Bridge>| async move {
        authorize(&req, &req.state().admin_token)?;
        let store = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?;
        let now = chrono::Utc::now().timestamp();
        let objects: Vec<_> = store.objects.values().map(|object| serde_json::json!({
            "id": object.id, "object_key": object.object_key, "name": object.name, "region": object.region,
            "desired": object.desired, "revision": object.revision, "applied_revision": object.applied_revision,
            "reported_text": object.reported_text, "message": object.message, "message_at": object.message_at,
            "state": object.state,
            "runtime": object.runtime,
            "last_seen": object.last_seen, "online": object.online(now),
        })).collect();
        Ok(json(serde_json::json!({"objects": objects})))
    });
    app.at("/objects/:id").post(|mut req: Request<Bridge>| async move {
        authorize(&req, &req.state().admin_token)?;
        let id = req.param("id")?.to_string();
        let display: Display = read_json(&mut req).await?;
        let revision = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?
            .set_display(&id, display)?;
        Ok(json(serde_json::json!({"revision": revision})))
    }).delete(|req: Request<Bridge>| async move {
        authorize(&req, &req.state().admin_token)?;
        let id = req.param("id")?;
        req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?
            .delete_offline(id)?;
        Ok(json(serde_json::json!({"deleted": id})))
    });
    app.at("/objects/:id/runtime").get(|req: Request<Bridge>| async move {
        authorize(&req, &req.state().admin_token)?;
        let store = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?;
        let object = store.objects.get(req.param("id")?)
            .ok_or_else(|| error(StatusCode::NotFound, "Unknown object"))?;
        Ok(json(serde_json::json!({"state": object.state, "runtime": object.runtime,
            "handler_enabled": store.handler.is_some(), "api_version": "2.0"})))
    }).post(|mut req: Request<Bridge>| async move {
        authorize(&req, &req.state().admin_token)?;
        let id = req.param("id")?.to_string();
        let update: RuntimeUpdate = read_json(&mut req).await?;
        let result = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?
            .update_runtime(&id, update)?;
        Ok(json(result))
    });
    app.at("/poll").post(|mut req: Request<Bridge>| async move {
        authorize(&req, &req.state().device_token)?;
        let poll: Poll = read_json(&mut req).await?;
        let result = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?.poll(poll)?;
        Ok(json(result))
    });
    app
}

const DEFAULT_ADMIN_TOKEN: &str = "slmessage-admin";

fn admin_token_or_default(configured: Option<String>) -> String {
    configured.unwrap_or_else(|| DEFAULT_ADMIN_TOKEN.to_owned())
}

fn persisted_admin_token(path: &std::path::Path, configured: Option<String>) -> tide::Result<String> {
    use partitioned_array_rust::PartitionedArray;
    let saved = crate::persistent_snapshot::load::<PartitionedArray>(path)?;
    let stored_token = saved.as_ref().map(|rows| {
        let records: Vec<_> = rows.data_arr.iter().filter(|row| !row.is_empty()).collect();
        if records.len() != 1 {
            return Err(error(StatusCode::InternalServerError, "Invalid SL Message admin token storage"));
        }
        records[0].get("admin_token").and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| error(StatusCode::InternalServerError, "Invalid SL Message admin token storage"))
    }).transpose()?;
    let token = admin_token_or_default(configured.or_else(|| stored_token.clone()));
    if stored_token.as_ref() != Some(&token) {
        let mut rows = PartitionedArray::new(1, 1, 1, false);
        rows.allocate(false);
        rows.add(|row| { row.insert("admin_token".into(), serde_json::json!(token)); })
            .ok_or_else(|| error(StatusCode::InternalServerError, "Admin token storage is full"))?;
        crate::persistent_snapshot::save(path, &rows)?;
    }
    Ok(token)
}

pub fn mount<State: Clone + Send + Sync + 'static>(app: &mut tide::Server<State>) -> tide::Result<()> {
    let path = PathBuf::from(std::env::var("SL_MESSAGE_STORE").unwrap_or_else(|_| "sl_message_data/objects.json".into()));
    let admin_token = persisted_admin_token(&path.with_extension("admin.json"), std::env::var("SL_MESSAGE_ADMIN_TOKEN").ok())?;
    let mut store = Store::open(path)?;
    store.handler = std::env::var("SL_MESSAGE_HANDLER").ok()
        .map(std::fs::read_to_string).transpose()?;
    let bridge = Bridge {
        store: Arc::new(Mutex::new(store)),
        admin_token: Some(admin_token),
        device_token: std::env::var("SL_MESSAGE_DEVICE_TOKEN").ok(),
    };
    mount_bridge(app, bridge);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report(id: &str) -> Poll {
        Poll { id: id.into(), secret: uuid::Uuid::new_v4().to_string(), object_key: uuid::Uuid::new_v4().to_string(),
            name: "Prim".into(), region: "Region".into(), applied_revision: -1,
            reported_text: "Original".into(), color: [1.0; 3], alpha: 1.0, message: String::new() }
    }

    #[test]
    fn per_prim_delivery_retries_acknowledgment_and_local_messages() {
        let mut store = Store::new(None);
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        let mut poll = report(&first);
        let secret = poll.secret.clone();
        let object_key = poll.object_key.clone();
        assert_eq!(store.poll(poll).unwrap()["text"], "Original");
        store.poll(report(&second)).unwrap();
        store.set_display(&first, Display { text: "New\ntext".into(), color: [0.2, 0.8, 1.0], alpha: 0.5 }).unwrap();
        for _ in 0..2 {
            poll = report(&first); poll.secret = secret.clone(); poll.object_key = object_key.clone();
            let reply = store.poll(poll).unwrap();
            assert_eq!(reply["text"], "New\ntext");
            assert_eq!(reply["revision"], 1);
            assert_eq!(store.objects[&first].applied_revision, -1);
        }
        assert_eq!(store.objects[&second].desired.text, "Original");
        poll = report(&first); poll.secret = secret; poll.object_key = object_key;
        poll.applied_revision = 1; poll.reported_text = "New\ntext".into(); poll.message = "Hello from SL".into();
        store.poll(poll).unwrap();
        assert_eq!(store.objects[&first].message, "Hello from SL");
        assert_eq!(store.objects[&first].applied_revision, 1);
        assert!(store.poll(report(&first)).is_err());
        assert!(store.set_display(&first, Display { text: "x".repeat(255), color: [1.0; 3], alpha: 1.0 }).is_err());
    }

    #[async_std::test]
    async fn mounted_routes_authentication_and_json_contract() {
        let bridge = Bridge { store: Arc::new(Mutex::new(Store::new(None))),
            admin_token: Some(admin_token_or_default(None)), device_token: Some("device".into()) };
        let mut app = tide::new();
        mount_bridge(&mut app, bridge);
        for path in ["/slmessage", "/slmessage/", "/slmessage/script.lsl"] {
            let response: Response = app.respond(tide::http::Request::new(tide::http::Method::Get,
                format!("http://localhost{path}").parse::<tide::http::Url>().unwrap())).await.unwrap();
            assert_eq!(response.status(), StatusCode::Ok, "{path}");
        }
        let mut request = tide::http::Request::new(tide::http::Method::Get, tide::http::Url::parse("http://localhost/slmessage/objects").unwrap());
        let response: Response = app.respond(request.clone()).await.unwrap();
        assert_eq!(response.status(), StatusCode::Unauthorized);
        request.insert_header("Authorization", format!("Bearer {DEFAULT_ADMIN_TOKEN}"));
        let mut response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        assert_eq!(response.take_body().into_json::<serde_json::Value>().await.unwrap(), json!({"objects": []}));
        let poll = report(&uuid::Uuid::new_v4().to_string());
        let mut request = tide::http::Request::new(tide::http::Method::Post, tide::http::Url::parse("http://localhost/slmessage/poll").unwrap());
        request.insert_header("Authorization", "Bearer device");
        request.set_body(serde_json::to_string(&json!({"id": poll.id, "secret": poll.secret,
            "object_key": poll.object_key, "name": poll.name, "region": poll.region, "applied_revision": -1,
            "reported_text": "Hello", "color": [1,1,1], "alpha": 1, "message": ""})).unwrap());
        let mut response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        assert_eq!(response.take_body().into_json::<serde_json::Value>().await.unwrap()["text"], "Hello");
    }

    #[test]
    fn admin_token_defaults_only_when_unset() {
        assert_eq!(admin_token_or_default(None), "slmessage-admin");
        assert_eq!(admin_token_or_default(Some("custom-admin".into())), "custom-admin");
        assert_eq!(admin_token_or_default(Some(String::new())), "");
    }

    #[async_std::test]
    async fn connect_persists_only_authenticated_admin_tokens() {
        let directory = std::env::temp_dir().join(format!("sl-connect-{}", uuid::Uuid::new_v4()));
        let path = directory.join("objects.json");
        let admin_path = path.with_extension("admin.json");
        let mut app = tide::new();
        mount_bridge(&mut app, Bridge {
            store: Arc::new(Mutex::new(Store::new(Some(path)))),
            admin_token: Some("connect-admin".into()), device_token: Some("device".into()),
        });
        for token in [None, Some("device"), Some("wrong-admin"), Some("connect-admin")] {
            let mut request = tide::http::Request::new(tide::http::Method::Post,
                tide::http::Url::parse("http://localhost/slmessage/connect").unwrap());
            if let Some(token) = token {
                request.insert_header("Authorization", format!("Bearer {token}"));
            }
            let mut response: Response = app.respond(request).await.unwrap();
            if token == Some("connect-admin") {
                assert_eq!(response.status(), StatusCode::Ok);
                assert_eq!(response.take_body().into_json::<serde_json::Value>().await.unwrap(), json!({"connected": true}));
            } else {
                assert_eq!(response.status(), StatusCode::Unauthorized);
                assert!(!admin_path.exists());
            }
        }
        let rows: partitioned_array_rust::PartitionedArray = crate::persistent_snapshot::load(&admin_path).unwrap().unwrap();
        assert_eq!(rows.data_arr.iter().find(|row| !row.is_empty()).unwrap()["admin_token"], "connect-admin");
        assert_eq!(persisted_admin_token(&admin_path, None).unwrap(), "connect-admin");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn admin_token_persists_in_rust_array_and_can_be_rotated_or_disabled() {
        let directory = std::env::temp_dir().join(format!("sl-admin-{}", uuid::Uuid::new_v4()));
        let path = directory.join("objects.admin.json");
        assert_eq!(persisted_admin_token(&path, None).unwrap(), DEFAULT_ADMIN_TOKEN);
        assert_eq!(persisted_admin_token(&path, Some("custom-admin".into())).unwrap(), "custom-admin");
        assert_eq!(persisted_admin_token(&path, None).unwrap(), "custom-admin");
        let rows: partitioned_array_rust::PartitionedArray = crate::persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(rows.data_arr.iter().find(|row| !row.is_empty()).unwrap()["admin_token"], "custom-admin");
        assert_eq!(persisted_admin_token(&path, Some(String::new())).unwrap(), "");
        assert_eq!(persisted_admin_token(&path, None).unwrap(), "");
        std::fs::write(&path, b"invalid snapshot").unwrap();
        assert!(persisted_admin_token(&path, None).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[async_std::test]
    async fn offline_deletion_requires_admin_and_persists_atomically() {
        let directory = std::env::temp_dir().join(format!("sl-delete-{}", uuid::Uuid::new_v4()));
        let path = directory.join("objects.json");
        let id = uuid::Uuid::new_v4().to_string();
        let mut store = Store::open(path.clone()).unwrap();
        store.poll(report(&id)).unwrap();
        let store = Arc::new(Mutex::new(store));
        let mut app = tide::new();
        mount_bridge(&mut app, Bridge { store: store.clone(), admin_token: Some("admin".into()), device_token: Some("device".into()) });
        let mut request = tide::http::Request::new(tide::http::Method::Delete,
            format!("http://localhost/slmessage/objects/{id}").parse::<tide::http::Url>().unwrap());
        for token in [None, Some("device"), Some("admin")] {
            if let Some(token) = token { request.insert_header("Authorization", format!("Bearer {token}")); }
            let response: Response = app.respond(request.clone()).await.unwrap();
            assert_eq!(response.status(), if token == Some("admin") { StatusCode::Conflict } else { StatusCode::Unauthorized });
            assert!(store.lock().unwrap().objects.contains_key(&id));
        }
        {
            let mut store = store.lock().unwrap();
            store.objects.get_mut(&id).unwrap().last_seen = chrono::Utc::now().timestamp() - 60;
            store.path = Some(directory.clone());
            assert!(store.delete_offline(&id).is_err());
            assert!(store.objects.contains_key(&id));
            assert!(Store::open(path.clone()).unwrap().objects.contains_key(&id));
            store.path = Some(path.clone());
        }
        let mut response: Response = app.respond(request.clone()).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        assert_eq!(response.take_body().into_json::<serde_json::Value>().await.unwrap()["deleted"], id);
        assert!(Store::open(path).unwrap().objects.is_empty());
        let response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NotFound);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[async_std::test]
    async fn multiple_objects_register_and_receive_independent_updates() {
        let bridge = Bridge { store: Arc::new(Mutex::new(Store::new(None))),
            admin_token: Some("admin".into()), device_token: Some("device".into()) };
        let mut app = tide::new();
        mount_bridge(&mut app, bridge);
        let reports: Vec<_> = (0..40).map(|index| {
            let poll = report(&uuid::Uuid::new_v4().to_string());
            json!({"id": poll.id, "secret": poll.secret, "object_key": poll.object_key,
                "name": "Same prim name", "region": poll.region, "applied_revision": -1,
                "reported_text": format!("Original {index}"), "color": [1, 1, 1], "alpha": 1,
                "message": format!("Message {index}")})
        }).collect();

        for updated in [false, true] {
            futures_util::future::join_all(reports.iter().enumerate().map(|(index, report)| {
                let app = &app;
                async move {
                    let mut request = tide::http::Request::new(tide::http::Method::Post,
                        "http://localhost/slmessage/poll".parse::<tide::http::Url>().unwrap());
                    request.insert_header("Authorization", "Bearer device");
                    request.set_body(serde_json::to_string(report).unwrap());
                    let mut response: Response = app.respond(request).await.unwrap();
                    assert_eq!(response.status(), StatusCode::Ok);
                    let reply = response.take_body().into_json::<serde_json::Value>().await.unwrap();
                    let expected = if updated { format!("Updated {index}") } else { format!("Original {index}") };
                    assert_eq!(reply["text"], expected);
                    assert_eq!(reply["revision"], if updated { 1 } else { 0 });
                    assert_eq!(reply["color"], if updated { json!([1.0, 0.0, 0.0]) } else { json!([1.0, 1.0, 1.0]) });
                    assert_eq!(reply["alpha"], if updated { 0.5 } else { 1.0 });
                }
            })).await;
            if !updated {
                futures_util::future::join_all(reports.iter().enumerate().map(|(index, report)| {
                    let app = &app;
                    async move {
                        let mut request = tide::http::Request::new(tide::http::Method::Post,
                            format!("http://localhost/slmessage/objects/{}", report["id"].as_str().unwrap())
                                .parse::<tide::http::Url>().unwrap());
                        request.insert_header("Authorization", "Bearer admin");
                        request.set_body(json!({"text": format!("Updated {index}"), "color": [1, 0, 0], "alpha": 0.5}).to_string());
                        let response: Response = app.respond(request).await.unwrap();
                        assert_eq!(response.status(), StatusCode::Ok);
                    }
                })).await;
            }
        }

        let mut impostor = reports[0].clone();
        impostor["secret"] = reports[1]["secret"].clone();
        impostor["object_key"] = reports[1]["object_key"].clone();
        let mut request = tide::http::Request::new(tide::http::Method::Post,
            "http://localhost/slmessage/poll".parse::<tide::http::Url>().unwrap());
        request.insert_header("Authorization", "Bearer device");
        request.set_body(impostor.to_string());
        let response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Unauthorized);

        let mut request = tide::http::Request::new(tide::http::Method::Get,
            "http://localhost/slmessage/objects".parse::<tide::http::Url>().unwrap());
        request.insert_header("Authorization", "Bearer admin");
        let mut response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        let registry = response.take_body().into_json::<serde_json::Value>().await.unwrap();
        let objects = registry["objects"].as_array().unwrap();
        assert_eq!(objects.len(), reports.len());
        for (index, report) in reports.iter().enumerate() {
            let object = objects.iter().find(|object| object["id"] == report["id"]).unwrap();
            assert_eq!(object["object_key"], report["object_key"]);
            assert_eq!(object["message"], format!("Message {index}"));
            assert_eq!(object["desired"]["text"], format!("Updated {index}"));
            assert_eq!(object["revision"], 1);
            assert!(object.get("secret").is_none());
        }
    }

    #[test]
    fn registry_grows_past_500_and_survives_restart_and_failed_saves() {
        let directory = std::env::temp_dir().join(format!("sl-message-{}", uuid::Uuid::new_v4()));
        let path = directory.join("objects.json");
        let mut store = Store::open(path.clone()).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let mut poll = report(&id);
        let secret = poll.secret.clone();
        let object_key = poll.object_key.clone();
        poll.message = "Hello from SL".into();
        store.poll(poll).unwrap();
        store.set_display(&id, Display { text: "Pending".into(), color: [0.2, 0.8, 1.0], alpha: 0.5 }).unwrap();
        for _ in 0..500 {
            store.poll(report(&uuid::Uuid::new_v4().to_string())).unwrap();
        }
        let rows: RubyRows = crate::persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(rows.backend, "ruby-partitioned-array-v1");
        assert_eq!(rows.data_arr.iter().filter(|row| !row.is_empty()).count(), 501);
        assert!(rows.db_size > 1);
        let mut restored = Store::open(path.clone()).unwrap();
        assert_eq!(serde_json::to_value(&restored.objects).unwrap(), serde_json::to_value(&store.objects).unwrap());
        assert!(restored.objects[&id].last_seen > 0);
        assert_eq!(restored.objects[&id].applied_revision, -1);
        let mut poll = report(&id);
        poll.secret = secret;
        poll.object_key = object_key;
        assert_eq!(restored.poll(poll).unwrap()["text"], "Pending");
        let mut poll = report(&id);
        poll.secret = restored.objects[&id].secret.clone();
        poll.object_key = restored.objects[&id].object_key.clone();
        poll.applied_revision = 1;
        poll.reported_text = "Pending".into();
        poll.message = "Acknowledged".into();
        restored.poll(poll).unwrap();
        let mut restored = Store::open(path.clone()).unwrap();
        assert_eq!(restored.objects[&id].applied_revision, 1);
        assert_eq!(restored.objects[&id].message, "Acknowledged");
        let before = std::fs::read(&path).unwrap();
        let before_rows = serde_json::to_value(&restored.rows).unwrap();
        restored.path = Some(directory.clone());
        assert!(restored.set_display(&id, Display { text: "Not saved".into(), color: [1.0; 3], alpha: 1.0 }).is_err());
        assert_eq!(restored.objects[&id].desired.text, "Pending");
        assert_eq!(serde_json::to_value(&restored.rows).unwrap(), before_rows);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn ruby_handler_state_is_isolated_durable_and_atomic() {
        let directory = std::env::temp_dir().join(format!("sl-handler-{}", uuid::Uuid::new_v4()));
        let path = directory.join("objects.json");
        let mut store = Store::open(path.clone()).unwrap();
        let handler = r#"lambda do |input|
          raise 'Secret leaked' if input.key?('secret')
          state = input.fetch('state')
          if state['last_message'] == input['message']
            {'state' => state}
          else
            total = state.fetch('total', 0)
            [1, 2, 3].each { |increment| total += increment }
            {'state' => {'total' => total, 'last_message' => input['message']},
             'display' => {'text' => "Total #{total}", 'color' => [0, 1, 0], 'alpha' => 1}}
          end
        end"#.to_string();
        store.handler = Some(handler.clone());
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        let mut first_report = report(&first);
        let secret = first_report.secret.clone();
        let object_key = first_report.object_key.clone();
        first_report.message = "raise 'this is data, not code'".into();
        assert_eq!(store.poll(first_report).unwrap()["text"], "Total 6");
        assert_eq!(store.poll(report(&second)).unwrap()["text"], "Total 6");
        let mut restored = Store::open(path.clone()).unwrap();
        restored.handler = Some(handler);
        assert_eq!(restored.objects[&first].state["total"], 6);
        assert_eq!(restored.objects[&second].state["total"], 6);
        let mut acknowledgment = report(&first);
        acknowledgment.secret = secret.clone();
        acknowledgment.object_key = object_key.clone();
        acknowledgment.message = "raise 'this is data, not code'".into();
        acknowledgment.applied_revision = 1;
        acknowledgment.reported_text = "Total 6".into();
        assert_eq!(restored.poll(acknowledgment).unwrap()["revision"], 1);
        let mut next = report(&first);
        next.secret = secret.clone();
        next.object_key = object_key.clone();
        next.message = "new message".into();
        assert_eq!(restored.poll(next).unwrap()["text"], "Total 12");
        assert_eq!(restored.objects[&second].state["total"], 6);
        let before = serde_json::to_value(&restored.objects).unwrap();
        let saved = std::fs::read(&path).unwrap();
        for source in [
            "lambda { |input| raise 'failure' }",
            "lambda { |input| {'state' => {}, 'display' => {'text' => 'x' * 255, 'color' => [1,1,1], 'alpha' => 1}} }",
            "lambda { |input| {'unexpected' => true} }",
            "lambda { |input| sleep 3; {'state' => {}} }",
        ] {
            restored.handler = Some(source.into());
            let mut next = report(&first);
            next.secret = secret.clone();
            next.object_key = object_key.clone();
            assert!(restored.poll(next).is_err());
            assert_eq!(serde_json::to_value(&restored.objects).unwrap(), before);
            assert_eq!(std::fs::read(&path).unwrap(), saved);
        }
        restored.handler = Some("lambda { |input| {'state' => {'not_saved' => true}} }".into());
        restored.path = Some(directory.clone());
        let mut next = report(&first);
        next.secret = secret;
        next.object_key = object_key;
        assert!(restored.poll(next).is_err());
        assert_eq!(serde_json::to_value(&restored.objects).unwrap(), before);
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert_eq!(Store::open(path).unwrap().objects[&first].state["total"], 12);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn runtime_scheduling_pause_logs_and_reset_survive_restart() {
        let directory = std::env::temp_dir().join(format!("sl-runtime-{}", uuid::Uuid::new_v4()));
        let path = directory.join("objects.json");
        let mut store = Store::open(path.clone()).unwrap();
        let handler = r#"lambda do |prim|
          steps = prim['state'].fetch('steps', 0) + 1
          {'state' => {'steps' => steps, 'event' => prim.fetch('event'),
                       'previous_message' => prim.fetch('previous_message')},
           'logs' => Array.new(16) { "Step #{steps}" }, 'wake_at' => prim.fetch('now') + 3600}
        end"#.to_string();
        store.handler = Some(handler.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let initial = report(&id);
        let secret = initial.secret.clone();
        let object_key = initial.object_key.clone();
        let poll_again = || {
            let mut next = report(&id);
            next.secret = secret.clone();
            next.object_key = object_key.clone();
            next
        };
        store.poll(initial).unwrap();
        assert_eq!(store.objects[&id].state["event"], "register");
        assert_eq!(store.objects[&id].runtime.runs, 1);
        store.poll(poll_again()).unwrap();
        assert_eq!(store.objects[&id].runtime.runs, 1);
        let mut restored = Store::open(path.clone()).unwrap();
        restored.handler = Some(handler.clone());
        let update = |value| serde_json::from_value::<RuntimeUpdate>(value).unwrap();
        restored.update_runtime(&id, update(json!({"expected_version": 1, "paused": true, "poll_seconds": 45}))).unwrap();
        restored.handler = Some("lambda { |prim| raise 'paused handler ran' }".into());
        assert_eq!(restored.poll(poll_again()).unwrap()["poll_seconds"], 45);
        assert_eq!(restored.objects[&id].runtime.runs, 1);
        let stale = restored.update_runtime(&id, update(json!({"expected_version": 1, "paused": false}))).unwrap_err();
        assert_eq!(stale.status(), StatusCode::Conflict);
        restored.handler = Some(handler);
        for step in 2..=6 {
            let version = restored.objects[&id].runtime.version;
            restored.update_runtime(&id, update(json!({"expected_version": version, "paused": false, "wake_now": true}))).unwrap();
            let mut next = poll_again();
            next.message = format!("Message {step}");
            restored.poll(next).unwrap();
        }
        assert_eq!(restored.objects[&id].runtime.runs, 6);
        assert_eq!(restored.objects[&id].runtime.logs.len(), 64);
        assert_eq!(restored.objects[&id].runtime.logs[0], "Step 3");
        assert_eq!(restored.objects[&id].state["event"], "message");
        assert_eq!(restored.objects[&id].state["previous_message"], "Message 5");
        let before = serde_json::to_value(&restored.objects).unwrap();
        let saved = std::fs::read(&path).unwrap();
        let version = restored.objects[&id].runtime.version;
        for invalid in [json!({"expected_version": version, "poll_seconds": 14}),
            json!({"expected_version": version, "state": {}, "reset_state": true})] {
            assert!(restored.update_runtime(&id, update(invalid)).is_err());
        }
        restored.path = Some(directory.clone());
        assert!(restored.update_runtime(&id, update(json!({"expected_version": version, "reset_state": true}))).is_err());
        assert_eq!(serde_json::to_value(&restored.objects).unwrap(), before);
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        restored.path = Some(path.clone());
        restored.update_runtime(&id, update(json!({"expected_version": version, "paused": true,
            "reset_state": true, "clear_logs": true}))).unwrap();
        let restored = Store::open(path).unwrap();
        assert_eq!(restored.objects[&id].state, json!({}));
        assert!(restored.objects[&id].runtime.paused);
        assert!(restored.objects[&id].runtime.logs.is_empty());
        assert_eq!(restored.objects[&id].runtime.wake_at, 0);
        assert_eq!(restored.objects[&id].runtime.runs, 6);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[async_std::test]
    async fn runtime_api_requires_admin_and_rejects_stale_or_invalid_changes() {
        let mut store = Store::new(None);
        let id = uuid::Uuid::new_v4().to_string();
        store.poll(report(&id)).unwrap();
        let mut app = tide::new();
        mount_bridge(&mut app, Bridge { store: Arc::new(Mutex::new(store)),
            admin_token: Some("admin".into()), device_token: Some("device".into()) });
        let url: tide::http::Url = format!("http://localhost/slmessage/objects/{id}/runtime").parse().unwrap();
        for method in [tide::http::Method::Get, tide::http::Method::Post] {
            for token in [None, Some("device")] {
                let mut request = tide::http::Request::new(method, url.clone());
                if let Some(token) = token { request.insert_header("Authorization", format!("Bearer {token}")); }
                let response: Response = app.respond(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::Unauthorized);
            }
        }
        let mut request = tide::http::Request::new(tide::http::Method::Get, url.clone());
        request.insert_header("Authorization", "Bearer admin");
        let mut response: Response = app.respond(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        let body = response.take_body().into_json::<serde_json::Value>().await.unwrap();
        assert_eq!(body["api_version"], "2.0");
        assert_eq!(body["runtime"]["version"], 0);
        assert_eq!(body["handler_enabled"], false);
        assert!(body.get("secret").is_none());
        for (body, expected) in [
            (json!({"expected_version": 0, "paused": true, "state": {"counter": 42}}), StatusCode::Ok),
            (json!({"expected_version": 0, "paused": false}), StatusCode::Conflict),
            (json!({"expected_version": 1, "poll_seconds": 121}), StatusCode::BadRequest),
            (json!({"expected_version": 1, "source": "raise 'no uploads'"}), StatusCode::BadRequest),
            (json!({"paused": false}), StatusCode::BadRequest),
        ] {
            let mut request = tide::http::Request::new(tide::http::Method::Post, url.clone());
            request.insert_header("Authorization", "Bearer admin");
            request.set_body(body.to_string());
            let response: Response = app.respond(request).await.unwrap();
            assert_eq!(response.status(), expected);
        }
        let mut request = tide::http::Request::new(tide::http::Method::Get, url);
        request.insert_header("Authorization", "Bearer admin");
        let mut response: Response = app.respond(request).await.unwrap();
        let body = response.take_body().into_json::<serde_json::Value>().await.unwrap();
        assert_eq!(body["state"]["counter"], 42);
        assert_eq!(body["runtime"]["version"], 1);
        assert_eq!(body["runtime"]["paused"], true);
    }

    #[test]
    fn invalid_runtime_handler_output_is_rolled_back() {
        let mut store = Store::new(None);
        let id = uuid::Uuid::new_v4().to_string();
        let initial = report(&id);
        let secret = initial.secret.clone();
        let object_key = initial.object_key.clone();
        store.poll(initial).unwrap();
        let before = serde_json::to_value(&store.objects).unwrap();
        for source in [
            "lambda { |prim| {'state' => {'changed' => true}, 'logs' => Array.new(17, 'too many')} }",
            "lambda { |prim| {'state' => {}, 'logs' => ['x' * 513]} }",
            "lambda { |prim| {'state' => {}, 'wake_at' => -1} }",
        ] {
            store.handler = Some(source.into());
            let mut next = report(&id);
            next.secret = secret.clone();
            next.object_key = object_key.clone();
            assert!(store.poll(next).is_err());
            assert_eq!(serde_json::to_value(&store.objects).unwrap(), before);
        }
    }

    #[test]
    fn rust_partitioned_registry_migrates_to_ruby_without_identity_changes() {
        let path = std::env::temp_dir().join(format!("sl-rust-migration-{}.json", uuid::Uuid::new_v4()));
        let mut original = Store::new(None);
        let id = uuid::Uuid::new_v4().to_string();
        original.poll(report(&id)).unwrap();
        let mut legacy = partitioned_array_rust::PartitionedArray::new(1, 32, 1, true);
        legacy.allocate(false);
        legacy.add(|row| {
            *row = serde_json::to_value(&original.objects[&id]).unwrap().as_object().unwrap().clone();
            row.remove("state");
        }).unwrap();
        crate::persistent_snapshot::save(&path, &legacy).unwrap();
        let restored = Store::open(path.clone()).unwrap();
        assert_eq!(serde_json::to_value(&restored.objects).unwrap(), serde_json::to_value(&original.objects).unwrap());
        let saved: RubyRows = crate::persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(saved.backend, "ruby-partitioned-array-v1");
        assert_eq!(saved.data_arr.iter().filter(|row| !row.is_empty()).count(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn legacy_registry_migrates_and_corrupt_storage_is_rejected() {
        let path = std::env::temp_dir().join(format!("sl-message-{}.json", uuid::Uuid::new_v4()));
        let mut legacy = Store::new(None);
        let id = uuid::Uuid::new_v4().to_string();
        legacy.poll(report(&id)).unwrap();
        let mut objects = serde_json::to_value(&legacy.objects).unwrap();
        objects[&id].as_object_mut().unwrap().remove("last_seen");
        crate::persistent_snapshot::save(&path, &objects).unwrap();
        let restored = Store::open(path.clone()).unwrap();
        assert_eq!(restored.objects[&id].secret, legacy.objects[&id].secret);
        assert_eq!(restored.objects[&id].last_seen, 0);
        let rows: RubyRows = crate::persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(rows.backend, "ruby-partitioned-array-v1");
        assert_eq!(rows.data_arr.iter().filter(|row| !row.is_empty()).count(), 1);
        assert_eq!(Store::open(path.clone()).unwrap().objects[&id].desired.text, "Original");
        std::fs::write(&path, "broken JSON").unwrap();
        assert!(Store::open(path.clone()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken JSON");
        std::fs::remove_file(path).unwrap();
    }
}