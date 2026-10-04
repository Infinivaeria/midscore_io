# Organizer

Open `/organizer` on the Rust Tide server after rebuilding and restarting it.
The page uses plain HTML, CSS, and JavaScript, with no frontend build step.
The implementation is in [organizer.rs](src/organizer.rs) and
[organizer.html](src/organizer.html), mounted from [main.rs](src/main.rs).

For the local browser preview, run `python3 scripts/organizer_preview.py` while
the Rust server is listening on local HTTPS port 443, then open
`http://127.0.0.1:8766/organizer`. The old `/organizer.html` preview address
redirects there. This proxy forwards both the page and JSON requests to Rust,
so edits use the same persistent data as the running site. Serving the HTML
with a static file server or mocked requests does not persist changes.

- Creating a tag starts its count at zero. Names are unique ignoring case.
- Clicking the large tag button records one occurrence with a server timestamp.
- Saving an entry adds a note under its tag and records exactly one occurrence.
- “Open journal” opens a tag's collection of notes without recording anything.
- Tags have Rename and Delete controls; journal entries have Edit entry and
  Delete entry controls. Renames and edits retain the same IDs, timestamps,
  associations, and occurrence counts. Names must be unique among active tags.
- Delete asks for confirmation and archives the item instead of erasing history.
  Deleted tags leave the active organizer; deleted notes leave active journals
  and recent activity. Their original occurrences still contribute to frequency
  counts and the archive. Overview totals include archived tags; the live table
  lists active tags. Enable “Show archived journals and entries” to read archived
  notes, including journals belonging to deleted tags. Archived items are read only.
  Deletion flags are backward compatible with existing stored records.
- Each tag can contain multiple independent journal entries. Saving a new entry
  appends it without replacing earlier notes. The journal displays ten entries
  per page, newest first; use “Older entries” and “Newer entries” to browse the
  entire collection. Click-only occurrences appear in recent activity, not in
  the journal. Unsaved drafts are retained per tag while the page stays open.
- The frequency table shows all-time totals and the current Pacific calendar year,
  month, week (Monday start), day, hour, minute, and second. These are counts,
  not average rates. Refresh updates the displayed periods and other clients' changes.
- The live table refreshes every five seconds while the tab is visible. Pause
  with the Live checkbox. Search tags and sort by name or any frequency column;
  filtered table totals cover visible tags, while overview cards cover all tags.
  Background updates preserve focus and unsaved journal drafts. Failed refreshes
  keep the last displayed counts, and older background responses cannot replace
  newer writes or filter changes.
- The frequency archive covers every saved click and journal entry, including
  events older than the recent-activity or history limits. Choose a period and
  optional inclusive Pacific dates. It shows all tags (including zero counts),
  with thirty active periods per page; periods without any events are omitted.
  Archive totals cover the whole chosen date range, across all pages. It is
  independent of the live table search and the journal/activity filters.
  The archive is derived from the persistent partitioned-array event records;
  it survives restart without keeping redundant snapshots or expiring history.
- History groups occurrences into the latest 30 selected periods, including zeros.
  Its tag filter also applies to recent activity, which shows the newest 100
  matching clicks and entries. Older records remain stored and counted.

Pacific time uses `America/Los_Angeles`: PST (UTC−8) in winter and PDT (UTC−7)
during daylight saving time. Calendar buckets follow local midnight, including
23- and 25-hour days. Hourly history skips the nonexistent spring-forward hour
and keeps both fall-back hours separate, labeled PDT and PST. Stored timestamps
and API timestamps remain UTC instants; existing records need no migration.

## Storage

The existing local `partitioned_array_rust` dependency stores tags and events as
rows. Its `save_to_dir` / `load_from_dir` methods persist partition files and
metadata. Each mutation writes a new generation, then atomically replaces the
`CURRENT` manifest. Failed writes leave the prior generation and in-memory data
intact; successful requests are acknowledged only after publication. Corrupt
storage causes startup to fail rather than silently discarding records.

By default, data lives in `organizer_data/` under this server's Cargo project
directory, independent of the process working directory. Set `ORGANIZER_DATA_DIR`
to override it. Back up this whole directory while the server is stopped. Do not
point multiple server processes at the same directory; writes are serialized
within one process. An interrupted save may leave an unreferenced generation.

This is a shared organizer for visitors to the server, with no per-user accounts.
It does not use browser storage. Each mutation currently rewrites the partition
snapshot, so storage work increases with the journal's size.

## Single-route protocol

`GET /organizer` serves the page. JSON `POST /organizer` reads or changes data:

```json
{"action":"read"}
{"action":"add_tag","name":"Morning walk"}
{"action":"click","tag_id":"<returned tag id>"}
{"action":"add_entry","tag_id":"<returned tag id>","note":"Walked by the river"}
{"action":"rename_tag","tag_id":"<returned tag id>","name":"Evening walk"}
{"action":"delete_tag","tag_id":"<returned tag id>"}
{"action":"edit_entry","entry_id":"<returned entry id>","note":"Updated journal entry"}
{"action":"delete_entry","entry_id":"<returned entry id>"}
```

Optional query parameters `unit=second|minute|hour|day|week|month|year` (default
`day`) and `tag=<id>` control history and recent activity. Every successful POST
returns the refreshed snapshot. Send `Content-Type: application/json`.
The independent `journal_tag=<id>` parameter selects the journal (default: the
first tag); `journal_page=<zero-based page>` selects a page of ten notes. The
response includes `journal` with `tag_id`, `total`, `page`, `pages`, and `entries`.
Out-of-range page numbers clamp to the last page. Tags include `entry_count`.
Add `include_deleted=true` to allow `journal_tag` to select an archived tag and
to include deleted notes. The journal and its entries expose `deleted` flags.
Archive tag metadata also includes `deleted`; the top-level `tags` list contains
only active tags. Edits and deletions use the same atomic generation commit as
new entries, and reject missing or already-deleted targets.
Archive parameters are `archive_unit` (same units as `unit`, default `day`),
`archive_page` (zero-based), and optional `archive_from` / `archive_to`
(`YYYY-MM-DD`, inclusive Pacific dates). Invalid dates or reversed ranges return
400 before any mutation. The `archive` response contains `tags`, `rows` (each
with `at`, `total`, and counts in tag order), range-wide `totals` and `total`,
and `periods`, `page`, and `pages`. A range may cover only part of a week or month;
its archive count includes only events inside the requested dates.
Requests are limited to 16 KiB, tag names to 80 characters, and notes to 2,000.
Failed network requests are not retried automatically: refresh before repeating
a click, since a disconnected response may still have been saved.

## Validation

From this directory run `cargo test organizer::tests` for persistence, failed-save
recovery, validation, concurrent clicks, Pacific calendar and daylight-saving boundaries, and filtered history.
Then `cargo check` validates integration with the existing server.

On the current nightly toolchain, the existing `rustix` 0.37 dependency needs
its libc backend to avoid compiler-internal attribute errors. The verified test
command is:

```sh
RUSTFLAGS='--cfg rustix_use_libc' CARGO_PROFILE_DEV_OPT_LEVEL=0 cargo test organizer::tests
```
