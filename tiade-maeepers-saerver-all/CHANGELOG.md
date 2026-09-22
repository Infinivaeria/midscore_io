# Changelog

## [1.0.0] - 2026-07-31

### Added
- Integrated `partitioned_array_rust` as primary persistence backend.
- Added operational admin endpoints:
  - `GET /admin/list`
  - `POST /admin/rehash/:db_name`
- Added `README.md` for server usage and endpoint documentation.

### Changed
- Migrated compatibility store load/save in `src/roda_tide_rewrite.rs` to LineDB helper APIs.
- Replaced simulated admin database operations with real LineDB operations.
- Bumped crate version to `1.0.0`.

### Removed
- Removed `partitioned_array_db_addon` dependency from this crate.
