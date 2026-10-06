use serde::{Serialize, de::DeserializeOwned};
use std::{fs, io::{self, Write}, path::Path};

pub fn load<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn save<T: Serialize>(path: &Path, snapshot: &T) -> io::Result<()> {
    let payload = serde_json::to_vec_pretty(snapshot).map_err(io::Error::other)?;
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".memory-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(&payload)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        let _ = fs::File::open(parent).and_then(|directory| directory.sync_all());
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn update<T: Clone, Output>(
    current: &mut T,
    change: impl FnOnce(&mut T) -> Result<Output, String>,
    persist: impl FnOnce(&T) -> io::Result<()>,
) -> Result<Output, String> {
    let mut next = current.clone();
    let output = change(&mut next)?;
    persist(&next).map_err(|error| format!("Could not persist variables; change was not applied: {error}"))?;
    *current = next;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use partitioned_array_rust::PartitionedArray;
    use serde_json::{Value, json};
    use std::path::PathBuf;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("ruby-persistence-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    fn store() -> PartitionedArray {
        let mut entries = PartitionedArray::new(1, 2, 1, true);
        entries.allocate(false);
        entries
    }

    #[test]
    fn partitioned_values_and_deletions_survive_reopening() {
        let directory = TestDir::new();
        let path = directory.0.join("memory.json");
        let mut entries = store();
        let value = json!({"score":42,"items":["key",true,null],"text":"héllo"});
        let entry_id = update(&mut entries, |next| {
            next.add(|row| {
                row.insert("name".into(), json!("__rf_session_player__progress"));
                row.insert("value".into(), value.clone());
            }).ok_or_else(|| "full".to_string())
        }, |next| save(&path, next)).unwrap();
        let mut restored: PartitionedArray = load(&path).unwrap().unwrap();
        assert_eq!(restored.get(entry_id).unwrap()["value"], value);
        update(&mut restored, |next| {
            next.delete(entry_id);
            Ok(())
        }, |next| save(&path, next)).unwrap();
        let restored: PartitionedArray = load(&path).unwrap().unwrap();
        assert!(restored.non_empty_ids().is_empty());
    }

    #[test]
    fn rejected_changes_and_failed_saves_preserve_memory_and_disk() {
        let directory = TestDir::new();
        let path = directory.0.join("memory.json");
        let mut entries = store();
        save(&path, &entries).unwrap();
        let before = fs::read(&path).unwrap();
        let blocker = directory.0.join("not-a-directory");
        fs::write(&blocker, "occupied").unwrap();
        let failed = update(&mut entries, |next| {
            next.add(|row| { row.insert("value".into(), json!(99)); });
            Ok(())
        }, |next| save(&blocker.join("memory.json"), next));
        assert!(failed.unwrap_err().contains("change was not applied"));
        assert!(entries.non_empty_ids().is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        let rejected: Result<(), String> = update(&mut entries, |next| {
            next.add(|row| { row.insert("value".into(), json!(99)); });
            Err("invalid change".into())
        }, |next| save(&path, next));
        assert!(rejected.is_err());
        assert!(entries.non_empty_ids().is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        let target_directory = directory.0.join("directory.json");
        fs::create_dir(&target_directory).unwrap();
        assert!(save(&target_directory, &entries).is_err());
        assert!(!fs::read_dir(&directory.0).unwrap().any(|entry| {
            entry.unwrap().file_name().to_string_lossy().ends_with(".tmp")
        }));
    }

    #[test]
    fn only_missing_snapshots_start_empty() {
        let directory = TestDir::new();
        let path = directory.0.join("memory.json");
        assert!(load::<Value>(&path).unwrap().is_none());
        fs::write(&path, "broken JSON").unwrap();
        assert!(load::<Value>(&path).is_err());
        assert!(load::<Value>(&directory.0).is_err());
    }
}
