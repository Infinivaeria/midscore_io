# Tide Route Manual

This manual describes the Tide routes exposed by `tiade-maeepers-saerver-all`.
The language runtime is bounded by a request step limit, but supports persistent
state, loops, arrays, hashes, mixed Forth/RubyForth execution, named words,
Second Life calls, and isolated sessions.

## Conventions

- Base URL: `https://your-server.example`
- JSON POST routes require `Content-Type: application/json`.
- Missing `session_id` uses the legacy `default` session.
- Session IDs use 1-64 ASCII letters, digits, `_`, or `-`.
- LSL defaults to the current prim UUID as its session.
- `max_steps` defaults to 10,000 and may be set from 1 through 1,000,000.

## Language Routes

| Method | Route | Purpose |
| --- | --- | --- |
| GET | `/forth` | Machine-readable language and route catalog. |
| GET | `/forth/ui` | Browser console for Forth, RubyForth, algebra, and bridge jobs. |
| GET | `/ruby` | RubyForth capability description. |
| POST | `/forth/eval` | Execute Forth. Body: `source`, optional `max_steps`, optional `session_id`. |
| POST | `/ruby/eval` | Compile and execute RubyForth. Body: `source`, optional `max_steps`, optional `session_id`. |
| POST | `/sessions/:session_id/forth/eval` | Execute Forth in the path-selected session. |
| POST | `/sessions/:session_id/ruby/eval` | Execute RubyForth in the path-selected session. |
| GET | `/sessions/:session_id/status` | Return values visible in one session. |
| POST | `/program/run` | Run the lower-level JSON register VM. |
| GET | `/program/example` | Return a register-program example. |
| POST | `/forth/algebra` | Simplify, differentiate, integrate, and optionally evaluate an integer polynomial. |
| GET | `/forth/example` | Return a Forth countdown example. |

Example:

```sh
curl -sS https://your-server.example/sessions/workshop_1/ruby/eval \
  -H 'Content-Type: application/json' \
  --data '{"source":"count = 2; puts count; count += 3; puts count"}'
```

RubyForth and Forth share the selected session. RubyForth supports semicolon
statement separation and multiline Forth blocks:

```ruby
count = 2; puts count
forth do
  $count 3 + count=
  $count puts
end
puts count
```

## Variable Routes

These are compatibility routes used by the LSL variable commands.

| Method | Route | Body | Purpose |
| --- | --- | --- | --- |
| POST | `/vars/set` | JSON values | Set variables. |
| POST | `/vars/get` | `{"name":"key"}` | Read a variable. |
| POST | `/vars/view` | optional action | View variables. |
| POST | `/vars/delete` | `{"name":"key"}` | Delete a variable. |
| POST | `/vars/clear` | optional action | Clear variables. |
| POST | `/vars/history` | optional action | Read variable history. |
| POST | `/vars/status` | optional action | Read store status. |

For new code, prefer `/sessions/:session_id/...` language routes so the session
is explicit and cannot be confused with a normal variable name.

## Notecards, Matrices, and Files

### Notecards

| Method | Route | Purpose |
| --- | --- | --- |
| POST | `/forth/notecards/save` | Save `name`, `source`, and optional `session_id`. |
| POST | `/forth/notecards/get` | Read a named notecard in the selected session. |
| POST | `/forth/notecards/list` | List notecards in the selected session. |
| POST | `/forth/notecards/run` | Run a saved Forth notecard in the selected session. |
| POST | `/ruby/notecards/run` | Compile and run a saved RubyForth notecard in the selected session. |
| POST | `/forth/notecards/delete` | Delete a saved notecard in the selected session. |

### Matrices

| Method | Route | Purpose |
| --- | --- | --- |
| POST | `/forth/matrices/get` | Read a named persisted matrix. |

Matrices are created and changed by Forth words such as `matrix`, `mset`,
`m.scale`, `m.solve`, `m.transpose`, and `m.show`.
Ordinary Ruby sent to `POST /sl/ruby/eval` can read the same session's matrix
with `matrix_get("A")` (a hash with `rows`, `cols`, and row-major `values`).

### Sandboxed files

| Method | Route | Purpose |
| --- | --- | --- |
| POST | `/forth/files/write` | Write a UTF-8 file with `name`, `content`, and optional `session_id`. |
| POST | `/forth/files/read` | Read a named file in the selected session. |
| POST | `/forth/files/list` | List files in the selected session. |
| POST | `/forth/files/delete` | Delete a file in the selected session. |

File names are safe basenames. Files are separate from the partitioned-array
language state and are not automatically executable.
Ordinary Ruby sent to `POST /sl/ruby/eval` can access the same session's
files with `file_write(name, content)`, `file_read(name)`, `file_list`, and
`file_delete(name)`. Its `var_set(name, value)`, `var_get(name)`,
`var_view`, and `var_delete(name)` methods use the same session-backed
variable store as `/vars/*`. See [README.md](README.md#second-life-ollama-and-ruby)
for a Ruby example and the route's authentication requirements.

## Second Life Bridge

| Method | Route | Purpose |
| --- | --- | --- |
| POST | `/forth/bridge/enqueue` | Queue Forth or RubyForth using `token`, `source`, optional `language`, optional `max_steps`, and optional `session_id`. |
| POST | `/forth/bridge/poll` | Authenticated LSL poll; returns one matching queued job. |
| POST | `/sl_logger` | Store incoming Second Life log data in the Rust `partitioned_array_rust` chatlog array and immediately persist the full memory snapshot to disk. |
| POST | `/avatarfrequency` | Store a cumulative nearby-avatar scan snapshot containing UUIDs, names, encounter counts, scan totals, and timing metadata. |
| GET | `/avatarencounter` | Render the live avatar-encounter HTML dashboard. Add `?format=json` for probabilities, encounter rates, count deltas, and per-source scan statistics. |

Set `MSSL_FORTH_BRIDGE_TOKEN` on the server and the same `BRIDGE_TOKEN` in the
LSL object. Use `/1111 bridge-on` to start polling. The LSL client polls
immediately, then on its timer interval, and prevents overlapping polls with
`BRIDGE_POLL_IN_FLIGHT`.

### Chatlog durability

`/sl_logger` is the write route. Its sequence is:

1. Read the request body.
2. Append a row to `ChatlogStore.entries`, which is a
  `partitioned_array_rust::PartitionedArray`.
3. Increment the chatlog revision.
4. Release the chatlog mutex.
5. Clone the chatlog, variable, and bridge partitioned arrays into the durable
  JSON snapshot configured by `MSSL_MEMORY_STORE_PATH`.
6. Atomically replace the snapshot file through a temporary file and rename.
7. Return success only after the disk write succeeds.

`/avatarfrequency` uses a separate persisted partitioned array. The LSL scanner
posts a cumulative snapshot on every timer scan, so the read route can derive
current probabilities, encounter rates, count deltas, and source-level scan
statistics without mixing presence observations into chat moderation records.

`/chatlog` is read-only. It reads the current in-memory partitioned array and
may cache rendered dashboard output, but it does not create a second log store.
On a clean shutdown, the same full snapshot is written again by the shutdown
handler. On an unexpected process or machine failure, the most recent completed
`/sl_logger` request is the durable boundary.

For a persistence test, set a temporary path, post one log, stop the server
cleanly, start it again with the same path, and request `/chatlog?format=recent`.
The posted entry should still be present.

A bridge job has three stages: queued, delivered, and executed. Delivery removes
the job from the queue. If execution fails afterward, this LSL client does not
requeue it. Use a session-specific, idempotent program for retryable work.

## Static and Utility Routes

| Route | Purpose |
| --- | --- |
| `/` | Main site entry or redirect. |
| `/ae`, `/time`, `/weather` | Legacy utility output. |
| `/analytics` | Analytics output. |
| `/bridge/*rest` | Browser bridge pages. |
| `/chatlog` | Chat-log viewing. |
| `/css`, `/fonts`, `/img`, `/js`, `/public` | Static assets. |
| `/echo` | Echo a request body. |
| `/file/add`, `/file/delete` | Legacy temporary-file utilities. |
| `/flashcard` | Flashcard utility. |
| `/paema` | Legacy PAEMA handler. |
| `/parse_plink` | URL parsing utility. |
| `/praexy-saerver` | Form relay. |
| `/random`, `/random2` | Random utilities. |
| `/restart-servers` | Persist and signal restart; protect at the proxy layer. |
| `/rneutri`, `/rneutrialg` | Legacy text/file routes. |
| `/rustby` | Rustby integration. |
| `/sigil-deck` and `/sigil-deck/*` | Sigil deck cards, uploads, images, and random selection. |
| `/tiade/img/resize`, `/tiade/moon`, `/tiade/sun` | Namespaced legacy Tiade utilities. |
| `/tiade-maepers/*rest` | Legacy iframe bridge pages. |

Sigil deck subroutes include `/sigil-deck/card/:id`, `/sigil-deck/card/:id/update`,
`/sigil-deck/card/:id/delete`, `/sigil-deck/image/:filename`,
`/sigil-deck/random`, and `/sigil-deck/upload`.

## Mounted Compatibility Families

The server also mounts larger compatibility families documented in `README.md`:

- Admin: `/admin/login`, `/admin`, `/admin/add`, `/admin/remove/:db_name`,
  `/admin/delete`, `/admin/reload`, `/admin/list`, `/admin/rehash/:db_name`.
- Blog: `/blog/login`, `/blog/signup`, `/blog/logout`, and `/blog/:user/*`.
- Gallery: `/gallery`, upload routes, and `/gallery/view/:user/*`.
- Gallery collections: `/gallery/uwu/*` and `/gallery/owo/*`.
- Ollama relay: `/ollama`, `/ollama/health`, `/chat/:team`, `/game/:team/:player`,
  `/history/:team`, and `/teams/:team/prompt`.

## LSL Quick Reference

```text
/1111 help
/1111 channel 0
/1111 session workshop_1
/1111 ruby count = 1; puts count; count += 1; puts count
/1111 forth 2 3 + puts; : square dup * ; 4 square puts;
/1111 bridge-on
/1111 bridge-off
/1111 session-reset
```

For the internal request ledger, asynchronous HTTP lifecycle, notecard loader,
and detailed bridge polling behavior, see [LSL_CLIENT_REFERENCE.md](LSL_CLIENT_REFERENCE.md).

## Battle-Test Checklist

Run these checks against a temporary server or maintenance session before using
production data.

### Language and session isolation

```sh
BASE=https://your-server.example
curl -sS "$BASE/sessions/alpha/forth/eval" -H 'Content-Type: application/json' \
  --data '{"source":"7 counter=; $counter puts"}'
curl -sS "$BASE/sessions/beta/forth/eval" -H 'Content-Type: application/json' \
  --data '{"source":"$counter puts"}'
curl -sS "$BASE/sessions/alpha/status"
```

Expected behavior: Alpha reports `7`, Beta reports `0`, and Alpha status
contains only `counter: 7`.

### Variable, file, and notecard isolation

```sh
curl -sS "$BASE/vars/set" -H 'Content-Type: application/json' \
  --data '{"session_id":"alpha","note":"alpha-value"}'
curl -sS "$BASE/vars/get" -H 'Content-Type: application/json' \
  --data '{"session_id":"beta","name":"note"}'

curl -sS "$BASE/forth/files/write" -H 'Content-Type: application/json' \
  --data '{"session_id":"alpha","name":"shared.txt","content":"alpha-file"}'
curl -sS "$BASE/forth/files/read" -H 'Content-Type: application/json' \
  --data '{"session_id":"beta","name":"shared.txt"}'

curl -sS "$BASE/forth/notecards/save" -H 'Content-Type: application/json' \
  --data '{"session_id":"alpha","name":"CardA","source":"7 puts"}'
curl -sS "$BASE/forth/notecards/get" -H 'Content-Type: application/json' \
  --data '{"session_id":"beta","name":"CardA"}'
```

The Beta variable, file, and notecard reads should each return `404`.

### Bridge isolation

```sh
TOKEN=battle-token
curl -sS "$BASE/forth/bridge/enqueue" -H 'Content-Type: application/json' \
  --data "{\"token\":\"$TOKEN\",\"session_id\":\"alpha\",\"language\":\"forth\",\"source\":\"0 \\\"alpha job\\\" sl.owner_say\"}"
curl -sS "$BASE/forth/bridge/enqueue" -H 'Content-Type: application/json' \
  --data "{\"token\":\"$TOKEN\",\"session_id\":\"beta\",\"language\":\"forth\",\"source\":\"0 \\\"beta job\\\" sl.owner_say\"}"
curl -sS "$BASE/forth/bridge/poll" -H 'Content-Type: application/json' \
  --data "{\"token\":\"$TOKEN\",\"session_id\":\"beta\"}"
curl -sS "$BASE/forth/bridge/poll" -H 'Content-Type: application/json' \
  --data "{\"token\":\"$TOKEN\",\"session_id\":\"alpha\"}"
```

The Beta poll must return the Beta source, and the Alpha poll must return the
Alpha source. An empty matching session returns `message: null`.
