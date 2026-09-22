# partitioned_array_rust

Rust 1.0 port of the LineDB / PartitionedArray stack, designed as a file-backed array-of-hashes database with chunked partition allocation.

## Status

- Stable: `1.0.0`
- Primary top-level API: `LineDb`
- Persistence model: file-context managed partitioned arrays

## Architecture

The stack mirrors the Ruby layering with `LineDb` as the highest level manager:

- `LineDb`: manages named databases from a line-based database index file (`db_list.txt`)
- `PartitionedArrayDatabase`: per-database wrapper around file-context managed storage
- `FileContextManagedPartitionedArrayManager`: manager for file-context backed managed arrays
- `FileContextManagedPartitionedArray`: on-disk managed partitioned array state
- `ManagedPartitionedArray`: capacity and growth policy layer
- `PartitionedArray`: core partitioned storage engine

## Features

- Partition-aware storage with dynamic growth
- Add/get/set/delete row operations
- Database add/remove/delete/reload operations
- Database-name sanitization for safer filesystem mapping
- Persistence via `metadata.json` plus partition files
- JSON convenience helpers at `LineDb` level
- Rehash/compact operation to move non-empty rows forward

## Quick Start

```rust
use partitioned_array_rust::{LineDb, LineDbConfig, Row};
use serde_json::Value;

let mut ldb = LineDb::new(
    "./example_linedb",
    "db",
    "./example_linedb/db/db_list.txt",
    LineDbConfig::default(),
).unwrap();

ldb.add_db("posts").unwrap();

let mut row = Row::new();
row.insert("title".to_string(), Value::from("hello world"));
row.insert("views".to_string(), Value::from(1));

let id = ldb.add_row("posts", row).unwrap();
let loaded = ldb.get_row("posts", id).unwrap();
assert_eq!(loaded.get("title").and_then(Value::as_str), Some("hello world"));
```

## JSON Store Pattern

For app-state blobs (for example, a blog store struct), use:

- `LineDb::save_json_value(db_name, &value)`
- `LineDb::load_json_value::<T>(db_name)`

This keeps a canonical JSON payload in row `0` under key `value`.

## Migration Notes

- `LineDb` should be the primary public API for application integration.
- Use sanitized names (`a-zA-Z0-9_.-`); other characters are normalized.
- For legacy sparse data, run `rehash_database` to compact rows.

## Testing

```bash
cargo test
```

## Version

At runtime, use `partitioned_array_rust::VERSION`.
