# Second Life Prim Messaging 2.0

## Server Setup

The initial default admin token is `slmessage-admin` when `SL_MESSAGE_ADMIN_TOKEN` is unset and no saved token exists. This is a public development default, not a secure secret. Override it before exposing the server publicly. The admin token is persisted atomically in Rust's `PartitionedArray` at `sl_message_data/objects.admin.json`, alongside the registry (the `SL_MESSAGE_STORE` path with its extension replaced by `admin.json`). An explicit environment token updates the saved token; when the variable is unset, startup reuses the saved token. An explicitly empty token disables admin access and that disabled setting persists. Invalid token storage fails startup. Protect and back up this file: it contains the admin credential in plaintext. Device tokens remain environment-only.

Set two different, randomly generated secrets in the server process environment:

```sh
export SL_MESSAGE_ADMIN_TOKEN="YOUR_LONG_RANDOM_ADMIN_TOKEN"
export SL_MESSAGE_DEVICE_TOKEN="YOUR_DIFFERENT_LONG_RANDOM_DEVICE_TOKEN"
```

Restart the rebuilt server and open `/slmessage` over HTTPS. Enter the admin token to load the object registry. Connect authenticates and saves the accepted admin token in the Rust PartitionedArray snapshot at `SL_MESSAGE_STORE` with its extension replaced by `.admin.json` before loading objects. In the browser, tokens are held only in page memory, not browser storage. The page and template script are public; all object data and changes require authentication. Do not put the admin token in a prim.

The registry is built using Ruby's actual `PartitionedArray` class from `../lib/partitioned_array/lib/partitioned_array.rb`, running in the embedded Ruby VM. Override that library's absolute path with `SL_MESSAGE_RUBY_ARRAY` when deploying elsewhere. The adapter explicitly adds partitions as needed and uses `add_nosave`, leaving atomic disk writes to the server. It does not use Rust's array class for new snapshots. The complete Ruby array snapshot has backend marker `ruby-partitioned-array-v1` and is saved atomically to `sl_message_data/objects.json`, relative to the server working directory. Override with `SL_MESSAGE_STORE`.

Each successful poll or admin edit is persisted before returning success, including identities, per-prim secrets, desired text/color/opacity, revisions and acknowledgments, reported text, latest owner message, timestamps, and programmable state. These records survive server restarts. Existing Rust-array and plain-map registries are automatically migrated at startup without changing prim identities or credentials. Back up the registry before upgrading: older servers cannot read the new Ruby snapshot format. Preserve this file across deployments and restrict its permissions: it contains per-prim secrets. A missing device token or an explicitly empty token disables the corresponding API with HTTP 503; existing server routes still work. Missing storage starts empty; invalid storage fails startup instead of silently discarding data.

## Prim Setup

Use `second_life/sl_message.lsl` or download `/slmessage/script.lsl`. Set `SERVER` to your public HTTPS origin (without a trailing slash) and `DEVICE_TOKEN` to the server's device token. Place the script in every prim whose hover text you want to control. Use a valid publicly trusted TLS certificate reachable from Second Life.

Each prim generates an ID and secret with `llGenerateKey`, saved in linkset data under its actual prim UUID. No manual ID configuration is needed. Resets reuse the identity; rezzed copies with different prim UUIDs get distinct identities. Owner chat shows the generated ID (used as `:id` in the update route) and the actual prim UUID (sent as `object_key`); these are different identifiers. The secret is never printed. Each script controls only the prim containing it. Deleting its linkset data creates a new registry entry on the next reset. Unlinking can also change the available linkset data; keep IDs shown in owner chat for reference.

Multiple standalone objects and multiple prims in a linked object can use the same server and device token. Install one script in each prim to control it independently. There is no fixed prim-count limit; storage partitions grow as new identities register, and names need not be unique. Each identity gets its own dashboard row, hover-text settings, revision, and latest message. Updates target the generated ID, not the object name.

Prims register on their first poll. The dashboard refreshes every five seconds, and prims poll about every 15-18 seconds (with error backoff). The script honors the server's polling interval, bounded to 15-120 seconds plus jitter. Requests are never overlapped. Edits to text, color, and opacity target one ID. Empty text clears hover text. Text is limited to 254 UTF-8 bytes, not 254 Unicode characters. After applying an update, the script immediately polls again to acknowledge the revision; failed acknowledgments retry on subsequent polls. Newer edits replace older pending edits; this is a latest-state service, not a chat queue. Offline edits persist until a prim returns. Online means a poll within the last 60 seconds.

Offline rows offer **Delete object** with confirmation. Deletion removes the entire saved object record, including settings, state and logs, and is persisted before success. The server rechecks offline status under the registry lock and rejects online objects with HTTP 409. This is cleanup, not a ban: if the prim returns, its client can register again with revision `-1`. With polling intervals longer than 60 seconds, an active prim can appear offline between polls.

## From Second Life To The Server

The prim reports its name, region, actual hover text, and applied revision every poll. Owner chat on channel `-934833` becomes its latest object message, for example:

```text
/-934833 Hello from Second Life
```

All nearby prim scripts owned by that avatar can receive that message. It does not change hover text locally. Local hover text changes made by another script appear as reported text; they do not overwrite the server's desired text. Only a new server revision or script reset reapplies desired text. The latest owner message is retained, not a history; the script's local message resets when the script resets.

Use `/-934833 id` to print nearby prim IDs without sending a message. To send a message from only one prim, use `/-934833 @<prim-id> Hello from this prim`, replacing `<prim-id>` with its generated ID. Other prims ignore messages targeted to that ID. Untargeted messages retain the broadcast behavior above.

The 2.0 LSL client also accepts owner-only commands on the same channel:

- `!status`: identity, applied revision, server polling interval, time since the last valid response, HTTP status, and whether a request is pending. `id` remains an alias.
- `!poll`: request an immediate report; if a request is already in flight, queue one follow-up poll instead of overlapping requests.
- `!help`: print the command summary. These commands do not change the latest object message.

Commands also support targeting, for example `/-934833 @<prim-id> !status`. These exact command words are reserved; other text remains a message, limited to 512 UTF-8 bytes. Messages entered during an in-flight request retain latest-state semantics and trigger a follow-up report once a valid response arrives. Intermediate messages are not queued individually. Failed or locally throttled requests retry with backoff. A five-second watchdog checks pending requests against a 60-second timeout independently of the server's 15-120 second polling interval. Failure notices remain limited to the first failure and every tenth subsequent failure, including long outages. Invalid display responses are rejected before changing hover text or acknowledging their revision.

The LSL client never receives the admin token or calls runtime administration routes. Ruby execution and persisted program state remain server-side; device polling and hover-text updates continue while a program is paused or waiting for its wake time. To upgrade an existing prim, replace/reset the script while retaining its linkset data and configured server/device token; its identity will be reused. Rebuild and restart the server to update the downloadable `/slmessage/script.lsl` template.

Run the local protocol checks with `ruby tests/sl_message_lsl_tests.rb`. For LSL syntax and type checking, run `lslint -p second_life/sl_message.lsl` when LSLint is installed. In-world execution still requires compiling and testing the script in Second Life.

## Programmable Ruby Handlers

Ruby already provides general-purpose, Turing-complete computation: conditionals, loops, functions, recursion, and dynamic data structures. SL Message can optionally run a trusted server-local Ruby program on every authenticated prim poll, with independent persisted JSON state per prim. This is resource-bounded programming, not literally infinite computation or storage.

To enable the included example, set this before starting the rebuilt server from the project directory:

```sh
export SL_MESSAGE_HANDLER="$PWD/ruby_client/sl_message_handler_example.rb"
```

The file is read at startup and must evaluate to a callable such as `lambda do |prim| ... end`. Restart to change the program. Unset `SL_MESSAGE_HANDLER` to disable execution without deleting saved state. Programs receive a JSON-compatible hash containing `id`, `object_key`, `name`, `region`, `desired`, `revision`, `applied_revision`, `reported_text`, `message`, `message_at`, `last_seen`, `state`, and `runtime`; the per-prim secret is omitted. New and migrated prims start with `state` equal to `{}`. Version 2.0 also supplies `now` (Unix seconds), `previous_message`, and `event` (`register`, `message`, or `poll`). These describe the current report, not a durable event queue.

Return `{'state' => new_json_state}` or add `'display' => {'text' => 'Hello', 'color' => [1, 1, 1], 'alpha' => 1}`. Version 2.0 also accepts `wake_at` (a nonnegative Unix timestamp) and `logs` (up to 16 strings per run, each at most 512 UTF-8 bytes). Only these four result keys are accepted. The last 64 log strings are retained per prim. Display values have the same limits as admin updates. A changed display increments that prim's revision; an identical or omitted display does not. State, display, logs, scheduling, and run metadata are saved together atomically; exceptions, invalid output, and failed saves do not change the stored prim. Admin registry responses include `state` and `runtime`.

The example increments a counter for each changed message, uses a loop, and remembers the last message to avoid advancing on acknowledgment polls. Handlers should be idempotent for repeated reports: changing hover text on every poll causes the LSL script to keep polling immediately to acknowledge each new revision. Distinct repeated messages are not guaranteed delivery events; this remains a latest-state service, not a queue.

Only install code you trust. Handlers run with the server process's filesystem and network permissions in the shared Ruby VM, not a sandbox. Ruby's two-second handler timeout is best effort, not a hard CPU/memory limit; blocking native calls or hostile code can still disrupt the server. The VM bridge also has a wait timeout, which does not cancel queued or running work. External side effects cannot be rolled back. Do not make callbacks into SL Message while its registry is locked. No HTTP endpoint accepts handler source, and device messages are passed as data, never evaluated as Ruby.

## Runtime Controls And Continuations

Each prim's persisted `runtime` contains `paused`, `poll_seconds` (default 15), `version`, `runs`, `last_run_at`, `wake_at`, and `logs`. Programs run only on authenticated polls while unpaused and once `now >= wake_at`. Omitted `wake_at` permits another run on the next poll. Polling, acknowledgments, reporting, and admin display updates continue while a program is paused or sleeping. There is no background execution: offline prims do not advance. Changes to owner messages during sleep or pause retain latest-state semantics, not queued delivery.

Use the admin-authenticated `GET /slmessage/objects/:id/runtime` to inspect `state`, `runtime`, `handler_enabled`, and `api_version` (`2.0`). `handler_enabled` means server-local handler source is configured, not that it has been tested or is currently running.

Each dashboard row has an expandable **Program runtime** section with a pause checkbox, polling interval, wake-now and clear-log commands, run metadata, program logs, and a JSON state editor. State drafts survive automatic refreshes. Saves use the version loaded with the draft; conflicts retain the draft instead of retrying automatically. **Reload state** confirms before discarding a draft. **Reset state** confirms, resets state to `{}`, and pauses the program; it does not clear logs or reset the run counter. Unsaved display and runtime edits remain independent. Disconnect hides all rows, including when a registry request is still in flight.

Use `POST /slmessage/objects/:id/runtime` with the latest `expected_version`. Every successful handler run and runtime edit increments this version. Stale edits return HTTP 409; reload and deliberately reapply the change rather than retrying blindly. Optional fields are `paused`, `poll_seconds` (15-120), `state` (a non-null JSON replacement), `reset_state` (restore `{}`), `clear_logs`, and `wake_now` (clear a pending wake time). Do not combine `state` with `reset_state`. Resetting state also clears the wake time but preserves run history and display; use `clear_logs` separately. Set `paused` to true when resetting to inspect the initial state before execution resumes.

For example, after a GET returns runtime version 7, pause execution and slow device polling with:

```json
{"expected_version":7,"paused":true,"poll_seconds":60}
```

Then reload the runtime and resume with its new version:

```json
{"expected_version":8,"paused":false,"wake_now":true}
```

The prim receives a changed polling interval on its next successful poll; the server does not push an immediate wake-up. Runtime edits and state replacements are persisted atomically and require the admin token; device tokens cannot inspect or edit runtime controls. No runtime API field accepts executable source.

Longer computations can save an instruction pointer or work cursor in `state` and yield between polls. This Ruby handler computes a sum in chunks of 100, checkpointing the cursor and accumulator so a restart can resume the computation:

```ruby
lambda do |prim|
	state = prim.fetch('state')
	cursor = state.fetch('cursor', 1)
	total = state.fetch('sum', 0)
	if cursor > 10_000
		{'state' => state, 'wake_at' => prim.fetch('now') + 3600}
	else
		next_cursor = [cursor + 100, 10_001].min
		(cursor...next_cursor).each { |value| total += value }
		{'state' => {'cursor' => next_cursor, 'sum' => total},
		 'wake_at' => prim.fetch('now') + 15,
		 'logs' => ["Processed through #{next_cursor - 1}"],
		 'display' => {'text' => "Sum: #{total}", 'color' => [0, 1, 0], 'alpha' => 1}}
	end
end
```

## Local Preview

The isolated preview mounts the real Ruby-backed API and dashboard without starting the other application services. From the project directory, use a temporary registry:

```sh
SL_MESSAGE_STORE="$(mktemp -d)/objects.json" \
SL_MESSAGE_DEVICE_TOKEN="preview-device-token" \
cargo run --example sl_message_preview
```

Open `http://127.0.0.1:8781/slmessage`. Set `SL_MESSAGE_PREVIEW_ADDR` to another loopback address and port if needed. The default admin token applies unless overridden in the environment. The preview device token above is public test data, not a production credential. To exercise Ruby programs, also configure `SL_MESSAGE_HANDLER` as described above. No prims appear until they register; polling and runtime controls use the same authentication and persistence as the main server.

## API

All bodies are JSON, at most 4096 bytes. Responses must not be cached.

- `GET /slmessage` or `/slmessage/`: control page.
- `GET /slmessage/script.lsl`: template script, without credentials.
- `POST /slmessage/connect`: admin-authenticated connection; persists the configured admin token to disk and returns `{"connected":true}`.
- `GET /slmessage/objects`: registry, using `Authorization: Bearer <admin token>`; never returns per-prim secrets.
- `POST /slmessage/objects/:id`: admin update, body `{"text":"Hello","color":[1,1,1],"alpha":1}`.
- `DELETE /slmessage/objects/:id`: admin-only offline object deletion; returns `{"deleted":"<id>"}`, HTTP 409 for online objects, or HTTP 404 for unknown IDs.
- `GET /slmessage/objects/:id/runtime`: admin-only runtime and persisted state inspection.
- `POST /slmessage/objects/:id/runtime`: admin-only, version-checked runtime controls and state replacement.
- `POST /slmessage/poll`: register/report/fetch, using the device bearer token. Body contains `id`, `secret`, `object_key`, `name`, `region`, `applied_revision`, `reported_text`, `color`, `alpha`, and `message`. Initial revision is `-1`. Reply contains `revision`, `text`, `color`, `alpha`, and `poll_seconds`. Existing IDs require their original prim key and secret as well as the shared device token.

The registry has no application-level prim-count cap. Actual capacity is bounded by available memory, disk space, and server throughput, not literally infinite. Every poll and admin update saves a complete registry snapshot, and the dashboard loads the complete registry, so costs increase with registry size. Treat scripts containing the shared device token as trusted: holders can enroll new prims without a count limit. Rotate that token on the server and in every script if exposed. This does not provision independent device enrollment permissions or provide a multi-server shared database. Back up storage before manual maintenance.