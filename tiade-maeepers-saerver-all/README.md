# tiade-maeepers-saerver-all

Tide-based Rust server that ports the Roda-era behavior and now uses LineDB-backed persistence.

For the full maintainer and operator reference, see
[COMPLETE_DOCUMENTATION.md](COMPLETE_DOCUMENTATION.md). It consolidates setup,
configuration, persistence, HTTP routes, chatlog moderation, Forth/RubyForth,
the Second Life bridge, LSL operation, security, and troubleshooting.

## Version

- 1.0.0

## Highlights

- Roda compatibility routes for blog, gallery, admin, moon/sun, and image resize.
- LineDB integrated as the primary persistence backend.
- Blog and gallery state load/save through LineDB table mapping.
- Gallery image processing pipeline with derivative generation (original, thumbnail, resized).
- Partitioned-array-backed register programs with loop-capable control flow.

## Program Routes

- `GET /program/example` returns a runnable decrementing-loop program.
- `POST /program/run` executes a JSON program against the shared partitioned variable store.

Programs use named integer registers in the existing partitioned array. Supported
operations are `set`, `increment`, `decrement`, `label`, `jump`,
`jump_if_nonzero`, and `halt`. `jump` and `jump_if_nonzero` target a label name;
`set` accepts an integer `value`. Each request may set `max_steps` from 1 through
1,000,000 and returns the final variable snapshot. The per-request limit prevents
an accidental infinite loop from holding the HTTP handler indefinitely.

```json
{
	"max_steps": 100,
	"program": [
		{ "op": "set", "name": "counter", "value": 3 },
		{ "op": "label", "name": "loop" },
		{ "op": "decrement", "name": "counter" },
		{ "op": "jump_if_nonzero", "name": "counter", "target": "loop" },
		{ "op": "halt" }
	]
}
```

## Forth Console And Second Life Bridge

- `GET /forth` returns the machine-readable Forth API catalog.
- `GET /forth/ui` provides a browser console for evaluation, algebra, and bridge jobs.
- `POST /forth/eval` executes Forth/Ruby-like source and returns output, stack state,
  partitioned variables, matrices, and allowlisted Second Life calls.
- `POST /forth/bridge/enqueue` queues source for an in-world bridge object.
- `POST /forth/bridge/poll` delivers one queued source program to that object.

Forth variables, history, notecards, matrices, and pending Second Life bridge
jobs are persisted through `partitioned_array_rust`. The file routes remain a
separate sandboxed UTF-8 file store because they intentionally model file I/O.

To enable the bridge, set the same nonempty token in the server environment and
the `BRIDGE_TOKEN` constant in `src/Relote.Stack.Service.lsl`:

```sh
export MSSL_FORTH_BRIDGE_TOKEN='replace-with-a-long-random-token'
```

After deploying the script, the owner can use `/1111 bridge-on`. The browser
console queues Forth source with that token; the object polls, evaluates it
through the normal runtime, and executes only documented `sl.*` calls. Second
Life permissions, parcel restrictions, and LSL function availability still
apply.

Bridge jobs may include `"language":"forth"` or `"language":"ruby"`; the
browser console provides a separate queue action for each. The LSL poller sends
Ruby jobs through `POST /ruby/eval` and Forth jobs through `POST /forth/eval`.

## RubyForth

`POST /ruby/eval` compiles a Ruby-shaped language into the same Forth runtime,
so variables, matrices, notecards, bridge jobs, and allowed `sl.*` actions use
the partitioned store and the existing execution limits. The browser console at
`GET /forth/ui` includes a `Run RubyForth` command, and the LSL script supports
`/1111 ruby ...`, `/1111 ruby-say <channel> ...`, `/1111 ruby-run <notecard>`,
and `/1111 ruby-run-say <channel> <notecard>`.

Supported syntax includes integer and string expressions, `=`, `+=`, `-=`,
`*=`, `/=`, `puts`, `p`, `if`, `unless`, `elsif`, `else`, `while`, `until`,
`n.times do ... end`, and `end`.
It deliberately does not expose Ruby `eval`, `require`, gems, classes, methods,
or arbitrary subprocess/file access.

```ruby
count = 0
while count < 3
	puts count
	count += 1
end
```

## Matrix Forth

Matrices are stored in the same partitioned variable store as Forth state.
`m.scale <source> <target>` consumes an integer scalar from the stack, while
`m.solve <left> <right> <target>` solves $A X = B$ only when $A$ is invertible
and every result is an exact integer. The runtime rejects fractional or
overflowing results.

```forth
2 2 matrix A
2 0 0 mset A 1 1 1 mset A
2 1 matrix B
4 0 0 mset B 6 1 0 mset B
m.solve A B X
m.show X
```

## Chatlog Moderation Dashboard

- `GET /chatlog` renders the full moderation dashboard: parses the raw Second Life
  chat log, scores each message against `hostile`/`positive`/`drug`/`slang` word
  dictionaries, and reports integrity, timing, and capture-source statistics.
- `GET /chatlog?format=summary` returns the same statistics as JSON instead of HTML.
- `GET /chatlog?format=recent` renders only the most recent messages.
- `GET /chatlog/summary`, `GET /chatlog/recent`, and `GET /chatlog/admin` are
  convenience redirects to the `format` variants above.
- `GET /chatlog/markov` and `GET /chatlog/markov.json` generate Markov-chain
  sentence samples from the logged chat; `GET /chatlog/markov/transitions.json`
  exposes the underlying state-transition table.
- `GET /avatarencounter` opens the timer-driven nearby-avatar frequency HTML
	dashboard; `GET /avatarencounter?format=json` exposes the underlying table,
	including UUID/name pairs, probabilities, encounter rates, count deltas, and
	per-source scan statistics. The `/chatlog` dashboard links out to this view;
	LSL scanners upload snapshots with `POST /avatarfrequency`.

### Custom Alert Words

The built-in `hostile`, `positive`, `drug`, and `slang` dictionaries can be
extended at runtime, without a rebuild, from `/chatlog`:

- `GET /chatlog/words` is a browser console for adding, scoring, and removing words.
- `GET /chatlog/words/list` returns the current custom words as JSON.
- `POST /chatlog/words/add` with `{"category":"hostile","word":"example","score":2}`
  upserts a word (`score` defaults to `2`, clamped to `-10..=10`).
- `POST /chatlog/words/delete` with `{"category":"hostile","word":"example"}` removes it.

Words are limited to 1-32 ASCII letters/numbers, persisted through
`partitioned_array_rust` alongside the other memory stores, and merged into the
matching dictionary every time `/chatlog` scores a message.

## Persistence

This server uses the `partitioned_array_rust` crate with `LineDb` as top-level manager.

Default LineDB root:
- `/root/midscore_io/logs/tiade_roda_compat/line_db`

Fallback LineDB root:
- `/tmp/tiade_line_db`

State table mapping uses file stem naming:
- Blog store path -> `blog_store`
- Gallery store path -> `gallery_store`

The Ollama 2.0 relay uses `LineDb` for team conversation history, team prompts, and isolated game-player histories:
- Store root: `/root/midscore_io/logs/ollama_teams/line_db`
- History is retained to the newest 500 entries per team by default.

## Admin Endpoints

- `GET /admin/login?password=...`
- `POST /admin/add` with form field `db_name`
- `GET /admin/remove/:db_name`
- `POST /admin/delete` with form field `db_name`
- `GET /admin/reload`
- `GET /admin/list`
- `POST /admin/rehash/:db_name`

## Route Matrix

Integration note:
- `src/main.rs` mounts `roda_tide_rewrite::mount_roda_compat_routes(&mut app)` last, so overlapping paths resolve to the Roda-compat handlers.
- `/` redirects to `/gallery`.
- Legacy Tiade overlap routes were namespaced to `/tiade/*`.

### Core and Static (Roda Compat)

| Method | Path | Source | Auth | Notes |
|---|---|---|---|---|
| GET | `/` | `src/roda_tide_rewrite.rs` | public | Redirects to `/gallery` |
| GET | `/assets/*path` | `src/roda_tide_rewrite.rs` | public | Serves project assets |
| GET | `/public/*path` | `src/roda_tide_rewrite.rs` | public | Serves public files |
| GET | `/card` | `src/roda_tide_rewrite.rs` | public | JPEG banner |
| GET | `/moon` | `src/roda_tide_rewrite.rs` | public | Text response |
| GET | `/sun` | `src/roda_tide_rewrite.rs` | public | Text response |
| POST | `/img/resize` | `src/roda_tide_rewrite.rs` | public | Image bytes resize endpoint |

### Admin (Roda Compat)

| Method | Path | Source | Auth | Notes |
|---|---|---|---|---|
| GET | `/admin/login` | `src/roda_tide_rewrite.rs` | public | `?password=` sets admin cookie on success |
| GET | `/admin` | `src/roda_tide_rewrite.rs` | admin cookie | Admin HTML dashboard |
| POST | `/admin/add` | `src/roda_tide_rewrite.rs` | admin cookie | Form field `db_name` |
| GET | `/admin/remove/:db_name` | `src/roda_tide_rewrite.rs` | admin cookie | Redirects back to admin |
| POST | `/admin/delete` | `src/roda_tide_rewrite.rs` | admin cookie | Form field `db_name` |
| GET | `/admin/reload` | `src/roda_tide_rewrite.rs` | admin cookie | Reload LineDB state |
| GET | `/admin/list` | `src/roda_tide_rewrite.rs` | admin cookie | JSON database listing |
| POST | `/admin/rehash/:db_name` | `src/roda_tide_rewrite.rs` | admin cookie | JSON rehash result |

### Blog (Roda Compat)

| Method | Path | Source | Auth | Notes |
|---|---|---|---|---|
| GET | `/blog` | `src/roda_tide_rewrite.rs` | public | Redirects to `/blog/login` |
| GET | `/blog/login` | `src/roda_tide_rewrite.rs` | public | HTML login form |
| POST | `/blog/login` | `src/roda_tide_rewrite.rs` | public | Requires `blog_user_name`, `blog_password_name`, `super_password` |
| GET | `/blog/logout` | `src/roda_tide_rewrite.rs` | session | Clears blog session cookie |
| GET | `/blog/signup` | `src/roda_tide_rewrite.rs` | public | HTML signup form |
| POST | `/blog/signup` | `src/roda_tide_rewrite.rs` | public | Creates user |
| GET | `/blog/render` | `src/roda_tide_rewrite.rs` | public | Render by query (`user`, `id`) |
| GET | `/blog/:user` | `src/roda_tide_rewrite.rs` | public | Redirects to `/blog/:user/view` |
| GET, POST | `/blog/:user/pin` | `src/roda_tide_rewrite.rs` | owner session | View or set pinned post |
| GET | `/blog/:user/tag/:tag` | `src/roda_tide_rewrite.rs` | public/private by profile | Tag filtered listing |
| GET, POST | `/blog/:user/edit/:id` | `src/roda_tide_rewrite.rs` | owner session | Edit post page and save |
| GET | `/blog/:user/delete` | `src/roda_tide_rewrite.rs` | public/private by profile | Delete/lock listing page |
| GET | `/blog/:user/delete/:id` | `src/roda_tide_rewrite.rs` | owner session | Toggle lock then redirect |
| GET | `/blog/:user/list` | `src/roda_tide_rewrite.rs` | public/private by profile | Post list |
| GET | `/blog/:user/private_toggle` | `src/roda_tide_rewrite.rs` | owner session | Toggle private view |
| GET | `/blog/:user/view` | `src/roda_tide_rewrite.rs` | public/private by profile | Main blog index |
| GET | `/blog/:user/view/:id` | `src/roda_tide_rewrite.rs` | public/private by profile | Post view (`?format=json` supported) |
| GET | `/blog/:user/view/:month/:day/:year/:time` | `src/roda_tide_rewrite.rs` | public/private by profile | Date-based post lookup |

### Gallery (Roda Compat)

| Method | Path | Source | Auth | Notes |
|---|---|---|---|---|
| GET | `/gallery` | `src/roda_tide_rewrite.rs` | public | Gallery home/user list |
| GET, POST | `/gallery/upload/url` | `src/roda_tide_rewrite.rs` | public | URL upload page and submit |
| GET, POST | `/gallery/upload` | `src/roda_tide_rewrite.rs` | public | Raw body or multipart file/url upload |
| GET | `/gallery/view/:user/latest` | `src/roda_tide_rewrite.rs` | public | Redirects to latest page index |
| GET | `/gallery/reset_session/:user` | `src/roda_tide_rewrite.rs` | public | Clears gallery preference cookies |
| GET | `/gallery/view/:user` | `src/roda_tide_rewrite.rs` | public/private by profile | Gallery index with cookie-backed prefs |
| GET | `/gallery/view/:user/id/:id` | `src/roda_tide_rewrite.rs` | public/private by profile | Single gallery item |
| GET | `/gallery/view/:user/id/:id/attachments` | `src/roda_tide_rewrite.rs` | public/private by profile | Attachment list |
| GET | `/gallery/view/:user/id/:id/attachments/delete/:attachment_id` | `src/roda_tide_rewrite.rs` | public/private by profile | Deletes attachment entry |
| GET, POST | `/gallery/view/:user/id/:id/attachments/upload` | `src/roda_tide_rewrite.rs` | public/private by profile | Form + multipart/url/value upload |
| GET | `/gallery/delete/:user/id/:id` | `src/roda_tide_rewrite.rs` | public/private by profile | Deletes post and files |
| GET | `/gallery/view/:user/tags/search` | `src/roda_tide_rewrite.rs` | public/private by profile | Include/exclude search (`search_tags`, `--tag`) |
| GET | `/gallery/view/:user/tags` | `src/roda_tide_rewrite.rs` | public/private by profile | Tag list page |
| GET, POST | `/gallery/edit/:user/id/:id` | `src/roda_tide_rewrite.rs` | public/private by profile | Edit gallery metadata |

### Gallery UWU/OWO (Roda Compat)

| Method | Path | Source | Auth | Notes |
|---|---|---|---|---|
| GET | `/gallery/uwu/view/:user` | `src/roda_tide_rewrite.rs` | public/private by profile | List collections |
| GET | `/gallery/uwu/view/:user/id/:id` | `src/roda_tide_rewrite.rs` | public/private by profile | View collection |
| GET | `/gallery/uwu/delete/id/:id` | `src/roda_tide_rewrite.rs` | session-dependent | Delete collection |
| GET, POST | `/gallery/uwu/new` | `src/roda_tide_rewrite.rs` | session-dependent | Create collection |
| POST | `/gallery/uwu/edit/id/:id` | `src/roda_tide_rewrite.rs` | session-dependent | Replace collection items |
| GET | `/gallery/uwu/delete_image/uwu_id/:uwu_id/gallery_id/:gallery_id` | `src/roda_tide_rewrite.rs` | session-dependent | Remove image from collection |
| POST | `/gallery/uwu/add_image/uwu_id/:uwu_id` | `src/roda_tide_rewrite.rs` | session-dependent | Add image to collection |
| GET | `/gallery/owo/add` | `src/roda_tide_rewrite.rs` | public | Increment counter |
| GET | `/gallery/owo/rem` | `src/roda_tide_rewrite.rs` | public | Decrement counter |
| GET | `/gallery/owo/sub` | `src/roda_tide_rewrite.rs` | public | Read counter |

### Tiade Main Server Routes (Legacy/Utility)

| Method | Path | Source | Notes |
|---|---|---|---|
| POST | `/praexy-saerver` | `src/main.rs` | Form relay utility |
| GET | `/bridge/*rest` | `src/main.rs` | iframe bridge page |
| GET | `/time` | `src/main.rs` | Ruby script output |
| GET | `/ae` | `src/main.rs` | Ruby script output |
| GET | `/weather` | `src/main.rs` | Ruby script output |
| GET | `/rneutrialg` | `src/main.rs` | Text file read |
| GET | `/rneutri` | `src/main.rs` | Text file write |
| GET | `/tiade/moon` | `src/main.rs` | Namespaced legacy moon endpoint |
| GET | `/tiade/sun` | `src/main.rs` | Namespaced legacy sun endpoint |
| GET | `/tiade-maepers/*rest` | `src/main.rs` | iframe bridge page |
| GET | `/parse_plink` | `src/main.rs` | URL parser redirect |
| POST | `/tiade/img/resize` | `src/main.rs` | Namespaced legacy placeholder resize |
| GET | `/` | `src/main.rs` | Redirects to `/gallery` |
| POST | `/echo` | `src/main.rs` | Echo body |
| POST | `/restart-servers` | `src/main.rs` | Process HUP command |
| POST | `/file/add` | `src/main.rs` | Writes `/tmp/new_file.txt` |
| DELETE | `/file/delete` | `src/main.rs` | Deletes `/tmp/new_file.txt` |

### Chatlog Moderation Dashboard (main.rs)

| Method | Path | Source | Notes |
|---|---|---|---|
| GET | `/chatlog` | `src/main.rs` | Dashboard HTML; `?format=summary` for JSON stats, `?format=recent` for latest-only |
| GET | `/chatlog/summary` | `src/main.rs` | Redirects to `/chatlog?format=summary` |
| GET | `/chatlog/recent` | `src/main.rs` | Redirects to `/chatlog?format=recent` |
| GET | `/chatlog/admin` | `src/main.rs` | Redirects to `/chatlog?format=summary` |
| GET | `/chatlog/markov` | `src/main.rs` | Markov sentence generator HTML console |
| GET | `/chatlog/markov.json` | `src/main.rs` | Markov sentence generator JSON |
| GET | `/chatlog/markov/transitions.json` | `src/main.rs` | Markov state-transition table JSON |
| GET | `/chatlog/words` | `src/main.rs` | Custom alert word console (HTML) |
| GET | `/chatlog/words/list` | `src/main.rs` | Lists custom alert words (JSON) |
| POST | `/chatlog/words/add` | `src/main.rs` | Upserts `{category, word, score}` |
| POST | `/chatlog/words/delete` | `src/main.rs` | Removes `{category, word}` |

### Mounted Relay Routes (tiade_ollama_relay)

These are mounted by `mount_ollama_routes(&mut app, OllamaRelayConfig::default())`:

- `GET /ollama` (general route catalog)
- `POST /ollama` with Ollama-compatible `{"model":"...","messages":[...],"stream":false}`
- `POST /chat/:team` with `{"message":"..."}`
- `POST /game/:team/:player` with `{"message":"...","game_prompt":"..."}`
- `POST /game/:team/:player/turn` with `{"action":"...","game_prompt":"...","state":{}}`
- `GET /game/:team/:player/state`
- `POST /game/:team/:player/reset`
- `GET /history/:team`
- `GET /ollama/health`
- `GET /teams/:team/prompt`
- `POST /teams/:team/prompt` with `{"prompt":"..."}`

### Team Prompts

Set `OLLAMA_TEAM_PROMPT_TOKEN` before starting the server to enable prompt management. A saved prompt is applied as a system message only to the matching team. An empty prompt clears the saved team prompt.

Read the prompt with `GET /teams/:team/prompt` and the same bearer token.

```bash
export OLLAMA_TEAM_PROMPT_TOKEN='set-a-long-random-token-here'
curl -X POST https://your-host/teams/research/prompt \
	-H "Authorization: Bearer $OLLAMA_TEAM_PROMPT_TOKEN" \
	-H 'Content-Type: application/json' \
	-d '{"prompt":"Answer as a concise research assistant."}'
```

### JavaScript Clients

The relay sends permissive CORS headers and accepts `OPTIONS` preflight requests for chat, game, history, health, and team-prompt routes.

Call `GET /ollama` to discover the generic Ollama route catalog, including methods and authorization requirements.

For conversational game I/O, `POST /game/:team/:player` isolates history by player. For model-directed play, `POST /game/:team/:player/turn` persists a JSON state object per player and requires Ollama to return a JSON directive containing `narrative`, the complete next `state`, `choices`, and `game_over`. The saved team prompt is applied first; the optional `game_prompt` is applied only to that request, letting the game provide current rules or scene context.

```js
const response = await fetch('https://your-host/chat/research', {
	method: 'POST',
	headers: { 'Content-Type': 'application/json' },
	body: JSON.stringify({ message: 'Summarize today\'s findings.' }),
});
const { response: reply } = await response.json();
```

### WebAssembly Client

`wasm_client` compiles to a browser ES module and is served by the existing static-assets route. Run `./start-with-wasm.sh` to install the Rust WebAssembly target and `wasm-bindgen-cli` on first use, build both artifacts, and start the release server.

```js
import init, { chat, game_turn } from '/assets/wasm/ollama_game_client.js';

await init();
const reply = await game_turn(
	window.location.origin,
	'arcade',
	'player-42',
	'I open the north door.',
	'Inventory: lantern, silver key.'
);
console.log(reply);
```

The generated module exports `routes`, `health`, `chat`, `game_turn`, and `history`. The relay route list is also available in [OLLAMA_ROUTES.txt](OLLAMA_ROUTES.txt).

### Ruby Raylib and Magnus Client

[ruby_client/ollama_game_client.rb](ruby_client/ollama_game_client.rb) is dependency-free Ruby using `Net::HTTP` and `JSON`, so it can run in a raylib-ruby game loop or in Ruby evaluated through Magnus. [ruby_client/raylib_game_loop_example.rb](ruby_client/raylib_game_loop_example.rb) shows a `GameDirector` that retains rendering, input, physics, and validation in Ruby while consuming Ollama's structured turn directives as data.

At server startup, `src/main.rs` loads this same client into the embedded Magnus VM. Ruby evaluated through that VM can instantiate `OllamaGameClient::Client` directly; no Ruby load-path setup or duplicated HTTP bridge is required.

```ruby
require_relative "ruby_client/ollama_game_client"

client = OllamaGameClient::Client.new(
	server_url: "https://your-host",
	team: "arcade",
	player: "player-42"
)
turn = client.turn(action: "open the north door", game_prompt: "Fantasy dungeon. Keep choices concise.")
puts turn.fetch("directive").fetch("narrative")
```

```js
const response = await fetch('https://your-host/game/arcade/player-42', {
	method: 'POST',
	headers: { 'Content-Type': 'application/json' },
	body: JSON.stringify({
		message: 'I open the north door.',
		game_prompt: 'Player inventory: lantern, silver key. Return concise JSON-ready prose.',
	}),
});
const { response: reply } = await response.json();
```

## Run

```bash
cargo run
```

For the TLS production server, build and start the release binary:

```bash
cargo build --release
./start.sh
```

To build and serve the browser WebAssembly client with the server instead:

```bash
./start-with-wasm.sh
```

Use `./stop-server.sh` to send the managed process `SIGTERM`.

## Verify

```bash
cargo check
```

## Notes

`src/main.rs` currently contains pre-existing warnings unrelated to the LineDB migration layer in `src/roda_tide_rewrite.rs`.
