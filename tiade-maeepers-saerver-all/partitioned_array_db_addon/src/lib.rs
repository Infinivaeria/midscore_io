use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PartitionedArrayDb {
    root: PathBuf,
    chunk_size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct PartitionMetadata {
    version: u8,
    table: String,
    parts: usize,
    total_bytes: usize,
}

impl PartitionedArrayDb {
    pub fn new<P: AsRef<Path>>(root: P, chunk_size: usize) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            chunk_size: chunk_size.max(256),
        })
    }

    pub fn load_json_or_default<T>(&self, table: &str) -> T
    where
        T: DeserializeOwned + Default,
    {
        match self.load_json(table) {
            Ok(value) => value,
            Err(_) => T::default(),
        }
    }

    pub fn load_json<T>(&self, table: &str) -> io::Result<T>
    where
        T: DeserializeOwned,
    {
        let table_dir = self.table_dir(table);
        let meta_path = table_dir.join("metadata.json");
        let meta_raw = fs::read_to_string(meta_path)?;
        let meta: PartitionMetadata = serde_json::from_str(&meta_raw).map_err(invalid_data)?;

        let mut bytes = Vec::with_capacity(meta.total_bytes);
        for i in 0..meta.parts {
            let part_path = table_dir.join(format!("part_{i}.bin"));
            let part = fs::read(part_path)?;
            bytes.extend_from_slice(&part);
        }

        serde_json::from_slice::<T>(&bytes).map_err(invalid_data)
    }

    pub fn save_json<T>(&self, table: &str, value: &T) -> io::Result<()>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec(value).map_err(invalid_data)?;
        let table_dir = self.table_dir(table);
        fs::create_dir_all(&table_dir)?;

        let mut start = 0usize;
        let mut part_count = 0usize;
        while start < bytes.len() {
            let end = (start + self.chunk_size).min(bytes.len());
            let part_path = table_dir.join(format!("part_{part_count}.bin"));
            fs::write(part_path, &bytes[start..end])?;
            part_count += 1;
            start = end;
        }

        if bytes.is_empty() {
            fs::write(table_dir.join("part_0.bin"), [])?;
            part_count = 1;
        }

        self.remove_stale_parts(&table_dir, part_count)?;

        let metadata = PartitionMetadata {
            version: 1,
            table: table.to_string(),
            parts: part_count,
            total_bytes: bytes.len(),
        };
        let meta_raw = serde_json::to_string_pretty(&metadata).map_err(invalid_data)?;
        fs::write(table_dir.join("metadata.json"), meta_raw)?;
        Ok(())
    }

    fn table_dir(&self, table: &str) -> PathBuf {
        self.root.join(sanitize_table_name(table))
    }

    fn remove_stale_parts(&self, table_dir: &Path, keep_parts: usize) -> io::Result<()> {
        let entries = fs::read_dir(table_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.starts_with("part_") || !name.ends_with(".bin") {
                continue;
            }
            let idx_raw = name.trim_start_matches("part_").trim_end_matches(".bin");
            let Ok(idx) = idx_raw.parse::<usize>() else {
                continue;
            };
            if idx >= keep_parts {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }
}

fn sanitize_table_name(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn invalid_data<E: ToString>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}
