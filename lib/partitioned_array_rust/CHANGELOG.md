# Changelog

All notable changes to this project are documented in this file.

## [1.0.0] - 2026-07-31

### Added
- Full LineDB-first API in Rust with hierarchical layering:
  - LineDb
  - PartitionedArrayDatabase
  - FileContextManagedPartitionedArrayManager
  - FileContextManagedPartitionedArray
  - ManagedPartitionedArray
  - PartitionedArray
- Database lifecycle management: add, remove, delete, reload, active database tracking.
- Top-level row APIs on LineDb: add/get/set/delete row.
- JSON convenience APIs on LineDb:
  - save_json_value
  - load_json_value
- Rehash/compact support:
  - PartitionedArray::rehash_compact
  - ManagedPartitionedArray::rehash_compact
  - PartitionedArrayDatabase::rehash_compact
  - LineDb::rehash_database
- Database name sanitization and normalization for safer filesystem usage.
- Expanded unit tests for LineDb round-trip persistence, name handling, JSON helpers, and rehash behavior.

### Changed
- Version updated from 0.1.0 to 1.0.0.
- README expanded with architecture, usage, and migration guidance.

### Integration
- Tide server crate wired to LineDb as persistence backend for Roda compatibility state.
