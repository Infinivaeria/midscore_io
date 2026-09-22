use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

pub type Row = Map<String, Value>;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionedArray {
    pub db_size: usize,
    pub partition_amount_and_offset: usize,
    pub partition_addition_amount: usize,
    pub dynamically_allocates: bool,
    pub allocated: bool,
    pub range_arr: Vec<(usize, usize)>,
    pub rel_arr: Vec<usize>,
    pub data_arr: Vec<Row>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionedArraySnapshot {
    pub db_size: usize,
    pub partition_amount_and_offset: usize,
    pub partition_addition_amount: usize,
    pub dynamically_allocates: bool,
    pub allocated: bool,
    pub range_arr: Vec<(usize, usize)>,
    pub rel_arr: Vec<usize>,
}

impl Default for PartitionedArray {
    fn default() -> Self {
        Self::new(10, 4, 1, true)
    }
}

impl PartitionedArray {
    pub fn new(
        db_size: usize,
        partition_amount_and_offset: usize,
        partition_addition_amount: usize,
        dynamically_allocates: bool,
    ) -> Self {
        Self {
            db_size,
            partition_amount_and_offset: partition_amount_and_offset.max(1),
            partition_addition_amount: partition_addition_amount.max(1),
            dynamically_allocates,
            allocated: false,
            range_arr: Vec::new(),
            rel_arr: Vec::new(),
            data_arr: Vec::new(),
        }
    }

    pub fn allocate(&mut self, override_existing: bool) {
        if self.allocated && !override_existing {
            return;
        }

        self.range_arr.clear();
        self.rel_arr.clear();
        self.data_arr.clear();

        let mut partition_high = 0usize;
        for i in 0..self.db_size {
            if i == 0 {
                partition_high = self.partition_amount_and_offset;
                self.range_arr.push((0, partition_high));
            } else {
                let start = partition_high + 1;
                partition_high += self.partition_amount_and_offset;
                self.range_arr.push((start, partition_high));
            }
        }

        let total_elements = if self.db_size == 0 {
            0
        } else {
            self.db_size * self.partition_amount_and_offset + 1
        };
        self.data_arr = (0..total_elements).map(|_| Row::new()).collect();
        self.rel_arr = (0..self.data_arr.len()).collect();
        self.allocated = true;
    }

    pub fn len(&self) -> usize {
        self.data_arr.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data_arr.is_empty()
    }

    pub fn at_capacity(&self) -> bool {
        self.data_arr.iter().all(|row| !row.is_empty())
    }

    pub fn get(&self, id: usize) -> Option<&Row> {
        self.data_arr.get(id)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut Row> {
        self.data_arr.get_mut(id)
    }

    pub fn set_with<F>(&mut self, id: usize, mut f: F) -> bool
    where
        F: FnMut(&mut Row),
    {
        if let Some(row) = self.data_arr.get_mut(id) {
            f(row);
            true
        } else {
            false
        }
    }

    pub fn delete(&mut self, id: usize) -> bool {
        if let Some(row) = self.data_arr.get_mut(id) {
            row.clear();
            true
        } else {
            false
        }
    }

    pub fn get_partition(&self, partition_id: usize) -> Option<Vec<Row>> {
        let (start, end) = *self.range_arr.get(partition_id)?;
        let range: RangeInclusive<usize> = start..=end;
        Some(
            range
                .filter_map(|i| self.data_arr.get(i).cloned())
                .collect(),
        )
    }

    pub fn get_partition_id(&self, id: usize) -> Option<usize> {
        self.range_arr
            .iter()
            .position(|(start, end)| id >= *start && id <= *end)
    }

    pub fn set_partition_subelement_with<F>(
        &mut self,
        partition_id: usize,
        partition_local_index: usize,
        mut f: F,
    ) -> bool
    where
        F: FnMut(&mut Row),
    {
        let (start, end) = match self.range_arr.get(partition_id).copied() {
            Some(v) => v,
            None => return false,
        };
        let absolute_id = start.saturating_add(partition_local_index);
        if absolute_id > end {
            return false;
        }
        self.set_with(absolute_id, |row| f(row))
    }

    pub fn delete_partition(&mut self, partition_id: usize) -> bool {
        let (start, end) = match self.range_arr.get(partition_id).copied() {
            Some(v) => v,
            None => return false,
        };
        for i in start..=end {
            if let Some(row) = self.data_arr.get_mut(i) {
                row.clear();
            }
        }
        true
    }

    pub fn add_partition(&mut self) {
        let (last_start, last_end) = self.range_arr.last().copied().unwrap_or((0, 0));
        let start = if self.range_arr.is_empty() {
            0
        } else {
            last_end + 1
        };
        let end = if self.range_arr.is_empty() {
            self.partition_amount_and_offset
        } else {
            start + self.partition_amount_and_offset - 1
        };

        if self.range_arr.is_empty() {
            let len = self.partition_amount_and_offset + 1;
            self.data_arr.extend((0..len).map(|_| Row::new()));
        } else {
            self.data_arr
                .extend((0..self.partition_amount_and_offset).map(|_| Row::new()));
        }

        self.db_size = if self.range_arr.is_empty() {
            1
        } else {
            self.db_size + 1
        };
        self.range_arr.push((start, end));
        let expected_len = self.data_arr.len();
        self.rel_arr = (0..expected_len).collect();

        let _ = last_start;
    }

    pub fn add<F>(&mut self, mut f: F) -> Option<usize>
    where
        F: FnMut(&mut Row),
    {
        for (idx, row) in self.data_arr.iter_mut().enumerate() {
            if row.is_empty() {
                f(row);
                if self.dynamically_allocates
                    && idx == self.data_arr.len() - 1
                    && self.at_capacity()
                {
                    for _ in 0..self.partition_addition_amount {
                        self.add_partition();
                    }
                }
                return Some(idx);
            }
        }

        if self.dynamically_allocates {
            for _ in 0..self.partition_addition_amount {
                self.add_partition();
            }
            return self.add(f);
        }
        None
    }

    pub fn snapshot(&self) -> PartitionedArraySnapshot {
        PartitionedArraySnapshot {
            db_size: self.db_size,
            partition_amount_and_offset: self.partition_amount_and_offset,
            partition_addition_amount: self.partition_addition_amount,
            dynamically_allocates: self.dynamically_allocates,
            allocated: self.allocated,
            range_arr: self.range_arr.clone(),
            rel_arr: self.rel_arr.clone(),
        }
    }

    pub fn non_empty_ids(&self) -> Vec<usize> {
        self.data_arr
            .iter()
            .enumerate()
            .filter_map(|(id, row)| if row.is_empty() { None } else { Some(id) })
            .collect()
    }

    pub fn rehash_compact(&mut self) -> usize {
        let non_empty_ids_before = self.non_empty_ids();
        let compacted_slots = non_empty_ids_before
            .iter()
            .enumerate()
            .filter(|(new_id, old_id)| **old_id != *new_id)
            .count();

        let rows: Vec<Row> = self
            .data_arr
            .iter()
            .filter(|row| !row.is_empty())
            .cloned()
            .collect();

        self.allocate(true);
        for row in rows {
            let _ = self.add(|slot| {
                *slot = row.clone();
            });
        }

        compacted_slots
    }

    pub fn save_to_dir<P: AsRef<Path>>(&self, base_dir: P, db_name: &str) -> io::Result<()> {
        let dir = base_dir.as_ref().join(db_name);
        fs::create_dir_all(&dir)?;

        let snapshot_raw = serde_json::to_string_pretty(&self.snapshot())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        fs::write(dir.join("metadata.json"), snapshot_raw)?;

        for (partition_id, (start, end)) in self.range_arr.iter().copied().enumerate() {
            let part_rows: Vec<Row> = (start..=end)
                .filter_map(|id| self.data_arr.get(id).cloned())
                .collect();
            let raw = serde_json::to_string_pretty(&part_rows)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            fs::write(
                dir.join(format!("{}_part_{}.json", db_name, partition_id)),
                raw,
            )?;
        }

        Ok(())
    }

    pub fn load_from_dir<P: AsRef<Path>>(&mut self, base_dir: P, db_name: &str) -> io::Result<()> {
        let dir = base_dir.as_ref().join(db_name);
        let snapshot_raw = fs::read_to_string(dir.join("metadata.json"))?;
        let snapshot: PartitionedArraySnapshot = serde_json::from_str(&snapshot_raw)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        self.db_size = snapshot.db_size;
        self.partition_amount_and_offset = snapshot.partition_amount_and_offset;
        self.partition_addition_amount = snapshot.partition_addition_amount;
        self.dynamically_allocates = snapshot.dynamically_allocates;
        self.allocated = snapshot.allocated;
        self.range_arr = snapshot.range_arr;
        self.rel_arr = snapshot.rel_arr;

        let mut loaded_data = vec![Row::new(); self.rel_arr.len()];
        for (partition_id, (start, end)) in self.range_arr.iter().copied().enumerate() {
            let path = dir.join(format!("{}_part_{}.json", db_name, partition_id));
            let part_raw = fs::read_to_string(path)?;
            let rows: Vec<Row> = serde_json::from_str(&part_raw)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            for (offset, absolute) in (start..=end).enumerate() {
                if let Some(row) = rows.get(offset)
                    && let Some(dst) = loaded_data.get_mut(absolute)
                {
                    *dst = row.clone();
                }
            }
        }
        self.data_arr = loaded_data;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaxCapacity {
    DataArrSize,
    Fixed(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPartitionedArray {
    pub pa: PartitionedArray,
    pub latest_id: usize,
    pub has_capacity: bool,
    pub endless_add: bool,
    pub max_capacity: MaxCapacity,
}

impl ManagedPartitionedArray {
    pub fn new(pa: PartitionedArray) -> Self {
        Self {
            pa,
            latest_id: 0,
            has_capacity: true,
            endless_add: false,
            max_capacity: MaxCapacity::DataArrSize,
        }
    }

    pub fn with_config(
        pa: PartitionedArray,
        has_capacity: bool,
        endless_add: bool,
        max_capacity: MaxCapacity,
    ) -> Self {
        Self {
            pa,
            latest_id: 0,
            has_capacity,
            endless_add,
            max_capacity,
        }
    }

    pub fn allocate(&mut self, override_existing: bool) {
        self.pa.allocate(override_existing);
        self.latest_id = 0;
    }

    pub fn at_capacity(&self) -> bool {
        if !self.has_capacity {
            return false;
        }
        match self.max_capacity {
            MaxCapacity::DataArrSize => self.latest_id >= self.pa.data_arr.len(),
            MaxCapacity::Fixed(max) => self.latest_id >= max,
        }
    }

    pub fn add<F>(&mut self, mut f: F) -> Option<usize>
    where
        F: FnMut(&mut Row),
    {
        if self.endless_add && self.at_capacity() {
            for _ in 0..self.pa.partition_addition_amount {
                self.pa.add_partition();
            }
        } else if self.at_capacity() {
            return None;
        }

        let id = self.pa.add(|row| f(row))?;
        self.latest_id = id.saturating_add(1);
        Some(id)
    }

    pub fn save_to_dir<P: AsRef<Path>>(&self, base_dir: P, db_name: &str) -> io::Result<()> {
        let mut root = PathBuf::from(base_dir.as_ref());
        root.push(db_name);
        fs::create_dir_all(&root)?;

        let state = serde_json::json!({
            "latest_id": self.latest_id,
            "has_capacity": self.has_capacity,
            "endless_add": self.endless_add,
            "max_capacity": self.max_capacity,
        });
        fs::write(
            root.join("managed_state.json"),
            serde_json::to_string_pretty(&state)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?,
        )?;

        self.pa.save_to_dir(base_dir, db_name)
    }

    pub fn load_from_dir<P: AsRef<Path>>(&mut self, base_dir: P, db_name: &str) -> io::Result<()> {
        self.pa.load_from_dir(&base_dir, db_name)?;

        let mut root = PathBuf::from(base_dir.as_ref());
        root.push(db_name);
        let raw = fs::read_to_string(root.join("managed_state.json"))?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        self.latest_id = v
            .get("latest_id")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or(0);
        self.has_capacity = v
            .get("has_capacity")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        self.endless_add = v
            .get("endless_add")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.max_capacity =
            serde_json::from_value(v.get("max_capacity").cloned().unwrap_or(Value::Null))
                .unwrap_or(MaxCapacity::DataArrSize);

        Ok(())
    }

    pub fn rehash_compact(&mut self) -> usize {
        let removed = self.pa.rehash_compact();
        self.latest_id = self
            .pa
            .data_arr
            .iter()
            .position(Row::is_empty)
            .unwrap_or(self.pa.data_arr.len());
        removed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineDbConfig {
    pub endless_add: bool,
    pub has_capacity: bool,
    pub db_size: usize,
    pub dynamically_allocates: bool,
    pub partition_amount: usize,
    pub partition_addition_amount: usize,
    pub max_capacity: MaxCapacity,
}

impl Default for LineDbConfig {
    fn default() -> Self {
        Self {
            endless_add: true,
            has_capacity: true,
            db_size: 100,
            dynamically_allocates: true,
            partition_amount: 20,
            partition_addition_amount: 1,
            max_capacity: MaxCapacity::DataArrSize,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContextManagedPartitionedArray {
    pub database_folder_name: PathBuf,
    pub managed: ManagedPartitionedArray,
}

impl FileContextManagedPartitionedArray {
    pub fn new<P: AsRef<Path>>(database_folder_name: P, config: &LineDbConfig) -> io::Result<Self> {
        let folder = database_folder_name.as_ref().to_path_buf();
        fs::create_dir_all(&folder)?;

        let mut managed = ManagedPartitionedArray::with_config(
            PartitionedArray::new(
                config.db_size,
                config.partition_amount,
                config.partition_addition_amount,
                config.dynamically_allocates,
            ),
            config.has_capacity,
            config.endless_add,
            config.max_capacity.clone(),
        );

        if folder.join("data").join("metadata.json").exists() {
            managed.load_from_dir(&folder, "data")?;
        } else {
            managed.allocate(false);
            managed.save_to_dir(&folder, "data")?;
        }

        Ok(Self {
            database_folder_name: folder,
            managed,
        })
    }

    pub fn save(&self) -> io::Result<()> {
        self.managed.save_to_dir(&self.database_folder_name, "data")
    }

    pub fn reload(&mut self) -> io::Result<()> {
        self.managed
            .load_from_dir(&self.database_folder_name, "data")
    }

    pub fn add_row(&mut self, row: Row) -> io::Result<usize> {
        let id = self.managed.add(|slot| {
            *slot = row.clone();
        });
        match id {
            Some(id) => {
                self.save()?;
                Ok(id)
            }
            None => Err(io::Error::other("managed partitioned array is at capacity")),
        }
    }

    pub fn get_row(&self, id: usize) -> Option<&Row> {
        self.managed.pa.get(id)
    }

    pub fn set_row(&mut self, id: usize, row: Row) -> io::Result<bool> {
        let changed = self.managed.pa.set_with(id, |slot| {
            *slot = row.clone();
        });
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    pub fn delete_row(&mut self, id: usize) -> io::Result<bool> {
        let changed = self.managed.pa.delete(id);
        if changed {
            self.save()?;
        }
        Ok(changed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContextManagedPartitionedArrayManager {
    pub parent_folder: PathBuf,
    pub children: BTreeMap<String, FileContextManagedPartitionedArray>,
    pub config: LineDbConfig,
}

impl FileContextManagedPartitionedArrayManager {
    pub fn new<P: AsRef<Path>>(parent_folder: P, config: LineDbConfig) -> io::Result<Self> {
        fs::create_dir_all(parent_folder.as_ref())?;
        Ok(Self {
            parent_folder: parent_folder.as_ref().to_path_buf(),
            children: BTreeMap::new(),
            config,
        })
    }

    pub fn load_single(&mut self, db_name: &str) -> io::Result<()> {
        let full = self.parent_folder.join(db_name);
        let child = FileContextManagedPartitionedArray::new(full, &self.config)?;
        self.children.insert(db_name.to_string(), child);
        Ok(())
    }

    pub fn remove_single(&mut self, db_name: &str) -> bool {
        self.children.remove(db_name).is_some()
    }

    pub fn get(&self, db_name: &str) -> Option<&FileContextManagedPartitionedArray> {
        self.children.get(db_name)
    }

    pub fn get_mut(&mut self, db_name: &str) -> Option<&mut FileContextManagedPartitionedArray> {
        self.children.get_mut(db_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionedArrayDatabase {
    pub database_folder_name: PathBuf,
    pub fcmpa: FileContextManagedPartitionedArray,
}

impl PartitionedArrayDatabase {
    pub fn new<P: AsRef<Path>>(database_folder_name: P, config: &LineDbConfig) -> io::Result<Self> {
        let folder = database_folder_name.as_ref().to_path_buf();
        let fcmpa = FileContextManagedPartitionedArray::new(&folder, config)?;
        Ok(Self {
            database_folder_name: folder,
            fcmpa,
        })
    }

    pub fn add_row(&mut self, row: Row) -> io::Result<usize> {
        self.fcmpa.add_row(row)
    }

    pub fn get_row(&self, id: usize) -> Option<&Row> {
        self.fcmpa.get_row(id)
    }

    pub fn set_row(&mut self, id: usize, row: Row) -> io::Result<bool> {
        self.fcmpa.set_row(id, row)
    }

    pub fn delete_row(&mut self, id: usize) -> io::Result<bool> {
        self.fcmpa.delete_row(id)
    }

    pub fn reload(&mut self) -> io::Result<()> {
        self.fcmpa.reload()
    }

    pub fn rehash_compact(&mut self) -> io::Result<usize> {
        let removed = self.fcmpa.managed.rehash_compact();
        self.fcmpa.save()?;
        Ok(removed)
    }
}

#[derive(Debug, Clone)]
pub struct LineDb {
    pub parent_folder: PathBuf,
    pub database_folder_name: String,
    pub database_file_name: PathBuf,
    pub linelist: BTreeMap<String, PartitionedArrayDatabase>,
    pub active_database: Option<String>,
    pub config: LineDbConfig,
}

impl LineDb {
    pub fn new<P: AsRef<Path>>(
        parent_folder: P,
        database_folder_name: &str,
        database_file_name: &str,
        config: LineDbConfig,
    ) -> io::Result<Self> {
        let parent = parent_folder.as_ref().to_path_buf();
        let db_folder = parent.join(database_folder_name);
        fs::create_dir_all(&db_folder)?;

        let db_file = PathBuf::from(database_file_name);
        if let Some(parent_dir) = db_file.parent() {
            fs::create_dir_all(parent_dir)?;
        }
        if !db_file.exists() {
            fs::write(&db_file, "")?;
        }

        let mut linedb = Self {
            parent_folder: parent,
            database_folder_name: database_folder_name.to_string(),
            database_file_name: db_file,
            linelist: BTreeMap::new(),
            active_database: None,
            config,
        };
        linedb.reload()?;
        Ok(linedb)
    }

    pub fn list_databases(&self) -> Vec<String> {
        self.linelist.keys().cloned().collect()
    }

    pub fn databases(&self) -> Vec<String> {
        self.list_databases()
    }

    pub fn has_database(&self, db_name: &str) -> bool {
        let Some(name) = sanitize_db_name(db_name) else {
            return false;
        };
        self.linelist.contains_key(&name)
    }

    pub fn add_db(&mut self, db_name: &str) -> io::Result<bool> {
        let Some(name) = sanitize_db_name(db_name) else {
            return Ok(false);
        };

        let mut lines = read_file_lines(&self.database_file_name)?;
        if lines.iter().any(|line| line == &name) {
            return Ok(false);
        }
        lines.push(name.clone());
        write_lines(&self.database_file_name, &lines)?;
        self.load_pad_single(&name)?;
        Ok(true)
    }

    pub fn remove_db(&mut self, db_name: &str) -> io::Result<bool> {
        let Some(name) = sanitize_db_name(db_name) else {
            return Ok(false);
        };
        let mut lines = read_file_lines(&self.database_file_name)?;
        let before = lines.len();
        lines.retain(|line| line != &name);
        let changed = before != lines.len();
        if changed {
            write_lines(&self.database_file_name, &lines)?;
        }
        self.linelist.remove(&name);
        if self.active_database.as_deref() == Some(name.as_str()) {
            self.active_database = None;
        }
        Ok(changed)
    }

    pub fn delete_db(&mut self, db_name: &str) -> io::Result<bool> {
        let Some(name) = sanitize_db_name(db_name) else {
            return Ok(false);
        };
        let changed = self.remove_db(&name)?;
        let folder = self.parent_folder.join(&name);
        if folder.exists() {
            fs::remove_dir_all(folder)?;
        }
        Ok(changed)
    }

    pub fn reload(&mut self) -> io::Result<()> {
        self.linelist.clear();
        for db_name in read_file_lines(&self.database_file_name)? {
            self.load_pad_single(&db_name)?;
        }
        if let Some(active) = self.active_database.clone()
            && !self.linelist.contains_key(&active)
        {
            self.active_database = None;
        }
        Ok(())
    }

    pub fn db(&mut self, db_name: &str) -> Option<&PartitionedArrayDatabase> {
        let name = sanitize_db_name(db_name)?;
        self.active_database = Some(name.clone());
        self.linelist.get(&name)
    }

    pub fn db_mut(&mut self, db_name: &str) -> Option<&mut PartitionedArrayDatabase> {
        let name = sanitize_db_name(db_name)?;
        self.active_database = Some(name.clone());
        self.linelist.get_mut(&name)
    }

    pub fn active_database(&self) -> Option<&PartitionedArrayDatabase> {
        let active = self.active_database.as_ref()?;
        self.linelist.get(active)
    }

    pub fn active_database_name(&self) -> Option<&str> {
        self.active_database.as_deref()
    }

    pub fn add_row(&mut self, db_name: &str, row: Row) -> io::Result<usize> {
        if !self.has_database(db_name) {
            let _ = self.add_db(db_name)?;
        }
        let Some(db) = self.db_mut(db_name) else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "database not found",
            ));
        };
        db.add_row(row)
    }

    pub fn get_row(&mut self, db_name: &str, id: usize) -> Option<Row> {
        self.db(db_name).and_then(|db| db.get_row(id).cloned())
    }

    pub fn set_row(&mut self, db_name: &str, id: usize, row: Row) -> io::Result<bool> {
        let Some(db) = self.db_mut(db_name) else {
            return Ok(false);
        };
        db.set_row(id, row)
    }

    pub fn delete_row(&mut self, db_name: &str, id: usize) -> io::Result<bool> {
        let Some(db) = self.db_mut(db_name) else {
            return Ok(false);
        };
        db.delete_row(id)
    }

    pub fn rehash_database(&mut self, db_name: &str) -> io::Result<Option<usize>> {
        let Some(db) = self.db_mut(db_name) else {
            return Ok(None);
        };
        db.rehash_compact().map(Some)
    }

    pub fn save_json_value<T: Serialize>(&mut self, db_name: &str, value: &T) -> io::Result<()> {
        let encoded = serde_json::to_value(value)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        let mut row = Row::new();
        row.insert("value".to_string(), encoded);

        if !self.has_database(db_name) {
            let _ = self.add_db(db_name)?;
        }

        match self.set_row(db_name, 0, row.clone())? {
            true => Ok(()),
            false => {
                let _ = self.add_row(db_name, row)?;
                Ok(())
            }
        }
    }

    pub fn load_json_value<T: for<'a> Deserialize<'a>>(
        &mut self,
        db_name: &str,
    ) -> io::Result<Option<T>> {
        let Some(row) = self.get_row(db_name, 0) else {
            return Ok(None);
        };
        let Some(value) = row.get("value") else {
            return Ok(None);
        };
        let parsed = serde_json::from_value::<T>(value.clone())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(Some(parsed))
    }

    fn load_pad_single(&mut self, db_name: &str) -> io::Result<()> {
        let db_path = self.parent_folder.join(db_name);
        let db = PartitionedArrayDatabase::new(db_path, &self.config)?;
        self.linelist.insert(db_name.to_string(), db);
        Ok(())
    }
}

fn read_file_lines(path: &Path) -> io::Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    Ok(out)
}

fn write_lines(path: &Path, lines: &[String]) -> io::Result<()> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            normalized.push(trimmed.to_string());
        }
    }
    let mut raw = normalized.join("\n");
    if !raw.is_empty() {
        raw.push('\n');
    }
    fs::write(path, raw)
}

fn sanitize_db_name(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let collapsed = normalized.trim_matches('_').to_string();
    if collapsed.is_empty() || collapsed == "." || collapsed == ".." {
        None
    } else {
        Some(collapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn allocate_and_partition_ranges_match_expected_shape() {
        let mut pa = PartitionedArray::new(3, 4, 1, true);
        pa.allocate(false);
        assert_eq!(pa.range_arr, vec![(0, 4), (5, 8), (9, 12)]);
        assert_eq!(pa.len(), 13);
    }

    #[test]
    fn add_and_get_round_trip() {
        let mut pa = PartitionedArray::new(2, 4, 1, true);
        pa.allocate(false);
        let id = pa
            .add(|row| {
                row.insert("title".to_string(), Value::String("hello".to_string()));
            })
            .expect("id");
        let row = pa.get(id).expect("row should exist");
        assert_eq!(row.get("title").and_then(Value::as_str), Some("hello"));
    }

    #[test]
    fn save_and_load_keeps_data() {
        let dir = tempdir().expect("tmp dir");
        let mut pa = PartitionedArray::new(2, 4, 1, true);
        pa.allocate(false);
        let id = pa.add(|row| {
            row.insert("id".to_string(), Value::from(7));
            row.insert("name".to_string(), Value::from("wolf"));
        });
        assert!(id.is_some());

        pa.save_to_dir(dir.path(), "spec_db").expect("save");

        let mut loaded = PartitionedArray::new(1, 1, 1, false);
        loaded.load_from_dir(dir.path(), "spec_db").expect("load");
        assert_eq!(
            loaded
                .get(0)
                .and_then(|r| r.get("id"))
                .and_then(Value::as_i64),
            Some(7)
        );
        assert_eq!(
            loaded
                .get(0)
                .and_then(|r| r.get("name"))
                .and_then(Value::as_str),
            Some("wolf")
        );
    }

    #[test]
    fn managed_tracks_latest_id() {
        let mut managed = ManagedPartitionedArray::new(PartitionedArray::new(2, 4, 1, true));
        managed.allocate(false);
        let id0 = managed.add(|row| {
            row.insert("a".to_string(), Value::from(1));
        });
        let id1 = managed.add(|row| {
            row.insert("b".to_string(), Value::from(2));
        });
        assert_eq!(id0, Some(0));
        assert_eq!(id1, Some(1));
        assert_eq!(managed.latest_id, 2);
    }

    #[test]
    fn linedb_add_remove_reload_round_trip() {
        let dir = tempdir().expect("tmp dir");
        let parent = dir.path().join("db_root");
        let db_file = parent.join("db").join("db_list.txt");

        let mut linedb = LineDb::new(
            &parent,
            "db",
            db_file.to_str().expect("utf8 path"),
            LineDbConfig::default(),
        )
        .expect("linedb init");

        assert!(linedb.add_db("alpha").expect("add alpha"));
        assert!(linedb.has_database("alpha"));
        assert!(!linedb.add_db("alpha").expect("dup alpha"));

        assert!(linedb.remove_db("alpha").expect("remove alpha"));
        assert!(!linedb.has_database("alpha"));

        assert!(linedb.add_db("beta").expect("add beta"));
        linedb.reload().expect("reload");
        assert!(linedb.has_database("beta"));
    }

    #[test]
    fn linedb_database_row_persists_after_reload() {
        let dir = tempdir().expect("tmp dir");
        let parent = dir.path().join("db_root");
        let db_file = parent.join("db").join("db_list.txt");

        let mut linedb = LineDb::new(
            &parent,
            "db",
            db_file.to_str().expect("utf8 path"),
            LineDbConfig::default(),
        )
        .expect("linedb init");
        linedb.add_db("posts").expect("add posts");

        let mut row = Row::new();
        row.insert("title".to_string(), Value::from("hello"));
        let inserted_id = linedb
            .db_mut("posts")
            .expect("posts db")
            .add_row(row)
            .expect("insert row");

        linedb.reload().expect("reload");

        let row = linedb
            .db("posts")
            .and_then(|db| db.get_row(inserted_id))
            .expect("row should exist after reload");
        assert_eq!(row.get("title").and_then(Value::as_str), Some("hello"));
    }

    #[test]
    fn linedb_rejects_empty_or_path_like_names() {
        let dir = tempdir().expect("tmp dir");
        let parent = dir.path().join("db_root");
        let db_file = parent.join("db").join("db_list.txt");

        let mut linedb = LineDb::new(
            &parent,
            "db",
            db_file.to_str().expect("utf8 path"),
            LineDbConfig::default(),
        )
        .expect("linedb init");

        assert!(!linedb.add_db("").expect("empty name rejected"));
        assert!(linedb.add_db("../../unsafe").expect("sanitized add"));
        assert!(linedb.has_database("../../unsafe"));
    }

    #[test]
    fn linedb_json_value_helpers_round_trip() {
        let dir = tempdir().expect("tmp dir");
        let parent = dir.path().join("db_root");
        let db_file = parent.join("db").join("db_list.txt");

        let mut linedb = LineDb::new(
            &parent,
            "db",
            db_file.to_str().expect("utf8 path"),
            LineDbConfig::default(),
        )
        .expect("linedb init");

        let payload = serde_json::json!({"hello": "world", "n": 7});
        linedb
            .save_json_value("store", &payload)
            .expect("save json value");

        let loaded: Option<serde_json::Value> =
            linedb.load_json_value("store").expect("load json value");
        assert_eq!(loaded, Some(payload));
    }

    #[test]
    fn rehash_compacts_gaps_and_preserves_non_empty_rows() {
        let mut pa = PartitionedArray::new(2, 4, 1, true);
        pa.allocate(false);
        let id0 = pa.add(|row| {
            row.insert("id".to_string(), Value::from(1));
        });
        let id1 = pa.add(|row| {
            row.insert("id".to_string(), Value::from(2));
        });
        assert_eq!(id0, Some(0));
        assert_eq!(id1, Some(1));

        assert!(pa.delete(0));
        let removed = pa.rehash_compact();
        assert_eq!(removed, 1);
        assert_eq!(
            pa.get(0).and_then(|r| r.get("id")).and_then(Value::as_i64),
            Some(2)
        );
        assert!(pa.get(1).map(|r| r.is_empty()).unwrap_or(false));
    }
}
