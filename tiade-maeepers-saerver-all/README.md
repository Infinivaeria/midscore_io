# tiade-maeepers-saerver-all

Tide-based Rust server that ports the Roda-era behavior and now uses LineDB-backed persistence.

## Version

- 1.0.0

## Highlights

- Roda compatibility routes for blog, gallery, admin, moon/sun, and image resize.
- LineDB integrated as the primary persistence backend.
- Blog and gallery state load/save through LineDB table mapping.
- Gallery image processing pipeline with derivative generation (original, thumbnail, resized).

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
