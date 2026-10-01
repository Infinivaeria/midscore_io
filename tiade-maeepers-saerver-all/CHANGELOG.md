# Changelog

## [Unreleased]

### Added
- Second Life Ollama job routes `POST /sl/ask/:team` and `GET /sl/job/:id`, so
  replies no longer hit Second Life's 60 s HTTP timeout or get regenerated on retry.
- Token-protected Ruby evaluation for Second Life (`POST /sl/ruby/eval`,
  `POST /sl/ruby/reset`) with per-session bindings, stdout capture and a
  Ruby-level timeout (`src/sl_ruby.rs`).
- `Tiade.Ollama.Ruby.lsl`, an LSL client for both.
- `start.sh` loads an optional git-ignored `server.env`.

### Changed
- Ruby now runs in-process through an embedded Magnus VM (`src/ruby_vm.rs`).
  `/time`, `/ae`, `/weather`, `/tiade/moon`, `/tiade/sun` and the `rustby` CLI
  command no longer write scripts for the external `rustby-vm` watcher or poll
  for result files.
- The Ruby route prelude and `ruby_client/ollama_game_client.rb` are loaded into
  the VM at startup.
- Ruby-backed routes return `500` on a Ruby exception and `504` on timeout.
- `.ruby-version` now matches the installed Ruby (4.0.7).
- Ollama relay: requests send `keep_alive` (default `60m`), the model is
  preloaded at startup, and the installed-model lookup is cached for 60 s.

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
