use async_std::io::ReadExt;
use partitioned_array_rust::PartitionedArray;
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

struct Store {
    path: Option<PathBuf>,
    rows: PartitionedArray,
    objects: BTreeMap<String, Object>,
}

impl Store {
    fn new(path: Option<PathBuf>) -> Self {
        let mut rows = PartitionedArray::new(1, 32, 1, true);
        rows.allocate(false);
        Self { path, rows, objects: BTreeMap::new() }
    }

    fn open(path: PathBuf) -> tide::Result<Self> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Snapshot {
            Partitioned(PartitionedArray),
            Legacy(BTreeMap<String, Object>),
        }

        let snapshot = crate::persistent_snapshot::load::<Snapshot>(&path)?;
        let mut store = Self::new(Some(path));
        match snapshot {
            Some(Snapshot::Partitioned(rows)) => {
                for row in rows.data_arr.iter().filter(|row| !row.is_empty()) {
                    let object: Object = serde_json::from_value(serde_json::Value::Object(row.clone()))?;
                    let id = object.id.clone();
                    if store.objects.insert(id, object).is_some() {
                        return Err(error(StatusCode::InternalServerError, "Duplicate prim ID in message storage"));
                    }
                }
                store.rows = rows;
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
        let mut rows = self.rows.clone();
        for object in objects.values() {
            let value = serde_json::to_value(object)?;
            let record = value.as_object().unwrap();
            if let Some(row) = rows.data_arr.iter_mut().find(|row| {
                row.get("id").and_then(serde_json::Value::as_str) == Some(object.id.as_str())
            }) {
                *row = record.clone();
            } else {
                rows.add(|row| *row = record.clone())
                    .ok_or_else(|| error(StatusCode::ServiceUnavailable, "Object registry is full"))?;
            }
        }
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
        if !objects.contains_key(&poll.id) {
            if objects.len() >= 500 {
                return Err(error(StatusCode::ServiceUnavailable, "Object registry is full"));
            }
            if poll.applied_revision != -1 {
                return Err(error(StatusCode::Conflict, "Register again with revision -1"));
            }
            objects.insert(poll.id.clone(), Object {
                id: poll.id.clone(), secret: poll.secret.clone(), object_key: poll.object_key.clone(),
                name: String::new(), region: String::new(), desired: display,
                revision: 0, applied_revision: -1, reported_text: String::new(),
                message: String::new(), message_at: 0, last_seen: 0,
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
        object.name = poll.name;
        object.region = poll.region;
        object.applied_revision = poll.applied_revision;
        object.reported_text = poll.reported_text;
        if object.message != poll.message {
            object.message_at = now;
            object.message = poll.message;
        }
        object.last_seen = now;
        let response = serde_json::json!({"revision": object.revision, "text": object.desired.text,
            "color": object.desired.color, "alpha": object.desired.alpha, "poll_seconds": 15});
        self.commit(objects)?;
        Ok(response)
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
            "last_seen": object.last_seen, "online": object.last_seen > 0 && now - object.last_seen < 60,
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
    });
    app.at("/poll").post(|mut req: Request<Bridge>| async move {
        authorize(&req, &req.state().device_token)?;
        let poll: Poll = read_json(&mut req).await?;
        let result = req.state().store.lock().map_err(|_| error(StatusCode::InternalServerError, "Store unavailable"))?.poll(poll)?;
        Ok(json(result))
    });
    app
}

pub fn mount<State: Clone + Send + Sync + 'static>(app: &mut tide::Server<State>) -> tide::Result<()> {
    let path = PathBuf::from(std::env::var("SL_MESSAGE_STORE").unwrap_or_else(|_| "sl_message_data/objects.json".into()));
    let bridge = Bridge {
        store: Arc::new(Mutex::new(Store::open(path)?)),
        admin_token: std::env::var("SL_MESSAGE_ADMIN_TOKEN").ok(),
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
            admin_token: Some("admin".into()), device_token: Some("device".into()) };
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
        request.insert_header("Authorization", "Bearer admin");
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
    fn partitioned_records_survive_restart_and_failed_saves() {
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
        for _ in 0..40 {
            store.poll(report(&uuid::Uuid::new_v4().to_string())).unwrap();
        }
        let rows: PartitionedArray = crate::persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(rows.non_empty_ids().len(), 41);
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
        let rows: PartitionedArray = crate::persistent_snapshot::load(&path).unwrap().unwrap();
        assert_eq!(rows.non_empty_ids().len(), 1);
        assert_eq!(Store::open(path.clone()).unwrap().objects[&id].desired.text, "Original");
        std::fs::write(&path, "broken JSON").unwrap();
        assert!(Store::open(path.clone()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "broken JSON");
        std::fs::remove_file(path).unwrap();
    }
}