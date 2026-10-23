use crate::persistent_snapshot;
use partitioned_array_rust::PartitionedArray;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io, path::PathBuf, sync::{Arc, Mutex}};
use tide::{Request, StatusCode};

struct Sessions {
    path: PathBuf,
    rows: PartitionedArray,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    List,
    Create { name: String, data: Value },
    Read { id: String },
    Save { id: String, revision: u64, data: Value },
    Rename { id: String, name: String },
    Delete { id: String },
}

fn invalid(message: &str) -> tide::Error {
    tide::Error::from_str(StatusCode::BadRequest, message.to_owned())
}

fn session_name(value: String) -> tide::Result<String> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(invalid("Use a session name between 1 and 80 characters."));
    }
    Ok(name.to_owned())
}

fn validate(data: &Value) -> tide::Result<()> {
    if !data.is_object() || data["version"] != 1 || !data["sound"].is_boolean()
        || ["events", "tasks", "switches", "alarms"].iter().any(|key| {
            data[*key].as_array().is_none_or(|items| items.len() > 10_000)
        }) || !data["notes"].is_object() || !data["timer"].is_object() || !data["watch"].is_object()
    {
        return Err(invalid("Invalid organizer data."));
    }
    Ok(())
}

impl Sessions {
    fn open(path: PathBuf) -> io::Result<Self> {
        let rows = match persistent_snapshot::load::<PartitionedArray>(&path)? {
            Some(rows) => {
                for row in rows.data_arr.iter().filter(|row| !row.is_empty()) {
                    if !row["id"].is_string() || !row["owner"].is_string()
                        || !row["name"].is_string() || !row["revision"].is_u64()
                    {
                        return Err(io::Error::other("Invalid Palm session record"));
                    }
                    validate(&row["data"]).map_err(io::Error::other)?;
                }
                rows
            }
            None => {
                let mut rows = PartitionedArray::new(1, 32, 1, true);
                rows.allocate(false);
                rows
            }
        };
        Ok(Self { path, rows })
    }

    fn apply(&mut self, owner: &str, command: Command) -> tide::Result<Value> {
        if matches!(command, Command::List) {
            return Ok(json!({"sessions": self.rows.data_arr.iter()
                .filter(|row| row.get("owner").and_then(Value::as_str) == Some(owner))
                .map(|row| json!({"id": row["id"], "name": row["name"]}))
                .collect::<Vec<_>>() }));
        }
        let mut next = self.rows.clone();
        let result = match command {
            Command::Create { name, data } => {
                validate(&data)?;
                let row = json!({"owner": owner, "id": uuid::Uuid::new_v4().to_string(),
                    "name": session_name(name)?, "revision": 0, "data": data});
                next.add(|slot| *slot = row.as_object().unwrap().clone())
                    .ok_or_else(|| invalid("Session database is full."))?;
                json!({"id": row["id"], "name": row["name"], "revision": 0, "data": row["data"]})
            }
            command => {
                let id = match &command {
                    Command::Read { id } | Command::Save { id, .. }
                    | Command::Rename { id, .. } | Command::Delete { id } => id,
                    _ => unreachable!(),
                };
                let row = next.data_arr.iter_mut().find(|row| {
                    row.get("owner").and_then(Value::as_str) == Some(owner)
                        && row.get("id").and_then(Value::as_str) == Some(id.as_str())
                }).ok_or_else(|| tide::Error::from_str(StatusCode::NotFound, "Session not found."))?;
                match command {
                    Command::Save { revision, data, .. } => {
                        validate(&data)?;
                        let current = row["revision"].as_u64().unwrap();
                        if revision != current {
                            return Err(tide::Error::from_str(StatusCode::Conflict,
                                "Session changed in another tab. Export your changes, then reload."));
                        }
                        row.insert("data".into(), data);
                        row.insert("revision".into(), json!(current + 1));
                    }
                    Command::Rename { name, .. } => { row.insert("name".into(), json!(session_name(name)?)); }
                    Command::Delete { .. } => { row.clear(); }
                    Command::Read { .. } => return Ok(json!({"id": row["id"], "name": row["name"], "revision": row["revision"], "data": row["data"]})),
                    _ => unreachable!(),
                }
                if row.is_empty() { json!({"deleted": true}) }
                else { json!({"id": row["id"], "name": row["name"], "revision": row["revision"], "data": row["data"]}) }
            }
        };
        persistent_snapshot::save(&self.path, &next)?;
        self.rows = next;
        Ok(result)
    }
}

pub fn mount<State: Clone + Send + Sync + 'static>(app: &mut tide::Server<State>) -> io::Result<()> {
    let path = std::env::var_os("PALMORGANIZER_DATA_PATH").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("palmorganizer_data/sessions.json"));
    let sessions = Arc::new(Mutex::new(Sessions::open(path)?));
    app.at("/palmorganizer/sessions").post(move |mut request: Request<State>| {
        let sessions = sessions.clone();
        async move {
            use async_std::io::ReadExt;
            let existing = request.cookie("palm-owner").map(|cookie| cookie.value().to_owned())
                .filter(|value| uuid::Uuid::parse_str(value).is_ok());
            let owner = existing.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let mut body = Vec::new();
            request.take_body().into_reader().take(5 * 1024 * 1024 + 1).read_to_end(&mut body).await?;
            if body.len() > 5 * 1024 * 1024 {
                return Err(tide::Error::from_str(StatusCode::PayloadTooLarge, "Session exceeds 5 MB."));
            }
            let command = serde_json::from_slice(&body).map_err(|_| invalid("Invalid session command."))?;
            let database_owner = owner.clone();
            let result = async_std::task::spawn_blocking(move || {
                sessions.lock().map_err(|_| tide::Error::from_str(
                    StatusCode::InternalServerError, "Session database unavailable."))?.apply(&database_owner, command)
            }).await?;
            let mut response = tide::Response::new(StatusCode::Ok);
            response.set_body(tide::Body::from_json(&result)?);
            response.insert_header("Cache-Control", "no-store");
            if existing.is_none() {
                let secure = if request.url().scheme() == "https" { "; Secure" } else { "" };
                response.insert_header("Set-Cookie", format!(
                    "palm-owner={owner}; Path=/palmorganizer; HttpOnly; SameSite=Strict; Max-Age=31536000{secure}"));
            }
            Ok(response)
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(text: &str) -> Value {
        json!({"version":1,"sound":true,"events":[],"tasks":[],"switches":[],
            "alarms":[],"notes":{"2026-10-23":text},"timer":{},"watch":{}})
    }

    #[test]
    fn sessions_survive_restart_and_reject_stale_or_cross_owner_writes() {
        let path = std::env::temp_dir().join(format!("palm-{}.json", uuid::Uuid::new_v4()));
        let mut store = Sessions::open(path.clone()).unwrap();
        let first = store.apply("owner", Command::Create { name: "Work".into(), data: data("work") }).unwrap();
        let second = store.apply("owner", Command::Create { name: "Home".into(), data: data("home") }).unwrap();
        let id = first["id"].as_str().unwrap().to_owned();
        store.apply("owner", Command::Save { id: id.clone(), revision: 0, data: data("updated") }).unwrap();
        assert_eq!(store.apply("owner", Command::Save { id: id.clone(), revision: 0, data: data("stale") }).unwrap_err().status(), StatusCode::Conflict);
        assert_eq!(store.apply("other", Command::Read { id: id.clone() }).unwrap_err().status(), StatusCode::NotFound);
        let mut restored = Sessions::open(path.clone()).unwrap();
        assert_eq!(restored.apply("owner", Command::Read { id: id.clone() }).unwrap()["data"], data("updated"));
        assert_eq!(restored.apply("owner", Command::Read { id: second["id"].as_str().unwrap().into() }).unwrap()["data"], data("home"));
        restored.apply("owner", Command::Rename { id: id.clone(), name: "Office".into() }).unwrap();
        restored.apply("owner", Command::Delete { id: id.clone() }).unwrap();
        assert!(Sessions::open(path.clone()).unwrap().apply("owner", Command::Read { id }).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_save_preserves_memory_and_corrupt_disk_is_not_reset() {
        let directory = std::env::temp_dir().join(format!("palm-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let mut store = Sessions::open(directory.join("sessions.json")).unwrap();
        store.path = directory.clone();
        assert!(store.apply("owner", Command::Create { name: "Test".into(), data: data("") }).is_err());
        assert!(store.rows.non_empty_ids().is_empty());
        std::fs::write(directory.join("sessions.json"), "broken").unwrap();
        assert!(Sessions::open(directory.join("sessions.json")).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn partitions_grow_and_invalid_commands_do_not_change_data() {
        let path = std::env::temp_dir().join(format!("palm-{}.json", uuid::Uuid::new_v4()));
        let mut store = Sessions::open(path.clone()).unwrap();
        assert!(store.apply("owner", Command::Create { name: " ".into(), data: data("") }).is_err());
        assert!(store.apply("owner", Command::Create { name: "Bad".into(), data: json!({}) }).is_err());
        for index in 0..70 {
            store.apply("owner", Command::Create { name: format!("Session {index}"), data: data("") }).unwrap();
        }
        assert!(store.rows.db_size > 1);
        let mut restored = Sessions::open(path.clone()).unwrap();
        assert_eq!(restored.apply("owner", Command::List).unwrap()["sessions"].as_array().unwrap().len(), 70);
        assert!(restored.apply("other", Command::List).unwrap()["sessions"].as_array().unwrap().is_empty());
        std::fs::remove_file(path).unwrap();
    }
}