//! Single-path organizer: HTML on GET, JSON reads/writes on POST.
use chrono::{DateTime, Datelike, Days, Duration, Months, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::America::Los_Angeles;
use partitioned_array_rust::PartitionedArray;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io, path::PathBuf, sync::{Arc, Mutex}};
use tide::{Request, Response, StatusCode};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Record {
    Tag { id: String, name: String, #[serde(default)] deleted: bool },
    Event { id: String, tag_id: String, note: String, at: DateTime<Utc>, #[serde(default)] deleted: bool },
}

struct Store {
    root: PathBuf,
    rows: PartitionedArray,
}

impl Store {
    fn open(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        let mut rows = PartitionedArray::new(1, 128, 1, true);
        match fs::read_to_string(root.join("CURRENT")) {
            Ok(generation) => {
                // Only UUID directory names are accepted from the manifest.
                uuid::Uuid::parse_str(&generation).map_err(io::Error::other)?;
                rows.load_from_dir(root.join(generation), "records")?;
                for row in rows.data_arr.iter().filter(|r| !r.is_empty()) {
                    serde_json::from_value::<Record>(Value::Object(row.clone()))
                        .map_err(io::Error::other)?;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => rows.allocate(false),
            Err(e) => return Err(e),
        }
        Ok(Self { root, rows })
    }

    fn records(&self) -> Vec<Record> {
        self.rows.data_arr.iter().filter(|r| !r.is_empty())
            .map(|r| serde_json::from_value(Value::Object(r.clone())).expect("validated record"))
            .collect()
    }

    fn insert(&mut self, record: Record) -> io::Result<()> {
        self.write_record(record, false)
    }

    fn write_record(&mut self, record: Record, replace: bool) -> io::Result<()> {
        let mut next = self.rows.clone();
        let Value::Object(row) = serde_json::to_value(record).map_err(io::Error::other)? else {
            return Err(io::Error::other("invalid record"));
        };
        if replace {
            let slot = next.data_arr.iter_mut().find(|slot| slot.get("id") == row.get("id") && slot.get("kind") == row.get("kind"))
                .ok_or_else(|| io::Error::other("record not found"))?;
            *slot = row;
        } else {
            next.add(|slot| *slot = row.clone()).ok_or_else(|| io::Error::other("store full"))?;
        }
        // The crate writes several files. Publish a complete generation with one
        // atomic manifest rename, so an interrupted save cannot mix partitions.
        let generation = uuid::Uuid::new_v4().to_string();
        let dir = self.root.join(&generation);
        let old = fs::read_to_string(self.root.join("CURRENT")).ok();
        let saved = (|| -> io::Result<()> {
            next.save_to_dir(&dir, "records")?;
            for entry in fs::read_dir(dir.join("records"))? {
                fs::File::open(entry?.path())?.sync_all()?;
            }
            fs::File::open(dir.join("records"))?.sync_all()?;
            fs::File::open(&dir)?.sync_all()?;
            fs::write(self.root.join("CURRENT.tmp"), &generation)?;
            fs::File::open(self.root.join("CURRENT.tmp"))?.sync_all()?;
            fs::rename(self.root.join("CURRENT.tmp"), self.root.join("CURRENT"))?;
            Ok(())
        })();
        if let Err(error) = saved {
            let _ = fs::remove_dir_all(dir);
            return Err(error);
        }
        self.rows = next;
        // A failure after publication must not tell the caller to retry a click.
        if fs::File::open(&self.root).and_then(|f| f.sync_all()).is_ok() {
            if let Some(old) = old {
                if uuid::Uuid::parse_str(&old).is_ok() {
                    let _ = fs::remove_dir_all(self.root.join(old));
                }
            }
        }
        Ok(())
    }

    fn apply(&mut self, command: Command) -> Result<(), &'static str> {
        let records = self.records();
        let mut replace = false;
        let record = match command {
            Command::Read => return Ok(()),
            Command::AddTag { name } => {
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 80 {
                    return Err("Use a tag name between 1 and 80 characters.");
                }
                if records.iter().any(|r| matches!(r, Record::Tag { name: n, deleted: false, .. } if n.to_lowercase() == name.to_lowercase())) {
                    return Err("That tag already exists.");
                }
                Record::Tag { id: uuid::Uuid::new_v4().to_string(), name: name.into(), deleted: false }
            }
            Command::RenameTag { tag_id, name } => {
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 80 {
                    return Err("Use a tag name between 1 and 80 characters.");
                }
                if !records.iter().any(|r| matches!(r, Record::Tag { id, deleted: false, .. } if id == &tag_id)) {
                    return Err("Tag not found. Refresh and try again.");
                }
                if records.iter().any(|r| matches!(r, Record::Tag { id, name: n, deleted: false } if id != &tag_id && n.to_lowercase() == name.to_lowercase())) {
                    return Err("That tag already exists.");
                }
                replace = true;
                Record::Tag { id: tag_id, name: name.into(), deleted: false }
            }
            Command::DeleteTag { tag_id } => {
                let Some(Record::Tag { id, name, .. }) = records.iter().find(|r| matches!(r, Record::Tag { id, deleted: false, .. } if id == &tag_id)) else {
                    return Err("Tag not found. Refresh and try again.");
                };
                replace = true;
                Record::Tag { id: id.clone(), name: name.clone(), deleted: true }
            }
            Command::EditEntry { entry_id, note } => {
                let note = note.trim();
                if note.is_empty() || note.chars().count() > 2000 {
                    return Err("Use an entry between 1 and 2,000 characters.");
                }
                let mut entry = Self::active_entry(&records, &entry_id)?;
                if let Record::Event { note: text, .. } = &mut entry { *text = note.into(); }
                replace = true;
                entry
            }
            Command::DeleteEntry { entry_id } => {
                let mut entry = Self::active_entry(&records, &entry_id)?;
                if let Record::Event { deleted, .. } = &mut entry { *deleted = true; }
                replace = true;
                entry
            }
            Command::Click { tag_id } => self.event(&records, tag_id, String::new())?,
            Command::AddEntry { tag_id, note } => {
                let note = note.trim();
                if note.is_empty() || note.chars().count() > 2000 {
                    return Err("Use an entry between 1 and 2,000 characters.");
                }
                self.event(&records, tag_id, note.into())?
            }
        };
        self.write_record(record, replace).map_err(|error| {
            eprintln!("Organizer save failed: {error}");
            "Could not save. Your change was not applied."
        })
    }

    fn active_entry(records: &[Record], entry_id: &str) -> Result<Record, &'static str> {
        let entry = records.iter().find(|r| matches!(r, Record::Event { id, note, deleted: false, .. } if id == entry_id && !note.is_empty()));
        if let Some(entry @ Record::Event { tag_id, .. }) = entry {
            if records.iter().any(|r| matches!(r, Record::Tag { id, deleted: false, .. } if id == tag_id)) { return Ok(entry.clone()); }
        }
        Err("Journal entry not found. Refresh and try again.")
    }

    fn event(&self, records: &[Record], tag_id: String, note: String) -> Result<Record, &'static str> {
        if !records.iter().any(|r| matches!(r, Record::Tag { id, deleted: false, .. } if *id == tag_id)) {
            return Err("Tag not found. Refresh and try again.");
        }
        Ok(Record::Event { id: uuid::Uuid::new_v4().to_string(), tag_id, note, at: Utc::now(), deleted: false })
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Read,
    AddTag { name: String },
    RenameTag { tag_id: String, name: String },
    DeleteTag { tag_id: String },
    EditEntry { entry_id: String, note: String },
    DeleteEntry { entry_id: String },
    Click { tag_id: String },
    AddEntry { tag_id: String, note: String },
}

#[derive(Default, Deserialize)]
struct Query {
    #[serde(default)]
    unit: Unit,
    tag: Option<String>,
    journal_tag: Option<String>,
    #[serde(default)]
    journal_page: usize,
    #[serde(default)]
    include_deleted: bool,
    #[serde(default)]
    archive_unit: Unit,
    #[serde(default)]
    archive_page: usize,
    archive_from: Option<NaiveDate>,
    archive_to: Option<NaiveDate>,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Unit { Second, Minute, Hour, #[default] Day, Week, Month, Year }

fn pacific_midnight(day: NaiveDate) -> DateTime<Utc> {
    Los_Angeles.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
        .single().expect("Pacific calendar midnight is unambiguous").with_timezone(&Utc)
}

fn floor(at: DateTime<Utc>, unit: Unit) -> DateTime<Utc> {
    let local = at.with_timezone(&Los_Angeles);
    let day = local.date_naive();
    // Subtract elapsed time for sub-day buckets to preserve which occurrence
    // of the repeated fall-back hour this instant belongs to.
    let second = at - Duration::nanoseconds(at.nanosecond().into());
    match unit {
        Unit::Second => second,
        Unit::Minute => second - Duration::seconds(local.second().into()),
        Unit::Hour => second - Duration::seconds((local.minute() * 60 + local.second()).into()),
        Unit::Day => pacific_midnight(day),
        Unit::Week => pacific_midnight(day - Days::new(local.weekday().num_days_from_monday().into())),
        Unit::Month => pacific_midnight(day.with_day(1).unwrap()),
        Unit::Year => pacific_midnight(NaiveDate::from_ymd_opt(local.year(), 1, 1).unwrap()),
    }
}

fn previous(at: DateTime<Utc>, unit: Unit) -> DateTime<Utc> {
    let day = at.with_timezone(&Los_Angeles).date_naive();
    match unit {
        Unit::Second => at - Duration::seconds(1),
        Unit::Minute => at - Duration::minutes(1),
        Unit::Hour => at - Duration::hours(1),
        Unit::Day => pacific_midnight(day - Days::new(1)),
        Unit::Week => pacific_midnight(day - Days::new(7)),
        Unit::Month => pacific_midnight(day.checked_sub_months(Months::new(1)).unwrap()),
        Unit::Year => pacific_midnight(day.checked_sub_months(Months::new(12)).unwrap()),
    }
}

fn archive(records: &[Record], query: &Query, now: DateTime<Utc>) -> Value {
    let tags: Vec<_> = records.iter().filter_map(|r| match r {
        Record::Tag { id, name, deleted } => Some(json!({ "id": id, "name": name, "deleted": deleted })),
        _ => None,
    }).collect();
    let indices: std::collections::HashMap<_, _> = tags.iter().enumerate()
        .map(|(i, tag)| (tag["id"].as_str().unwrap(), i)).collect();
    let mut buckets = std::collections::BTreeMap::new();
    let mut totals = vec![0u64; tags.len()];
    for record in records {
        if let Record::Event { tag_id, at, .. } = record {
            let day = at.with_timezone(&Los_Angeles).date_naive();
            if *at > now || query.archive_from.is_some_and(|from| day < from)
                || query.archive_to.is_some_and(|to| day > to) { continue; }
            if let Some(&i) = indices.get(tag_id.as_str()) {
                let counts = buckets.entry(floor(*at, query.archive_unit))
                    .or_insert_with(|| vec![0u64; tags.len()]);
                counts[i] += 1;
                totals[i] += 1;
            }
        }
    }
    // Paginate occupied periods rather than walking every elapsed second.
    // This keeps the entire archive reachable even across years of inactivity.
    let periods = buckets.len();
    let pages = periods.div_ceil(30).max(1);
    let page = query.archive_page.min(pages - 1);
    let rows: Vec<_> = buckets.into_iter().rev().skip(page * 30).take(30)
        .map(|(at, counts)| json!({ "at": at, "total": counts.iter().sum::<u64>(), "counts": counts })).collect();
    json!({ "tags": tags, "rows": rows, "totals": totals, "total": totals.iter().sum::<u64>(),
        "periods": periods, "page": page, "pages": pages })
}

fn snapshot(records: &[Record], query: &Query, now: DateTime<Utc>) -> Value {
    let units = [Unit::Year, Unit::Month, Unit::Week, Unit::Day, Unit::Hour, Unit::Minute, Unit::Second];
    let starts = units.map(|unit| floor(now, unit));
    let mut tags_all: Vec<(String, String, bool)> = Vec::new();
    let mut tag_index: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for record in records {
        let Record::Tag { id, name, deleted } = record else { continue };
        if tag_index.contains_key(id.as_str()) {
            continue;
        }
        tag_index.insert(id.as_str(), tags_all.len());
        tags_all.push((id.clone(), name.clone(), *deleted));
    }

    let mut tag_counts = vec![vec![0u64; 8]; tags_all.len()];
    let mut tag_entry_counts = vec![vec![0u64; 8]; tags_all.len()];
    let mut tag_entry_total = vec![0u64; tags_all.len()];

    for record in records {
        let Record::Event { tag_id, at, note, deleted: entry_deleted, .. } = record else { continue };
        if *at > now {
            continue;
        }
        let Some(&idx) = tag_index.get(tag_id.as_str()) else { continue };

        tag_counts[idx][0] += 1;
        for (i, start) in starts.iter().enumerate() {
            if at >= start {
                tag_counts[idx][i + 1] += 1;
            }
        }

        if !note.is_empty() {
            if !entry_deleted {
                tag_entry_total[idx] += 1;
            }
            tag_entry_counts[idx][0] += 1;
            for (i, start) in starts.iter().enumerate() {
                if at >= start {
                    tag_entry_counts[idx][i + 1] += 1;
                }
            }
        }
    }

    let mut totals = vec![0u64; 8];
    for counts in &tag_counts {
        for (total, count) in totals.iter_mut().zip(counts) {
            *total += *count;
        }
    }

    let tags: Vec<Value> = tags_all.iter().enumerate().filter_map(|(idx, (id, name, deleted))| {
        if *deleted {
            return None;
        }
        Some(json!({
            "id": id,
            "name": name,
            "counts": tag_counts[idx],
            "entry_counts": tag_entry_counts[idx],
            "entry_count": tag_entry_total[idx],
        }))
    }).collect();
    let events: Vec<&Record> = records.iter().filter(|r| matches!(r,
        Record::Event { tag_id, at, .. } if *at <= now && query.tag.as_ref().is_none_or(|id| id == tag_id)
    )).collect();
    let mut buckets = std::collections::BTreeMap::new();
    let mut start = floor(now, query.unit);
    for _ in 0..30 {
        buckets.insert(start, 0u64);
        start = previous(start, query.unit);
    }
    for event in &events {
        if let Record::Event { at, .. } = event {
            if let Some(count) = buckets.get_mut(&floor(*at, query.unit)) { *count += 1; }
        }
    }
    let history: Vec<Value> = buckets.into_iter().map(|(at, count)| json!({"at": at, "count": count})).collect();
    let recent: Vec<_> = events.iter().rev().filter(|r| matches!(r, Record::Event { tag_id, deleted: false, .. } if tags.iter().any(|t| t["id"].as_str() == Some(tag_id.as_str())))).take(100).collect();
    // Journal pagination is independent of the activity filter and includes
    // every saved note, even when it is older than the recent-activity limit.
    let journal_tag = query.journal_tag.as_deref().filter(|target| records.iter().any(|r| matches!(r,
        Record::Tag { id, deleted, .. } if id == target && (!deleted || query.include_deleted))))
        .or_else(|| tags.first().and_then(|tag| tag["id"].as_str()))
        .or_else(|| if query.include_deleted {
            records.iter().find_map(|r| match r { Record::Tag { id, .. } => Some(id.as_str()), _ => None })
        } else { None });
    let journal_deleted = records.iter().any(|r| matches!(r, Record::Tag { id, deleted: true, .. } if Some(id.as_str()) == journal_tag));
    let entries: Vec<&Record> = records.iter().rev().filter(|r| matches!(r,
        Record::Event { tag_id, note, at, deleted, .. }
            if Some(tag_id.as_str()) == journal_tag && !note.is_empty() && *at <= now && (!deleted || query.include_deleted)
    )).collect();
    let page_size = 10;
    let pages = entries.len().div_ceil(page_size).max(1);
    let page = query.journal_page.min(pages - 1);
    let journal = json!({ "tag_id": journal_tag, "deleted": journal_deleted, "total": entries.len(),
        "page": page, "pages": pages,
        "entries": entries.iter().skip(page * page_size).take(page_size).collect::<Vec<_>>() });
    json!({ "tags": tags, "totals": totals, "history": history, "recent": recent,
        "matching_events": events.len(), "as_of": now, "journal": journal,
        "archive": archive(records, query, now) })
}

fn json_reply(status: StatusCode, value: Value) -> tide::Result {
    let mut response = Response::new(status);
    response.set_body(tide::Body::from_json(&value)?);
    response.insert_header("Cache-Control", "no-store");
    Ok(response)
}

pub fn mount<State: Clone + Send + Sync + 'static>(app: &mut tide::Server<State>) -> tide::Result<()> {
    let root = std::env::var_os("ORGANIZER_DATA_DIR").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("organizer_data"));
    let store = Arc::new(Mutex::new(Store::open(root)?));
    mount_store(app, store)
}

fn mount_store<State: Clone + Send + Sync + 'static>(
    app: &mut tide::Server<State>, store: Arc<Mutex<Store>>,
) -> tide::Result<()> {
    app.at("/organizer").get(|_| async {
        let mut response = Response::new(StatusCode::Ok);
        response.set_content_type(tide::http::mime::HTML);
        response.set_body(include_str!("organizer.html"));
        Ok(response)
    }).post(move |mut req: Request<State>| {
        let store = Arc::clone(&store);
        async move {
            if req.content_type().is_none_or(|mime| mime.essence() != "application/json") {
                return json_reply(StatusCode::UnsupportedMediaType, json!({"error": "Send application/json."}));
            }
            let query = match req.query::<Query>() {
                Ok(query) => query,
                Err(_) => return json_reply(StatusCode::BadRequest, json!({"error": "Invalid time unit, date, or page."})),
            };
            if matches!((query.archive_from, query.archive_to), (Some(from), Some(to)) if from > to) {
                return json_reply(StatusCode::BadRequest, json!({"error": "Archive start date must be on or before the end date."}));
            }
            // Bound the actual body, including requests without Content-Length.
            use async_std::io::ReadExt;
            let mut bytes = Vec::new();
            req.take_body().into_reader().take(16385).read_to_end(&mut bytes).await?;
            if bytes.len() > 16384 {
                return json_reply(StatusCode::PayloadTooLarge, json!({"error": "Request is too large."}));
            }
            let command = match serde_json::from_slice::<Command>(&bytes) {
                Ok(command) => command,
                Err(_) => return json_reply(StatusCode::BadRequest, json!({"error": "Invalid organizer action."})),
            };
            async_std::task::spawn_blocking(move || {
                let mut store = store.lock().map_err(|_| tide::Error::from_str(StatusCode::InternalServerError, "Organizer unavailable"))?;
                if let Err(message) = store.apply(command) {
                    let status = if message.starts_with("Could not save") { StatusCode::InternalServerError } else { StatusCode::BadRequest };
                    return json_reply(status, json!({"error": message}));
                }
                json_reply(StatusCode::Ok, snapshot(&store.records(), &query, Utc::now()))
            }).await
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("organizer-test-{}", uuid::Uuid::new_v4())))
        }
        fn store(&self) -> Store { Store::open(self.0.clone()).unwrap() }
    }
    impl Drop for TestDir {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    fn date(raw: &str) -> DateTime<Utc> { raw.parse().unwrap() }
    fn tag_id(store: &Store) -> String {
        match &store.records()[0] { Record::Tag { id, .. } => id.clone(), _ => panic!("expected tag") }
    }
    fn event(tag: &str, at: &str) -> Record {
        Record::Event { deleted: false, id: uuid::Uuid::new_v4().to_string(), tag_id: tag.into(), note: String::new(), at: date(at) }
    }

    #[test]
    fn clicks_and_entries_survive_restart_and_count_once() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: " Walk ".into() }).unwrap();
        let id = tag_id(&store);
        store.apply(Command::Click { tag_id: id.clone() }).unwrap();
        store.apply(Command::AddEntry { tag_id: id, note: "By the river".into() }).unwrap();
        let reopened = dir.store();
        let state = snapshot(&reopened.records(), &Query::default(), Utc::now());
        assert_eq!(state["totals"][0], 2);
        assert_eq!(state["tags"][0]["name"], "Walk");
        assert_eq!(state["recent"][0]["note"], "By the river");
        assert_eq!(state["recent"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn renaming_tags_and_editing_notes_preserves_identity_time_and_counts() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "Original".into() }).unwrap();
        let id = tag_id(&store);
        store.apply(Command::AddEntry { tag_id: id.clone(), note: "First draft".into() }).unwrap();
        let before = snapshot(&store.records(), &Query::default(), Utc::now());
        let entry_id = before["journal"]["entries"][0]["id"].as_str().unwrap().to_string();
        store.apply(Command::RenameTag { tag_id: id.clone(), name: " Renamed ".into() }).unwrap();
        store.apply(Command::EditEntry { entry_id: entry_id.clone(), note: "Revised note".into() }).unwrap();
        let after = snapshot(&dir.store().records(), &Query::default(), Utc::now());
        assert_eq!(after["tags"][0]["id"], id);
        assert_eq!(after["tags"][0]["name"], "Renamed");
        assert_eq!(after["archive"]["tags"][0]["name"], "Renamed");
        assert_eq!(after["journal"]["entries"][0]["id"], entry_id);
        assert_eq!(after["journal"]["entries"][0]["at"], before["journal"]["entries"][0]["at"]);
        assert_eq!(after["journal"]["entries"][0]["note"], "Revised note");
        assert_eq!(after["totals"][0], 1);
    }

    #[test]
    fn deleted_tags_and_notes_leave_active_views_but_remain_archived() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "Journal".into() }).unwrap();
        let id = tag_id(&store);
        store.apply(Command::AddEntry { tag_id: id.clone(), note: "Keep in archive".into() }).unwrap();
        let before = snapshot(&store.records(), &Query::default(), Utc::now());
        let entry_id = before["journal"]["entries"][0]["id"].as_str().unwrap().to_string();
        store.apply(Command::DeleteEntry { entry_id: entry_id.clone() }).unwrap();
        let deleted_note = snapshot(&dir.store().records(), &Query::default(), Utc::now());
        assert_eq!(deleted_note["journal"]["total"], 0);
        assert_eq!(deleted_note["tags"][0]["entry_count"], 0);
        assert_eq!(deleted_note["recent"], json!([]));
        assert_eq!(deleted_note["archive"]["total"], 1);
        assert!(store.apply(Command::EditEntry { entry_id, note: "Cannot edit archived entry".into() }).is_err());
        store.apply(Command::DeleteTag { tag_id: id.clone() }).unwrap();
        let normal = snapshot(&dir.store().records(), &Query::default(), Utc::now());
        assert_eq!(normal["tags"], json!([]));
        assert_eq!(normal["totals"][0], 1);
        assert_eq!(normal["archive"]["tags"][0]["deleted"], true);
        let query = Query { journal_tag: Some(id.clone()), include_deleted: true, ..Query::default() };
        let archived = snapshot(&dir.store().records(), &query, Utc::now());
        assert_eq!(archived["journal"]["deleted"], true);
        assert_eq!(archived["journal"]["entries"][0]["note"], "Keep in archive");
        assert_eq!(archived["journal"]["entries"][0]["deleted"], true);
        assert!(store.apply(Command::Click { tag_id: id.clone() }).is_err());
        assert!(store.apply(Command::AddEntry { tag_id: id.clone(), note: "No".into() }).is_err());
        assert!(store.apply(Command::RenameTag { tag_id: id, name: "No".into() }).is_err());
        store.apply(Command::AddTag { name: "Journal".into() }).unwrap(); // Reuse a deleted name, with a new ID.
    }

    #[test]
    fn edits_validate_names_and_rollback_on_failed_persistence() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "One".into() }).unwrap();
        let id = tag_id(&store);
        store.apply(Command::AddTag { name: "Two".into() }).unwrap();
        for name in [" ".into(), "two".into(), "x".repeat(81)] {
            assert!(store.apply(Command::RenameTag { tag_id: id.clone(), name }).is_err());
        }
        assert!(store.apply(Command::DeleteEntry { entry_id: id.clone() }).is_err());
        assert!(store.apply(Command::DeleteTag { tag_id: "missing".into() }).is_err());
        store.apply(Command::AddEntry { tag_id: id.clone(), note: "Original note".into() }).unwrap();
        let entry_id = snapshot(&store.records(), &Query::default(), Utc::now())["journal"]["entries"][0]["id"].as_str().unwrap().to_string();
        for note in [" ".into(), "x".repeat(2001)] {
            assert!(store.apply(Command::EditEntry { entry_id: entry_id.clone(), note }).is_err());
        }
        fs::create_dir(dir.0.join("CURRENT.tmp")).unwrap();
        assert!(store.apply(Command::RenameTag { tag_id: id.clone(), name: "Unsaved".into() }).is_err());
        assert!(store.apply(Command::DeleteEntry { entry_id }).is_err());
        assert!(store.apply(Command::DeleteTag { tag_id: id }).is_err());
        let after = snapshot(&dir.store().records(), &Query::default(), Utc::now());
        assert_eq!(after["tags"][0]["name"], "One");
        assert_eq!(after["journal"]["entries"][0]["note"], "Original note");
        assert_eq!(store.records().len(), 3);
    }

    #[test]
    fn legacy_records_default_to_not_deleted() {
        let tag: Record = serde_json::from_value(json!({"kind":"tag", "id":"a", "name":"Legacy"})).unwrap();
        let entry: Record = serde_json::from_value(json!({"kind":"event", "id":"b", "tag_id":"a", "note":"Legacy note", "at":"2026-10-04T12:00:00Z"})).unwrap();
        assert!(matches!(tag, Record::Tag { deleted: false, .. }));
        assert!(matches!(entry, Record::Event { deleted: false, .. }));
    }

    #[test]
    fn archive_keeps_every_tag_and_old_period_with_range_totals() {
        let mut records = vec![
            Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
            Record::Tag { deleted: false, id: "b".into(), name: "B".into() },
            Record::Tag { deleted: false, id: "empty".into(), name: "Empty".into() },
        ];
        for i in 0..65 {
            records.push(Record::Event { deleted: false, id: i.to_string(), tag_id: "a".into(),
                note: if i % 2 == 0 { "Journal entry".into() } else { String::new() },
                at: date("2025-01-01T12:00:00Z") + Duration::days(i) });
        }
        records.push(event("b", "2025-01-01T12:00:00Z"));
        let now = date("2026-10-04T12:00:00Z");
        let mut query = Query { tag: Some("empty".into()), ..Query::default() };
        let mut rows = Vec::new();
        for page in 0..3 {
            query.archive_page = page;
            let state = archive(&records, &query, now);
            assert_eq!(state["totals"], json!([65, 1, 0]));
            assert_eq!(state["total"], 66);
            assert_eq!(state["periods"], 65);
            assert_eq!(state["pages"], 3);
            rows.extend(state["rows"].as_array().unwrap().iter().cloned());
        }
        assert_eq!(rows.len(), 65);
        assert_eq!(rows.last().unwrap()["counts"], json!([1, 1, 0]));
        assert_eq!(rows.iter().map(|row| row["total"].as_u64().unwrap()).sum::<u64>(), 66);
        query.archive_page = usize::MAX;
        assert_eq!(archive(&records, &query, now)["page"], 2);
    }

    #[test]
    fn archive_date_range_is_inclusive_in_pacific_and_keeps_repeated_hours() {
        let records = vec![
            Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
            event("a", "2026-11-01T06:59:59Z"), // Previous Pacific day.
            event("a", "2026-11-01T08:30:00Z"), // 01:30 PDT.
            event("a", "2026-11-01T09:30:00Z"), // 01:30 PST.
            event("a", "2026-11-02T07:59:59Z"), // Still November 1 Pacific.
            event("a", "2026-11-02T08:00:00Z"), // Next Pacific day.
        ];
        let query = Query { archive_unit: Unit::Hour,
            archive_from: Some(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()),
            archive_to: Some(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()), ..Query::default() };
        let state = archive(&records, &query, date("2026-11-03T12:00:00Z"));
        assert_eq!(state["total"], 3);
        assert_eq!(state["periods"], 3);
        assert_eq!(state["rows"][1]["at"], "2026-11-01T09:00:00Z");
        assert_eq!(state["rows"][2]["at"], "2026-11-01T08:00:00Z");
        let empty = archive(&[], &Query::default(), Utc::now());
        assert_eq!(empty["rows"], json!([]));
        assert_eq!(empty["total"], 0);
        assert_eq!(empty["pages"], 1);
    }

    #[test]
    fn multiple_journal_entries_remain_separate_after_restart() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "Journal".into() }).unwrap();
        let id = tag_id(&store);
        for note in ["First reflection", "Second reflection", "Third reflection"] {
            store.apply(Command::AddEntry { tag_id: id.clone(), note: note.into() }).unwrap();
        }
        store.apply(Command::Click { tag_id: id.clone() }).unwrap();
        let reopened = dir.store();
        let state = snapshot(&reopened.records(), &Query::default(), Utc::now());
        assert_eq!(state["totals"][0], 4);
        assert_eq!(state["archive"]["total"], 4);
        assert_eq!(state["archive"]["totals"], json!([4]));
        assert_eq!(state["tags"][0]["entry_count"], 3);
        assert_eq!(state["journal"]["tag_id"], id);
        assert_eq!(state["journal"]["total"], 3);
        let entries = state["journal"]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["note"], "Third reflection");
        assert_eq!(entries[2]["note"], "First reflection");
        assert_ne!(entries[0]["id"], entries[1]["id"]);
    }

    #[test]
    fn journal_pages_reach_old_notes_and_stay_scoped_to_the_tag() {
        let mut records = vec![
            Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
            Record::Tag { deleted: false, id: "b".into(), name: "B".into() },
        ];
        for i in 0..25 {
            records.push(Record::Event { deleted: false, id: i.to_string(), tag_id: "a".into(),
                note: format!("Entry {i}"), at: date("2026-10-01T12:00:00Z") });
        }
        records.push(Record::Event { deleted: false, id: "other".into(), tag_id: "b".into(),
            note: "Other journal".into(), at: date("2026-10-01T12:00:00Z") });
        for _ in 0..120 { records.push(event("a", "2026-10-02T12:00:00Z")); }
        let mut query = Query { journal_tag: Some("a".into()), tag: Some("b".into()), ..Query::default() };
        let now = date("2026-10-04T12:00:00Z");
        let mut notes = Vec::new();
        for page in 0..3 {
            query.journal_page = page;
            let state = snapshot(&records, &query, now);
            assert_eq!(state["journal"]["total"], 25);
            assert_eq!(state["journal"]["pages"], 3);
            assert_eq!(state["recent"][0]["note"], "Other journal");
            notes.extend(state["journal"]["entries"].as_array().unwrap().iter().map(|e| e["note"].as_str().unwrap().to_string()));
        }
        assert_eq!(notes, (0..25).rev().map(|i| format!("Entry {i}")).collect::<Vec<_>>());
        query.journal_page = usize::MAX;
        assert_eq!(snapshot(&records, &query, now)["journal"]["page"], 2);
        query.journal_tag = Some("b".into());
        let other = snapshot(&records, &query, now);
        assert_eq!(other["journal"]["page"], 0);
        assert_eq!(other["journal"]["total"], 1);
        assert_eq!(other["journal"]["entries"][0]["note"], "Other journal");
        let empty = snapshot(&[], &Query::default(), now);
        assert_eq!(empty["journal"]["entries"], json!([]));
        assert_eq!(empty["journal"]["pages"], 1);
    }

    #[test]
    fn rejects_invalid_tags_entries_and_unknown_parents_without_writes() {
        let dir = TestDir::new();
        let mut store = dir.store();
        for name in [" ".to_string(), "x".repeat(81)] {
            assert!(store.apply(Command::AddTag { name }).is_err());
        }
        store.apply(Command::AddTag { name: "Walk".into() }).unwrap();
        assert!(store.apply(Command::AddTag { name: " walk ".into() }).is_err());
        let id = tag_id(&store);
        assert!(store.apply(Command::AddEntry { tag_id: id, note: " ".into() }).is_err());
        assert!(store.apply(Command::Click { tag_id: "missing".into() }).is_err());
        assert_eq!(store.records().len(), 1);
        assert_eq!(dir.store().records().len(), 1);
    }

    #[test]
    fn counts_calendar_boundaries_and_excludes_future_events() {
        let records = vec![
            Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
            event("a", "2025-12-31T23:59:59-08:00"),
            event("a", "2026-01-01T00:00:00-08:00"),
            event("a", "2026-01-05T00:00:00-08:00"), // Monday, inclusive.
            event("a", "2026-01-05T10:00:00-08:00"),
            event("a", "2026-01-05T10:20:00-08:00"),
            event("a", "2026-01-05T10:20:30-08:00"),
            event("a", "2026-01-05T10:20:31-08:00"), // Future.
        ];
        let state = snapshot(&records, &Query::default(), date("2026-01-05T10:20:30.500-08:00"));
        assert_eq!(state["totals"], json!([6, 5, 5, 4, 4, 3, 2, 1]));
        assert_eq!(floor(date("2026-01-01T12:00:00-08:00"), Unit::Week), date("2025-12-29T00:00:00-08:00"));
        assert_eq!(previous(date("2024-03-01T00:00:00-08:00"), Unit::Month), date("2024-02-01T00:00:00-08:00"));
    }

    #[test]
    fn pacific_days_follow_dst_and_local_midnight() {
        for (now, midnight, prior_midnight, hours) in [
            ("2026-03-09T12:00:00Z", "2026-03-09T07:00:00Z", "2026-03-08T08:00:00Z", 23),
            ("2026-11-02T12:00:00Z", "2026-11-02T08:00:00Z", "2026-11-01T07:00:00Z", 25),
        ] {
            let start = floor(date(now), Unit::Day);
            let previous_start = previous(start, Unit::Day);
            assert_eq!(start, date(midnight));
            assert_eq!(previous_start, date(prior_midnight));
            assert_eq!((start - previous_start).num_hours(), hours);
        }
        // UTC is already April, but Pacific time is still in March.
        let now = date("2026-04-01T06:59:59Z");
        assert_eq!(floor(now, Unit::Month), date("2026-03-01T08:00:00Z"));
        assert_eq!(floor(now, Unit::Day), date("2026-03-31T07:00:00Z"));
        assert_eq!(previous(date("2026-03-09T07:00:00Z"), Unit::Week), date("2026-03-02T08:00:00Z"));
        assert_eq!(previous(date("2026-04-01T07:00:00Z"), Unit::Month), date("2026-03-01T08:00:00Z"));
    }

    #[test]
    fn hourly_history_handles_both_dst_transitions() {
        for (first, second) in [
            // Spring: 01:30 PST, then 03:30 PDT (no 02:00 bucket).
            ("2026-03-08T09:30:00Z", "2026-03-08T10:30:00Z"),
            // Fall: 01:30 PDT, then 01:30 PST (two distinct buckets).
            ("2026-11-01T08:30:00Z", "2026-11-01T09:30:00Z"),
        ] {
            let records = vec![
                Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
                event("a", first), event("a", second),
            ];
            let query = Query { unit: Unit::Hour, ..Query::default() };
            let state = snapshot(&records, &query, date(second));
            assert_eq!(state["totals"][4], 2); // Both are in the same Pacific day.
            assert_eq!(state["totals"][5], 1); // Only the latest hour.
            assert_eq!(state["history"][28]["count"], 1);
            assert_eq!(state["history"][29]["count"], 1);
            assert_eq!(previous(floor(date(second), Unit::Hour), Unit::Hour), floor(date(first), Unit::Hour));
        }
    }

    #[test]
    fn daily_history_counts_all_events_on_short_and_long_days() {
        for (first, last, now) in [
            ("2026-03-08T08:00:00Z", "2026-03-09T06:59:59Z", "2026-03-09T07:00:00Z"),
            ("2026-11-01T07:00:00Z", "2026-11-02T07:59:59Z", "2026-11-02T08:00:00Z"),
        ] {
            let records = vec![
                Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
                event("a", first), event("a", last), event("a", now),
            ];
            let state = snapshot(&records, &Query::default(), date(now));
            assert_eq!(state["totals"][4], 1);
            assert_eq!(state["history"][28]["count"], 2);
            assert_eq!(state["history"][29]["count"], 1);
        }
    }

    #[test]
    fn history_zero_fills_and_filters_without_changing_overall_totals() {
        let records = vec![
            Record::Tag { deleted: false, id: "a".into(), name: "A".into() },
            Record::Tag { deleted: false, id: "b".into(), name: "B".into() },
            event("a", "2026-10-04T12:00:00Z"),
            event("b", "2026-10-04T12:00:00Z"),
        ];
        let query = Query { unit: Unit::Second, tag: Some("a".into()), ..Query::default() };
        let state = snapshot(&records, &query, date("2026-10-04T12:00:01Z"));
        assert_eq!(state["totals"][0], 2);
        assert_eq!(state["matching_events"], 1);
        assert_eq!(state["history"].as_array().unwrap().len(), 30);
        assert_eq!(state["history"][28]["count"], 1);
        assert_eq!(state["history"][29]["count"], 0);
    }

    #[test]
    fn failed_save_keeps_memory_and_previous_generation() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "Walk".into() }).unwrap();
        fs::create_dir(dir.0.join("CURRENT.tmp")).unwrap(); // Force manifest write failure.
        let id = tag_id(&store);
        assert!(store.apply(Command::Click { tag_id: id }).is_err());
        assert_eq!(store.records().len(), 1);
        assert_eq!(dir.store().records().len(), 1);
    }

    #[test]
    fn concurrent_clicks_are_not_lost() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "Walk".into() }).unwrap();
        let id = tag_id(&store);
        let shared = Arc::new(Mutex::new(store));
        let threads: Vec<_> = (0..8).map(|_| {
            let shared = Arc::clone(&shared);
            let id = id.clone();
            std::thread::spawn(move || shared.lock().unwrap().apply(Command::Click { tag_id: id }).unwrap())
        }).collect();
        for thread in threads { thread.join().unwrap(); }
        assert_eq!(dir.store().records().len(), 9);
    }

    #[test]
    fn corrupt_storage_is_never_silently_reset() {
        let dir = TestDir::new();
        let mut store = dir.store();
        store.apply(Command::AddTag { name: "Walk".into() }).unwrap();
        let generation = fs::read_to_string(dir.0.join("CURRENT")).unwrap();
        fs::write(dir.0.join(generation).join("records/records_part_0.json"), "broken").unwrap();
        assert!(Store::open(dir.0.clone()).is_err());
    }

    #[async_std::test]
    async fn http_edits_survive_reopening_and_preserve_frequency() {
        async fn post(app: &tide::Server<()>, command: Value) -> Value {
            let mut request = tide::http::Request::new(tide::http::Method::Post,
                "http://localhost/organizer?unit=day&archive_unit=day&journal_page=0&archive_page=0"
                    .parse::<tide::http::Url>().unwrap());
            request.insert_header("Content-Type", "application/json");
            request.set_body(command.to_string());
            let mut response: tide::http::Response = app.respond(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::Ok);
            response.body_json().await.unwrap()
        }

        let dir = TestDir::new();
        let mut app = tide::new();
        mount_store(&mut app, Arc::new(Mutex::new(dir.store()))).unwrap();
        let added = post(&app, json!({"action":"add_tag", "name":"Morning walk"})).await;
        let tag = added["tags"][0]["id"].clone();
        let before = post(&app, json!({"action":"add_entry", "tag_id":tag, "note":"First note"})).await;
        let entry = before["journal"]["entries"][0]["id"].clone();
        let renamed = post(&app, json!({"action":"rename_tag", "tag_id":tag, "name":"Evening walk"})).await;
        assert_eq!(renamed["tags"][0]["name"], "Evening walk");
        let edited = post(&app, json!({"action":"edit_entry", "entry_id":entry, "note":"Updated note"})).await;
        assert_eq!(edited["journal"]["entries"][0]["note"], "Updated note");
        drop(app);

        let mut reopened = tide::new();
        mount_store(&mut reopened, Arc::new(Mutex::new(dir.store()))).unwrap();
        let saved = post(&reopened, json!({"action":"read"})).await;
        assert_eq!(saved["tags"][0]["id"], tag);
        assert_eq!(saved["tags"][0]["name"], "Evening walk");
        assert_eq!(saved["archive"]["tags"][0]["name"], "Evening walk");
        assert_eq!(saved["journal"]["entries"][0]["id"], entry);
        assert_eq!(saved["journal"]["entries"][0]["note"], "Updated note");
        assert_eq!(saved["journal"]["entries"][0]["at"], before["journal"]["entries"][0]["at"]);
        assert_eq!(saved["totals"][0], 1);
        assert_eq!(saved["archive"]["total"], 1);
    }

    #[async_std::test]
    async fn single_route_serves_html_and_validates_json_requests() {
        let dir = TestDir::new();
        let mut app = tide::new();
        mount_store(&mut app, Arc::new(Mutex::new(dir.store()))).unwrap();
        let mut response: tide::http::Response = app.respond(tide::http::Request::new(
            tide::http::Method::Get, "http://localhost/organizer".parse::<tide::http::Url>().unwrap(),
        )).await.unwrap();
        assert_eq!(response.status(), StatusCode::Ok);
        assert!(response.body_string().await.unwrap().contains("Frequency table"));
        for (path, content_type, body, expected) in [
            ("/organizer", "application/json", r#"{"action":"read"}"#.to_string(), StatusCode::Ok),
            ("/organizer", "text/plain", r#"{"action":"click","tag_id":"x"}"#.to_string(), StatusCode::UnsupportedMediaType),
            ("/organizer", "application/json", "broken".to_string(), StatusCode::BadRequest),
            ("/organizer?unit=century", "application/json", r#"{"action":"read"}"#.to_string(), StatusCode::BadRequest),
            ("/organizer?archive_from=2026-11-02&archive_to=2026-11-01", "application/json", r#"{"action":"add_tag","name":"Invalid range"}"#.to_string(), StatusCode::BadRequest),
            ("/organizer?archive_from=invalid", "application/json", r#"{"action":"read"}"#.to_string(), StatusCode::BadRequest),
            ("/organizer", "application/json", "x".repeat(16385), StatusCode::PayloadTooLarge),
            ("/organizer", "application/json", r#"{"action":"add_tag","name":"Reading"}"#.to_string(), StatusCode::Ok),
        ] {
            let mut request = tide::http::Request::new(tide::http::Method::Post,
                format!("http://localhost{path}").parse::<tide::http::Url>().unwrap());
            request.insert_header("Content-Type", content_type);
            request.set_body(body);
            let mut response: tide::http::Response = app.respond(request).await.unwrap();
            assert_eq!(response.status(), expected);
            let _: Value = response.body_json().await.unwrap();
        }
        assert_eq!(dir.store().records().len(), 1);
    }

    #[test]
    fn grows_and_restores_multiple_partitions() {
        let dir = TestDir::new();
        let mut store = dir.store();
        for i in 0..260 {
            let Value::Object(row) = serde_json::to_value(Record::Tag {
                deleted: false, id: i.to_string(), name: format!("Tag {i}"),
            }).unwrap() else { unreachable!() };
            store.rows.add(|slot| *slot = row.clone()).unwrap();
        }
        store.insert(event("259", "2026-10-04T00:00:00Z")).unwrap();
        let reopened = dir.store();
        assert!(reopened.rows.db_size > 1);
        assert_eq!(reopened.records().len(), 261);
    }
}
