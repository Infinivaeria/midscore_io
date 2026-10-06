# Ruby, LSL, and Magnus: complete command and usage reference

This guide covers the finite interfaces of the **Tiade.Ollama.Ruby** Second Life client: every controller command and alias, all notecard directives, the nine Ruby storage methods, the Ruby game client, and the HTTP interfaces used to connect them. It also explains the separate RubyForth compiler. General Ruby is a full programming language: there is no finite list of every Ruby program, library method, or possible usage. The recipes below demonstrate how to combine the project's finite APIs with ordinary Ruby.

The implementation references are [the combined controller and listeners](Tiade.Ollama.Ruby.lsl), [Help](Tiade.Ollama.Ruby.Help.lsl), [the Magnus routes and Ruby storage prelude](src/sl_ruby.rs), [the VM worker](src/ruby_vm.rs), [the Rust storage callbacks and RubyForth compiler](src/main.rs), [the Ruby game client](ruby_client/ollama_game_client.rb), and [the Ollama relay](tiade_ollama_relay/src/lib.rs). The controller ends with a complete commented command reference; `/7 help` and `/7 help all` list every command in-world, with eight detailed topic pages available separately.

## Contents

- [Which Ruby system am I using?](#which-ruby-system-am-i-using)
- [Setup and configuration](#setup-and-configuration)
- [Every controller command and alias](#every-controller-command-and-alias)
- [Notecards and their directives](#notecards-and-their-directives)
- [Sessions, resets, and persistence](#sessions-resets-and-persistence)
- [All nine Ruby storage methods](#all-nine-ruby-storage-methods)
- [Typed values and ordinary Ruby recipes](#typed-values-and-ordinary-ruby-recipes)
- [Local chat: capture, process, explicitly save](#local-chat-capture-process-explicitly-save)
- [Sharing variables, matrices, and files](#sharing-variables-matrices-and-files)
- [HTTP protocols](#http-protocols)
- [Ruby Ollama game client](#ruby-ollama-game-client)
- [RubyForth mini compiler](#rubyforth-mini-compiler)
- [Server console and other Magnus routes](#server-console-and-other-magnus-routes)
- [Troubleshooting](#troubleshooting)

## Which Ruby system am I using?

| Interface | Executes | Request / result | State |
| --- | --- | --- | --- |
| `/7 ruby ...`, Ruby-only inventory notecards, `POST /sl/ruby/eval` | Real Ruby embedded through Magnus | `code`, `session`, `include_result`; plain-text output | Ruby session binding plus explicitly accessed stores |
| `POST /ruby/eval`, `POST /sessions/:session_id/ruby/eval`, the browser's **Run RubyForth** | A Ruby-shaped subset compiled to Midscore Forth | `source`, `session_id`, `max_steps`; JSON including `compiled_forth` | Shared Rust partitioned variables |
| `POST /forth/eval` | Midscore Forth | `source`, `session_id`, `max_steps`; JSON | Same shared Rust store |
| Server stdin `rustby <code>` | Real embedded Ruby | Prints result's `to_s` to server console | VM top-level context, not the selected LSL session binding |

**There is no `/rubyforth` HTTP route in this implementation.** RubyForth is the name of the compiler behind `/ruby/eval`. The separate `/1111` bridge described in [the main README](README.md) and [the complete server reference](COMPLETE_DOCUMENTATION.md) targets that runtime; its commands are not aliases for this client's `/7` commands.

Magnus Ruby supports normal Ruby methods, blocks, classes, modules, exceptions, regular expressions, iteration, standard libraries, and installed compatible gems. RubyForth does not. Conversely, Forth words such as `sl.say` and RubyForth helpers such as `get(...)` are not automatically methods in Magnus Ruby. Real Ruby here runs on the server, not inside Second Life, and cannot call LSL functions directly.

## Setup and configuration

### Server

1. Use the Ruby selected by [.ruby-version](.ruby-version), currently **4.0.7**, with its headers and linkable library available to the Magnus build. Use the project's Rust toolchain and native dependencies. The [Cargo manifest](Cargo.toml) enables Magnus embedding; rebuild the server after changing the linked Ruby version.
2. Make sure the selected Ruby has the libraries needed by the startup preludes. The storage prelude requires `json`, `stringio`, and `timeout`; the game client uses `json`, `net/http`, and `uri`. The calendar/weather prelude also requires `oj`, `date`, `fileutils`, and `time`. A prelude loading error is logged and can leave its later definitions unavailable.
3. Configure a reachable HTTPS listener and valid certificate. Second Life verifies the certificate. If using the default privileged port 443, the deployment must be permitted to bind it; another port can be selected explicitly.
4. Set a private, nonempty `TIADE_RUBY_EVAL_TOKEN`. Put the same value in the main LSL script's `RUBY_TOKEN`. Do not put a real token in chat, a public notecard, this guide, or committed source.
5. Select stable, writable storage paths. Use the same paths after every restart. Create a private `server.env` if using the managed launcher; it is executable shell configuration, not JSON. Protect it with mode 600. No deployed secret values are shown here.

Run these commands from this project directory as appropriate:

```sh
ruby --version
cargo check
cargo build --release
./start.sh
```

[start.sh](start.sh) installs the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.128 if needed, builds the browser WASM client and release server, sources `server.env` with automatic export, and starts the binary with `nohup`. It records a PID and appends to the server log. Build-time Ruby selection must already be correct: configuration is sourced **after** the builds. [start-with-wasm.sh](start-with-wasm.sh) is a compatibility alias for the same launcher. [stop-server.sh](stop-server.sh) sends `SIGTERM` to the managed PID; stopping and starting again picks up runtime environment changes.

For an interactive foreground run, use `cargo run` or `./target/release/tiade-maeepers-saerver-all` with the desired variables already exported. These commands do not themselves source `server.env`. The detached launcher does not provide an interactive stdin console.

### Server environment reference

Defaults below are source defaults, not a report of the deployed environment.

| Variable | Default / meaning |
| --- | --- |
| `TIADE_RUBY_EVAL_TOKEN` | Unset/blank disables both `/sl/ruby/*` routes with HTTP 503. Leading/trailing whitespace is trimmed. |
| `TIADE_RUBY_EVAL_SL_OWNERS` | Optional comma-separated Second Life owner UUID allowlist; blank entries ignored, comparison case-insensitive. Requires `X-SecondLife-Owner-Key` when nonempty. Token is still required. |
| `TIADE_RUBY_EVAL_SECONDS` | 10 seconds; integer 1–120 accepted, invalid/out-of-range values fall back to 10. Applies to each SL Ruby evaluation. |
| `MSSL_MEMORY_STORE_PATH` | `/root/midscore_io/tiade-maeepers-saerver-all/partitioned_memory_store.json`; serialized Rust partitioned-memory snapshot. |
| `MSSL_FORTH_FILE_ROOT` | `/root/midscore_io/tiade-maeepers-saerver-all/forth_files`; the nine-method API stores files under this root followed by the session ID. |
| `MSSL_ADDRESS` | `0.0.0.0:443`; TLS listen address. |
| `MSSL_CERT_PATH` | `/etc/letsencrypt/live/stimky.info/fullchain.pem`; certificate chain path. |
| `MSSL_KEY_PATH` | `/etc/letsencrypt/live/stimky.info/privkey.pem`; private-key path, never its contents. |
| `OLLAMA_HTTP_ADDRESS` | `http://localhost:11434`; Ollama upstream, distinct from the LSL-facing server URL. |
| `OLLAMA_MODEL` | `llama2-uncensored:latest`; configured model. Install a model in the upstream Ollama instance, for example with `ollama pull llama2-uncensored:latest`. |
| `OLLAMA_KEEP_ALIVE` | `60m`; model retention on upstream requests. |
| `OLLAMA_PRELOAD` | `true`; set `false` to disable startup model preload. |
| `OLLAMA_SL_NUM_CTX` | 2048; context tokens for SL asks. |
| `OLLAMA_SL_NUM_PREDICT` | 200; maximum generated tokens for SL asks. |
| `OLLAMA_SL_REPLY_CHARS` | 1000; maximum reply characters returned to the SL job client. |
| `OLLAMA_TEAM_PROMPT_TOKEN` | Optional separate Bearer token enabling team prompt management; not the Ruby token. |
| `MSSL_FORTH_BRIDGE_TOKEN` | Optional separate token for the Forth/RubyForth queue bridge; not needed for `/7 ruby`. |

The relay's default team history context is 2,000 characters / 8 entries, with a 500-entry storage limit. Its source configuration allows 8,000-character messages and prompts, 32,000-character serialized game state, 32 pending SL jobs, and a 600-second finished-job retention threshold. These are configuration fields/constants, **not additional environment variable names**. Model lookup is cached for 60 seconds. Relay team/game data uses its own LineDB beneath the configured team log directory (default `/root/midscore_io/logs/ollama_teams/line_db`), separate from Ruby partitioned variables and helper files.

### Second Life object

Install and compile these two scripts **in the same prim**, preserving their names:

- [Tiade.Ollama.Ruby.lsl](Tiade.Ollama.Ruby.lsl): both listeners, commands, HTTP, current session, latest local chat, notecard runner, and a command reference at the end.
- [Tiade.Ollama.Ruby.Help.lsl](Tiade.Ollama.Ruby.Help.lsl): complete command list and eight detailed help pages.

Help uses `LINK_THIS` and link-message number `-708641`, so another prim in the linkset is insufficient. Keep that constant consistent. The main script checks for the Help script's exact inventory name. All non-help features work without the Help script. Remove the legacy Chat and LocalInput scripts from the object; their source files remain only for older split-controller deployments. The combined controller ignores their input link messages, preventing duplicate command execution.

Edit `BASE_URL` and `RUBY_TOKEN` in the controller before use. If changing `CONTROL_CHANNEL`, change it in the controller and Help scripts together. Recompile/reset affected scripts. Use a positive control channel, not channel 0, which is reserved for passive local capture.

| Controller constant | Source default | Purpose |
| --- | --- | --- |
| `BASE_URL` | `https://stimky.info` | Public HTTPS server base; configure your deployment. |
| `TEAM_NAME` | `secondlife` | Ollama team/history selection. |
| `RUBY_TOKEN` | Empty | Ruby authentication; status shows only whether it is set. |
| `CONTROL_CHANNEL` | 7 | `/7` command channel. |
| `PUBLIC_REPLIES` | `FALSE` | Owner-only output by default. |
| `DEBUG_OUTPUT` | `TRUE` | Include automatic `=> last_value.inspect`. |
| `TICK_SECONDS` | 2.0 | Job polling / notecard timer interval. |
| `ASK_DEADLINE` | 240 | Seconds before abandoning an accepted Ollama job locally. |
| `MAX_JOBS` | 4 | Accepted jobs plus in-flight ask submissions allowed by this client. |
| `MAX_REQUESTS` | 8 | Maximum outstanding HTTP ledger entries. |
| `CHUNK_CHARS` | 800 | Output chunk size in characters; chat byte limits still apply. |
| `READ_TIMEOUT` | 30 | Seconds waiting for a notecard line. |
| `RETRY_SECONDS` | 5 | Notecard retry delay for local throttling/busy conditions. |

Smoke-check from the owner account, waiting for each Ruby response:

```text
/7 status
/7 health
/7 help
/7 ruby puts RUBY_DESCRIPTION
/7 session tutorial
/7 ruby var_set("score", 21)
/7 ruby var_get("score") * 2
```

`health` tests the relay/upstream, not Ruby authentication or snapshot writability. A status of Ruby “on” means only that the script has a nonempty token. Ruby is arbitrary execution with the server's privileges; session names provide organization, not an authorization boundary. The owner header is an additional check and can be forged outside Second Life. The nine file helpers restrict their filenames, but general Ruby `File`/network/process APIs are not confined by those helpers.

## Every controller command and alias

Prefix each entry with `/7 ` in chat (or your configured channel). The owner must speak on the control channel; channel-0 conversation never executes commands automatically. The dispatcher trims the message, splits at the first literal space, and lowercases **only the verb**. Argument case is retained. Unknown commands fall through to an Ollama ask, including typos and most unsupported Forth commands.

| Command | Meaning / example |
| --- | --- |
| `ask <message>` | Queue a question for the current team: `/7 ask What is a prim?`. Empty message gives usage. |
| `<unrecognized text>` | Same ask path, using the entire message as the question. |
| `ruby <code>` | Run real Ruby: `/7 ruby x = 21; puts x * 2`. Empty code rejected. |
| `local-input-ruby <code>` | Assign the latest captured text to Ruby local `local_input`, then run code: `/7 local-input-ruby puts local_input.upcase`. |
| `ruby-local-input <code>` | Exact alias of `local-input-ruby`. |
| `ruby-reset` | Drop only the selected session's Ruby binding. Shared stores remain. |
| `debug on` / `debug off` | Show/hide automatic successful last-result output. Only these two settings are accepted, case-insensitively. |
| `session <id>` | Select a case-sensitive session ID: 1–64 ASCII letters, digits, `_`, `-`. Does not reset the old or new session. |
| `session-reset` | Select this object's current UUID again; does not clear any data. |
| `var-set <name> <text>` | Set a **string**: `/7 var-set score 21` saves `"21"`, not integer `21`. |
| `var-get <name>` | Return one stored value. Missing variable is a Ruby error. |
| `var-delete <name>` | Delete a value; return `true` if present, `false` if absent. |
| `var-view` | Return a hash of current-session logical names and values. Takes no arguments. |
| `file-write <name> <text>` | Overwrite a UTF-8 file with the supplied string. |
| `file-read <name>` | Return the file's text. |
| `file-delete <name>` | Delete a file; return whether it existed. |
| `file-list` | Return sorted regular-file names in this session directory. Takes no arguments. |
| `matrix-get <name>` | Return a Forth matrix as a Ruby hash; it must exist in this session. |
| `notecard <inventory name>` | Run a command notecard, line by line. Names may contain spaces. |
| `ruby-notecard <inventory name>` | Send an entire Ruby-only card as one Ruby program. |
| `local-input-ruby-notecard <inventory name>` | Send a Ruby-only card with the latest captured text assigned to `local_input`. |
| `ruby-local-input-notecard <inventory name>` | Exact alias of `local-input-ruby-notecard`. |
| `notecard-stop` | Stop local processing of the active card, or report that none is running. Does not cancel already submitted server work. |
| `team <name>` | Change Ollama team/history. Requires a nonempty name; bare `team` falls through to an ask. Does not change Ruby session. |
| `public <setting>` | `on`, `1`, `true`, `yes` enable public replies, case-insensitively. **Every other value, including empty, disables them.** Prefer `public on` / `public off`. |
| `url <https://host>` | Change base URL. Must begin with lowercase `https://`; removes one trailing slash. Does not change the token. |
| `health` | `GET /ollama/health`; report configured/installed models or failure. |
| `status` | Owner-only settings, session, waiting-job count, card/line progress, free memory, and a preview of latest local input. |
| `local-input` | Show the full latest captured text to the owner, or report that none exists. |
| `help` | Show every command, aliases, card directives, Ruby storage helpers, and the help index. |
| `help all` | Show the complete command overview without the topic index; `all` is case-insensitive. |
| `help 1` … `help 8` | Setup; Ollama; public replies; Ruby/sessions; variables; files/matrices; command cards; Ruby cards/troubleshooting. `help 0` is not accepted. |

These are all dispatcher verbs and both alias pairs. There is no controller command for variable history, clearing all variables, file append, matrix creation, token changes, local-input clearing, or automatic local-chat saving. Use the appropriate Ruby code or HTTP route where available. Commands such as `wait`, `ruby-begin`, and `ruby-end` have special meaning only in **command notecards**.

Storage shorthand is not a shell or Ruby argument parser: names cannot contain spaces, quoted names are not unquoted, and setter text is trimmed at both ends. Quotes, brackets, and backslashes in setter text are data. `/7 var-set config {"a":1}` saves a string containing JSON. Empty strings, significant outer whitespace, multiline strings, and typed values require `/7 ruby` or a Ruby notecard. Names/text are escaped through JSON before insertion into generated Ruby; they are not executed as Ruby expressions.

With `debug off`, return-only calls such as `var-get`, `var-view`, `file-read`, and `file-list` succeed silently. Use explicit output:

```text
/7 debug off
/7 ruby puts var_get("score")
/7 ruby p var_view
/7 ruby puts file_read("notes.txt")
/7 ruby p file_list
/7 debug on
```

`public on` publishes Ollama answers, successful Ruby output, and Ruby evaluation errors on channel 0. Help, status, settings confirmations, local-input display, HTTP diagnostics, and Ruby reset confirmations remain owner-only. Touching the object as owner shows status and the help index. A reset restores compiled settings, including debug on/public off with the supplied defaults.

## Notecards and their directives

### Command notecards

Create a saved inventory notecard named `Demo`, then run `/7 notecard Demo`. Each ordinary line is one controller command. All the commands/aliases above are accepted. An exact leading `/7 ` (or the configured channel) is optional outside Ruby blocks.

| Directive | Effect |
| --- | --- |
| Blank line | Skipped outside a Ruby block. |
| Line beginning `#` or `//` after trimming | Entire line is a comment outside a Ruby block. These are not trailing inline-comment delimiters for commands. |
| `wait <seconds>` | Set a notecard resume time using LSL's integer conversion. Use a nonnegative integer; timer scheduling determines actual resumption. No Ruby `sleep` is involved. |
| `ruby-begin` | Begin collecting a single Ruby program; token must be configured. |
| `ruby-end` | End the block and submit all collected lines together. Closing delimiter must be alone after trimming; do **not** put `/7 ` before the closing delimiter inside the block. |
| `notecard-stop` | Stop the card itself immediately. |

Directive matching is case-insensitive. Ruby source inside a block preserves indentation and is not parsed as commands/comments by the controller; ordinary Ruby `#` comments still work. `//` is not a Ruby comment. A literal trimmed `ruby-end` line ends the block even if you intended it to be source data. Missing `ruby-end` stops at EOF without submitting the unfinished block.

Example command card:

```text
# Demo: each line outside the block is a controller command.
public off
debug on
session tutorial
team secondlife
ask Give one short description of a prim.
wait 2
var-set greeting Hello from a command card
ruby-begin
scores = [10, 20, 30]
var_set("scores", scores)
puts "Total: #{scores.sum}"
ruby-end
var-get scores
file-write notes.txt Created by Demo
file-list
```

The runner waits for HTTP responses before advancing, and for the completed Ollama job after an accepted `ask`. It retries the same line after local HTTP throttling, a full request ledger, too many pending asks, or a busy Ruby request. This does **not** mean all server failures are retried: an HTTP error or Ruby error is reported and ordinary command-card processing generally continues. Cards are not transactions and do not undo successful earlier commands.

Only one inventory card runs at a time. A nested `notecard` command reports “Already running”; it is not a subroutine call. Stopping a card, editing/removing its asset, or resetting the controller stops local reading; already submitted jobs may still run. An unreadable line times out after 30 seconds, including an empty never-saved notecard. Inventory names are case-sensitive. First HTTP-response waits have no separate card deadline; if a response never arrives, stop/reset may be necessary.

### Ruby-only notecards

Create a saved inventory card named `RubyScript` containing only Ruby:

```ruby
record = {
  "name" => "Ada",
  "scores" => [10, 20, 30],
  "active" => true
}
var_set("player", record)
file_write("summary.txt", "Player: #{record['name']}\nTotal: #{record['scores'].sum}\n")
puts file_read("summary.txt")
```

Run `/7 ruby-notecard RubyScript`. The whole card is buffered and sent once at EOF; the “Sent notecard” message means submission, not successful execution. A Ruby reply follows separately. Do not include `/7 ruby`, `team`, `ask`, `local-input-ruby`, `ruby-begin`, or `ruby-end` as controller directives in a Ruby-only card. For example, Ruby interprets hyphenated command text as Ruby syntax rather than an LSL command.

`/7 local-input-ruby-notecard LocalInputRuby` uses the same mode but prefixes a safe assignment to `local_input`. It captures the **latest text when the buffered program is sent**, which may differ from the text present when reading began. It fails if there is no captured input. The reverse-order alias `/7 ruby-local-input-notecard LocalInputRuby` is equivalent.

Source examples are in [sl_notecards/](sl_notecards/), including [Demo.txt](sl_notecards/Demo.txt), [RubyScript.txt](sl_notecards/RubyScript.txt), [LocalInputCommands.txt](sl_notecards/LocalInputCommands.txt), and [LocalInputRuby.txt](sl_notecards/LocalInputRuby.txt). Upload/paste the appropriate contents into saved in-world notecards; an inventory notecard is not automatically a server-side Forth notecard or a helper file.

The server imposes no fixed Ruby source/output length cutoff in these routes, but that does not make LSL unlimited. Buffering and JSON encoding need free script memory. The controller checks roughly `4 * code character count + 2048` bytes of headroom before submission and performs additional checks while buffering. It asks for at most **16,384 response bytes**. Large server results can be truncated by the simulator. Output chunks are 800 characters, while chat limits are byte-based, so multibyte text can exceed a chat line's capacity. Prefer summaries, slices, or files for large data.

## Sessions, resets, and persistence

### Names and lifetime

The LSL session initially equals `llGetKey()` for the prim containing the scripts. Choosing `/7 session shared_demo` lets another object or HTTP caller use the same Ruby binding and storage namespace. Session IDs are case-sensitive; use 1–64 ASCII letters/digits/underscores/hyphens for all interfaces.

The Ruby HTTP parser uses a nonblank JSON `session`, otherwise `X-SecondLife-Object-Key`, otherwise `default`; it trims the chosen value and truncates to 128 characters. Evaluation itself does not apply the stricter storage-name rule. **Every storage helper validates the session as 1–64 safe ASCII characters**, so a session accepted for `1 + 1` can still fail at `var_get`. Use the portable 64-character rule consistently. The `/vars/*` and `/forth/*` APIs use `session_id`, not `session`.

The embedded VM has at most 256 SL session bindings. Creating a new one at capacity evicts the oldest inserted binding (not least-recently-used). Eviction drops its Ruby locals, not shared storage. All Ruby evaluations run serially on one VM worker thread; sessions are separate binding contexts within that process, not isolated Ruby interpreters. Global variables, constants, loaded libraries, and changes to classes/modules can affect other sessions. Do not use Ruby globals as a private or durable store.

| Action / event | Ruby locals | Partitioned variables, history, matrices, server notecards | Helper files | Controller settings / latest chat |
| --- | --- | --- | --- | --- |
| Another evaluation, same session | Retained | Available | Available | Retained |
| `session <id>` / `session-reset` | Select another binding; clear none | Select namespace | Select directory | Latest chat retained |
| `ruby-reset` / `POST /sl/ruby/reset` | Selected binding deleted | Unchanged | Unchanged | Unchanged |
| LSL reset, recompile, re-rez, owner change | Server binding not explicitly deleted | Unchanged | Unchanged | Reset to compiled defaults; chat cleared; session becomes current prim UUID |
| Binding eviction | Evicted binding lost | Unchanged | Unchanged | Unchanged |
| Server restart | All bindings lost | Restore successfully saved snapshot | Remain on disk under the configured root | LSL state remains unless separately reset |

A recreated/copied object may have a different UUID and therefore see a different default namespace. Returning to the old named session recovers its stored values, provided you are connected to the same server/storage. Changing `team` changes Ollama history only. Game client `reset` clears that player's game state/history, not an SL Ruby binding.

### What a successful save means

**Ruby `var_set` and `var_delete` persist before returning success.** Through `update_persisted_vars`, they stage changes in a cloned Rust `VarsStore`, then save the full serialized Rust `PartitionedArray` variable entries **and history** to `MSSL_MEMORY_STORE_PATH` as part of the memory snapshot. [persistent_snapshot.rs](src/persistent_snapshot.rs) writes a unique temporary file, flushes it with `sync_all`, and atomically renames it over the snapshot; directory syncing is best-effort. Only after a successful save does the staged store replace the live store. They do not merely save a simplified Ruby hash. On a mutation or snapshot-save failure, the live variable entries/history remain unchanged and the helper raises a Ruby error instead of acknowledging success. Even deleting a missing variable returns `false` only after the mutation/history operation's persistence path succeeds.

Startup distinguishes a missing snapshot from a broken one: a missing snapshot initializes new stores; an unreadable/corrupt snapshot or unsupported version causes startup to fail rather than silently resetting to empty data. Keep the snapshot and correct configured path when upgrading or restarting. These guarantees require the rebuilt server containing the persistence fix, not an older already-running binary.

`var_get` and `var_view` record history in memory, but do not themselves force an immediate snapshot save. Their history is included in a later successful persistence operation. General `/vars/*` writes and Forth/RubyForth mutations currently update shared memory without an immediate persistence call in those handlers. **Do not infer immediate durability from their successful HTTP responses.** A later snapshot operation may include those changes, but those routes have no individual save acknowledgment equivalent to Ruby `var_set`/`var_delete`.

File helpers are a different system: they perform filesystem I/O under `MSSL_FORTH_FILE_ROOT/<session>/<basename>`. File contents are **not partitioned-array entries** and are not stored inside the memory snapshot. File writes overwrite disk files directly; they are not part of variable rollback. Back up both storage locations if both are used. Do not infer stronger power-loss guarantees for ordinary helper-file writes.

Ruby locals remain ephemeral even when their values resemble stored data:

```text
/7 session tutorial
/7 ruby score = 21
/7 ruby var_set("score", score)
/7 ruby-reset
/7 ruby puts defined?(score).inspect
/7 ruby score = var_get("score"); puts score
```

The last call explicitly reloads the durable value. Updating an array/hash obtained from `var_get` changes a Ruby copy; call `var_set` to save it again. A whole Ruby program is **not** a database transaction: `var_set("x", 1); raise "later error"` leaves the earlier successful save in place. Multi-request read-modify-write logic is not an atomic counter API. A lost HTTP response leaves the client uncertain whether a successful server mutation occurred; read back before retrying a non-idempotent operation.

## All nine Ruby storage methods

These methods are mixed into the selected SL session context by [src/sl_ruby.rs](src/sl_ruby.rs). They are available inside `/7 ruby`, both Ruby-only notecard modes, command-card Ruby blocks, and `/sl/ruby/eval`. They are not automatically methods in a standalone Ruby process or the server stdin top-level binding. Internally they call a JSON bridge into the Rust store; this is not nine separate Ruby HTTP endpoints.

| Ruby method | Arguments | Return value / behavior |
| --- | --- | --- |
| `var_set(name, value)` | String name, JSON-compatible value | JSON-round-tripped stored value; saves entries/history before success. Overwrites the named value. |
| `var_get(name)` | String name | Stored JSON value as Ruby data; missing raises `RuntimeError`. Stored `null` returns `nil`, which differs from missing. |
| `var_delete(name)` | String name | Boolean: whether a value existed; persisted deletion/history before success. |
| `var_view` | None | Hash of logical names to values for the session. Can include Forth's internal matrix/notecard keys. |
| `file_write(name, content)` | Safe basename, String content | The content string, **not** byte count. Creates or overwrites; maximum 65,536 UTF-8 bytes per write. |
| `file_read(name)` | Safe basename | UTF-8 content string; missing/unreadable/invalid-UTF-8 files raise. |
| `file_list` | None | Sorted Array of regular-file names in this session directory. |
| `file_delete(name)` | Safe basename | `true` if deleted, `false` if absent; other filesystem failures raise. |
| `matrix_get(name)` | Forth identifier | Hash with string keys `"rows"`, `"cols"`, `"values"`; values are flat row-major integers. Missing/invalid matrix raises. |

The shared callback requires names to be strings. Variable helpers do not impose the Forth identifier or safe-filename validator on logical variable names; prefer simple identifiers for interoperability. Avoid the reserved `__rf_session_` namespace prefix, and treat `forth.matrix.*` / `forth.notecard.*` as internal data unless deliberately manipulating those formats. Matrix identifiers match `[A-Za-z_][A-Za-z0-9_]*`.

File names are 1–128 ASCII characters drawn from letters, digits, `_`, `-`, `.`. They may not begin with `.` or contain `..`; no slashes, directory traversal, or nested paths. `notes.txt` is valid; `.notes`, `../notes`, `a..b`, and `folder/notes.txt` are invalid. All nine helpers require a valid shared-storage session, including file-only calls.

The bridge uses `JSON.generate` on Ruby arguments and `JSON.parse` on results. Use strings, booleans, `nil`, representable integers, finite floats, arrays, and hashes with string keys. Arbitrary Ruby object identity, symbols, classes, procs, cycles, non-finite numbers, and unbounded Ruby integers are not preserved by this JSON/Rust boundary. Serialize custom objects intentionally; use decimal strings for numbers outside the supported JSON integer range and integer minor units or decimal strings when exact decimal arithmetic matters. A Ruby `Time` or `Date` should be converted to an explicit string rather than relying on incidental JSON behavior.

## Typed values and ordinary Ruby recipes

These examples run as Ruby source in a Ruby-only card or after `/7 ruby`. Long/multiline examples are best put in a card. General Ruby has far more functionality than these examples.

### Primitive values and mixed structures

```ruby
var_set("count", 21)                       # Integer, unlike /7 var-set count 21
var_set("ratio", 0.75)                     # Float
var_set("enabled", true)                   # Boolean
var_set("optional", nil)                   # Present JSON null
var_set("empty", "")                       # Empty String
var_set("greeting", "  Hello\nworld  ")     # Preserve whitespace/newline
var_set("items", ["key", "map", 3, false])
var_set("player", {"name" => "Ada", "hp" => 10, "tags" => ["mage"]})
puts var_get("count").class                # Integer
p var_get("optional")                     # nil, not a missing-variable error
puts JSON.pretty_generate(var_get("player"))
```

### Read, change, save; defaults without hiding failures

```ruby
values = var_view
count = values.fetch("count", 0)
raise "count must be an Integer" unless count.is_a?(Integer)
var_set("count", count + 1)

player = values.fetch("player", {"name" => "Ada", "hp" => 10, "tags" => []})
player["hp"] += 5
player["tags"] << "healed"
var_set("player", player)
```

`fetch` preserves a stored `false` or `nil`; `value || default` does not. Avoid broadly rescuing every storage error into an empty default, which can hide failed reads or validation problems. `var_view.key?("optional")` distinguishes a present `nil` from an absent key. There is no `var_exists?` method.

### Parse JSON text and use Ruby's language features

```ruby
input = '{"hp":10,"inventory":["key","map"]}'
parsed = JSON.parse(input)
var_set("state", parsed)

numbers = (1..5).map { |n| n * n }
puts numbers.select(&:odd?).sum
puts "ruby magnus".split.map(&:capitalize).join(" ")

def describe_player(player)
  "#{player.fetch('name')} has #{player.fetch('hp')} HP"
end
puts describe_player(var_get("player"))

begin
  JSON.parse("not JSON")
rescue JSON::ParserError => error
  puts "Input was not valid JSON: #{error.message}"
end
```

Methods/classes/modules and libraries are real Ruby features, but their lifetime and visibility follow Ruby rules in the shared VM. Session reset is not a VM-wide cleanup of global definitions. `puts` emits readable text and a newline; `print` does not add a newline; `p` prints `inspect`. `$stdout` is captured, not `$stderr` or every native/subprocess output stream. Returning `nil` normally adds `=> nil` unless debug/result inclusion is off.

### Time, UTF-8, and structured files

```ruby
require "time"
var_set("saved_at", Time.now.utc.iso8601)
file_write("unicode.txt", "Hello, 世界 🌍\n")
file_write("state.json", JSON.pretty_generate(var_get("state")))
restored = JSON.parse(file_read("state.json"))
var_set("restored_state", restored)
puts restored.fetch("inventory").join(", ")
```

`require` can load installed libraries; it does not install gems. The helper file is an explicit copy, not automatic synchronization with the variable. For an append-like operation, read then overwrite, keeping the size bound:

```ruby
name = "journal.txt"
old = file_list.include?(name) ? file_read(name) : ""
content = old + "One new line\n"
raise "journal too large" if content.bytesize > 65_536
file_write(name, content)
```

This is not an atomic append across callers. For selective cleanup, use `var_delete("restored_state")` or `file_delete("state.json")`. To remove every visible logical variable intentionally, Ruby can iterate `var_view.each_key { |name| var_delete(name) }`; this also includes stored matrix/notecard entries, performs separate saves, and is not a replacement for a transactional clear.

## Local chat: capture, process, explicitly save

The combined controller listens directly on channel 0 and keeps only the most recent message in `gLastLocalChat`. New messages overwrite that slot. It is not a conversation log and contains no retained speaker UUID, name, timestamp, or history. Capture and `/7 local-input` alone make no HTTP request and no durable write.

The capture listener is `llListen(0, "", NULL_KEY, "")` and its event only checks the channel. It does **not** filter by owner or verify that the speaker is an avatar. Any speaker delivered by that listener, including other objects where the simulator delivers their chat, can update the slot. Legacy local-input link messages are no longer accepted. Treat captured text as data from nearby chat; only the separate owner-filtered control-channel listener dispatches commands.

Explicit processing and saving are separate actions:

```text
/7 public off
/7 local-input
/7 local-input-ruby puts local_input.upcase
/7 local-input-ruby var_set("last_chat", local_input); puts "Saved last_chat"
/7 ruby puts var_get("last_chat")
```

The processing call safely JSON-escapes the text and assigns it to Ruby `local_input` before executing your code. It never evaluates the captured string as Ruby by itself. `local_input` then remains an ordinary ephemeral Ruby local in that session until overwritten, reset, eviction, or server restart. Only the explicit `var_set` makes the text part of the persisted variable store; `file_write("last-chat.txt", local_input)` instead saves a helper disk file.

For typed chat, say valid JSON on channel 0, then explicitly parse, validate, and save it:

```text
/7 local-input-ruby data = JSON.parse(local_input); raise "Expected a Hash" unless data.is_a?(Hash); var_set("chat_record", data); puts "Saved"
```

This does not treat text as code. Invalid JSON or a failed save raises and does not print “Saved”. Do not substitute `eval(local_input)` for JSON parsing.

A Ruby-only notecard can retain a bounded message list when explicitly invoked:

```ruby
require "time"
logs = var_view.fetch("chat_log", [])
raise "chat_log must be an Array" unless logs.is_a?(Array)
logs << {"text" => local_input, "saved_at" => Time.now.utc.iso8601}
var_set("chat_log", logs.last(20))
puts "Saved #{logs.last(20).length} entries"
```

Run this with `/7 local-input-ruby-notecard SaveChat`. The timestamp is save time, not original chat time; repeated invocation can save the same captured text twice. It does not install an automatic logger. Read-only output can use [LocalInputRuby.txt](sl_notecards/LocalInputRuby.txt). A command card should use `local-input-ruby ...` as a command and be run with `notecard`, not `ruby-notecard`.

`/7 local-input` always displays privately, even if public replies are on. Ruby that prints the same text follows the public-reply setting, so use `public off` for private processing. LSL reset clears the capture slot. `/7 ruby-reset` clears the Ruby local but not the controller's captured text or an explicitly saved variable/file.

## Sharing variables, matrices, and files

Use the same server and exact session string on every side. This is the only connection between an LSL object's selected namespace and a Forth/HTTP caller. No conversion is required between JSON-compatible Ruby values and `/vars/*` values.

### Variables through Ruby and HTTP

In-world:

```text
/7 session shared_demo
/7 ruby var_set("score", 42)
```

From a shell (use your server URL):

```sh
export TIADE_BASE_URL='https://your-host.example'
curl --silent --show-error "$TIADE_BASE_URL/vars/get" \
  -H 'Content-Type: application/json' \
  --data-binary '{"session_id":"shared_demo","name":"score"}'
```

The JSON response is `42`. A `/vars/set` body is a flat object such as `{"session_id":"shared_demo","score":43}`; `session_id` is removed before storing the remaining keys. It is not `{"name":"score","value":43}`. Such a `/vars/set` updates the shared in-memory value but does not immediately persist it. Use Ruby `var_set` when the save acknowledgment is needed.

### Matrices

Create a Forth matrix in the matching session:

```sh
curl --silent --show-error "$TIADE_BASE_URL/forth/eval" \
  -H 'Content-Type: application/json' \
  --data-binary '{"session_id":"shared_demo","source":"2 2 matrix A 1 0 0 mset A 2 0 1 mset A 3 1 0 mset A 4 1 1 mset A m.show A","max_steps":1000}'
```

`mset A` consumes `value row column`, using zero-based coordinates. Then:

```text
/7 session shared_demo
/7 matrix-get A
/7 ruby m = matrix_get("A"); p m["values"].each_slice(m["cols"]).to_a
```

The Ruby value is `{"rows"=>2, "cols"=>2, "values"=>[1,2,3,4]}`. Element `(row, col)` is `m["values"][row * m["cols"] + col]`. Matrices have 1–64 rows/columns, at most 4,096 cells, and signed integer values as enforced by the Forth representation. There is no tenth Ruby helper named `matrix_set`. Forth provides creation and operations including `matrix`, `m.identity`, `mget`, `mset`, `m.fill`, `m.scale`, `m.add`, `m.sub`, `m.mul`, `m.solve`, `m.transpose`, `m.det`, `m.rows`, `m.cols`, and `m.show`; see [the matrix reference](README.md#matrix-forth) and `GET /forth` for its runtime catalog.

Matrix records live in the partitioned variable store under logical names such as `forth.matrix.A`. `matrix_get` returns a copy and does not save local edits. For an explicit snapshot-backed rewrite of a validated existing matrix record, use:

```ruby
matrix = matrix_get("A")
var_set("forth.matrix.A", matrix)
```

This goes through Ruby's immediate variable-persistence path; the preceding Forth HTTP response alone did not. Directly storing malformed data under an internal matrix key can make `matrix_get` fail, so prefer Forth's matrix operations and preserve the validated structure. A normal variable containing a nested array is not automatically a Forth matrix.

### Shared disk files

```text
/7 session shared_demo
/7 ruby file_write("notes.txt", "One\nTwo\n")
```

Read the same file via HTTP:

```sh
curl --silent --show-error "$TIADE_BASE_URL/forth/files/read" \
  -H 'Content-Type: application/json' \
  --data-binary '{"session_id":"shared_demo","name":"notes.txt"}'
```

The HTTP response wraps the content as `{"name":"notes.txt","content":"One\nTwo\n"}`; Ruby `file_read` returns only the string. Even the `default` session uses a `default` subdirectory for scoped helper files. Switching sessions does not move files, and `ruby-reset` does not remove them.

## HTTP protocols

### Magnus evaluation and reset

`POST /sl/ruby/eval` accepts a JSON **object** or raw Ruby source. Object bodies use:

```json
{
  "code": "puts 'hello'; 6 * 7",
  "session": "tutorial",
  "include_result": true
}
```

`code` must be a nonblank string. `include_result` defaults to `true` and must be a JSON boolean if present; `"false"` is invalid. `session` selection is described above. A body that does not parse as a JSON object is treated as raw code regardless of its content-type. Therefore use an explicit `code` object for programs that could themselves look like JSON objects.

Both eval and reset require authentication. Token precedence is:

1. `X-Ruby-Token` header;
2. otherwise an `Authorization` header beginning with exactly `Bearer `;
3. otherwise a JSON string field `token`.

An incorrect higher-priority token does not fall back to a correct lower-priority one. Header token values are trimmed before comparison. Prefer headers over putting the token in code or data files. The optional owner allowlist checks `X-SecondLife-Owner-Key`; external clients must also supply an allowed owner header when configured.

`POST /sl/ruby/reset` uses the same authentication/session parser, accepts `{"session":"tutorial"}` or an empty body, and does not require code. A 200 response is `session tutorial reset` or `session tutorial was empty`. Reset is queued on the same VM worker and has a 10-second host wait. It cannot interrupt an evaluation currently occupying that worker.

Example requests assume `TIADE_RUBY_EVAL_TOKEN` is already set privately in the caller's environment. Add the owner header if your deployment requires it:

```sh
curl --silent --show-error "$TIADE_BASE_URL/sl/ruby/eval" \
  -H "X-Ruby-Token: $TIADE_RUBY_EVAL_TOKEN" \
  -H 'Content-Type: application/json' \
  --data-binary '{"session":"tutorial","code":"puts 6 * 7","include_result":false}'

curl --silent --show-error "$TIADE_BASE_URL/sl/ruby/eval" \
  -H "Authorization: Bearer $TIADE_RUBY_EVAL_TOKEN" \
  -H 'X-SecondLife-Object-Key: tutorial' \
  -H 'Content-Type: text/plain' \
  --data-binary 'var_get("score")'

curl --silent --show-error "$TIADE_BASE_URL/sl/ruby/reset" \
  -H "X-Ruby-Token: $TIADE_RUBY_EVAL_TOKEN" \
  -H 'Content-Type: application/json' \
  --data-binary '{"session":"tutorial"}'
```

Responses are `text/plain; charset=utf-8`, not JSON:

| Status | Meaning |
| --- | --- |
| 200 | Success: captured `$stdout` plus `=> value.inspect`, unless `include_result:false`. Successful output can be empty. |
| 400 | Empty/missing code or nonboolean `include_result`. |
| 401 | Token absent or incorrect. |
| 403 | Owner allowlist rejected the request. |
| 422 | Ruby exception, including syntax/load errors, `exit` (`SystemExit`), storage errors, or the Ruby-level timeout. Prior captured stdout is retained. |
| 503 | Route disabled or embedded VM unavailable. |
| 504 | Host wait for the VM exceeded its limit; eval waits configured Ruby seconds + 5. |

The SL prelude runs evaluation under `Timeout.timeout`. A normal Ruby timeout reports `Timeout: evaluation exceeded <seconds>s`. This is distinct from the generic VM host timeout: a 504 or simulator timeout does not cancel a queued/running VM job, and native/blocking work is not guaranteed to stop instantly. The LSL client reports simulator HTTP 499 separately and advises waiting. One outstanding Ruby/reset call per controller reduces queue delays but does not prevent another object, CLI command, or Ruby-backed route from using the same VM.

### Asynchronous Ollama asks

The controller uses `POST /sl/ask/:team` with `{"message":"...","speaker":"..."}`; raw text also works. An optional speaker becomes `speaker says: message` before processing. Team is URL-encoded; session is not sent because this route uses team history.

An accepted request returns 202:

```json
{"job":"opaque-job-id","status":"pending","poll":"/sl/job/opaque-job-id"}
```

Poll `GET /sl/job/:id` until a terminal result:

```json
{"job":"opaque-job-id","status":"pending"}
```

```json
{"job":"opaque-job-id","status":"done","reply":"Answer text","model":"model-name","seconds":1.2}
```

```json
{"job":"opaque-job-id","status":"error","error":"Explanation"}
```

These states all use HTTP 200; inspect `status`. Missing/expired jobs give 404. Submission can reject malformed/empty input with 400, oversized input with 413, or a full server queue with 429. The controller polls one job per two-second tick round-robin, does not submit duplicate polls while one is outstanding, and abandons jobs locally after 240 seconds. Non-200/non-404 poll responses are retried later. Abandonment does not cancel generation. Server jobs are in memory, so restart loses job IDs; finished jobs older than 600 seconds from creation are pruned when new jobs are submitted, not by a precise completion-time expiration timer. Pending jobs are retained by that pruning pass.

Ask output is bounded by the SL reply settings and shares the team's relay history. No Ruby token is sent for asks or health. `GET /ollama/health` returns 200 with `status`, `version`, `ollama_http_address`, `configured_model`, and `available_models`, or 503 with an upstream error. A healthy upstream with an empty model list is not proof that an ask can generate a reply.

### Shared-store HTTP route inventory

These are the finite shared-store routes relevant to Ruby interoperability. Bodies are JSON; include `session_id` to avoid unintentionally selecting `default`. The Ruby token guard does not automatically protect these routes; apply the deployment's access controls separately.

| Method and path | Body / result |
| --- | --- |
| `POST /vars/set` | Flat object of variable pairs plus optional `session_id`; success has empty body. No immediate persist. |
| `POST /vars/get` | `name`, `session_id`; raw JSON value or 404. |
| `POST /vars/view` | `session_id`; JSON object of visible values. |
| `POST /vars/delete` | `name`, `session_id`; empty success body. No immediate persist. |
| `POST /vars/clear` | `session_id`; clears visible entries (including internal matrix/notecard records), not files or history. Empty success body; no immediate persist. |
| `POST /vars/history` | `session_id`; array of recorded operations/snapshots with timestamps. No restore-history command is supplied. |
| `POST /vars/status` | `session_id`; count/session and static server/debug/public fields. Those fields are not your LSL toggles. |
| `GET /sessions/:session_id/status` | Returns `session_id`, `value_count`, `values`. |
| `POST /forth/files/write` | `session_id`, `name`, `content`; returns `name`, byte count `bytes`. |
| `POST /forth/files/read` | `session_id`, `name`; returns `name`, `content`. |
| `POST /forth/files/list` | `session_id`; returns `files` array. |
| `POST /forth/files/delete` | `session_id`, `name`; returns `name`, boolean `deleted`. |
| `POST /forth/matrices/get` | `session_id`, `name`; returns `name`, `matrix`. |
| `POST /forth/notecards/save` | `session_id`, `name`, `source`; returns `saved` name. Stores source in partitioned variables; no immediate persist. |
| `POST /forth/notecards/get` | `session_id`, `name`; returns `name`, `source`. |
| `POST /forth/notecards/list` | `session_id`; returns `notecards` array of name objects. |
| `POST /forth/notecards/delete` | `session_id`, `name`; returns `deleted` name; no immediate persist. |
| `POST /forth/notecards/run` | `session_id`, `name`, optional `max_steps`; executes saved Forth source. |
| `POST /ruby/notecards/run` | Same fields; compiles saved source as RubyForth, **not Magnus Ruby**. |

Older path-based `/vars/get/:name` and `/vars/delete/:name` examples occur inside commented-out code in [src/main.rs](src/main.rs); they are not mounted routes. Use the body-based session APIs above.

## Ruby Ollama game client

[ruby_client/ollama_game_client.rb](ruby_client/ollama_game_client.rb) implements `OllamaGameClient::Client` using Ruby standard libraries. It is preloaded into the embedded VM at startup; a standalone Ruby process must require it. It is an **HTTP client for the Ollama relay**, not a Magnus evaluation client or a substitute for the nine storage helpers.

All public methods and accessors:

| Method | HTTP operation / return |
| --- | --- |
| `Client.new(server_url:, team:, player:)` | Required keywords; strips trailing slashes from base URL, stores team/player. |
| `server_url`, `team`, `player` | Read-only accessors. |
| `routes` | `GET /ollama`; whole parsed route catalog. |
| `health` | `GET /ollama/health`; whole parsed health object. |
| `chat(message)` | `POST /chat/:team` with `message`; returns whole object including `response`, `team`, `model`, `fallback_used`, `history_chars`. |
| `turn(action:, game_prompt: nil, state: nil)` | `POST /game/:team/:player/turn`; returns whole object with `response`, `directive`, `state`, `team`, `player`, `model`, `fallback_used`. Omits empty/nil prompt and nil state. |
| `state` | `GET /game/:team/:player/state`; extracts and returns only `state`. |
| `reset` | `POST /game/:team/:player/reset`; clears that player's state/history and returns state `{}`. |

Private `game_path`, `escape`, and `request` implement routing, URI component escaping, and JSON requests. They are not additional user-facing commands. There is no public Ruby client `history`, `ask`, `eval`, `var_set`, or team-prompt method. Use HTTP explicitly or the SL storage context for those interfaces.

Standalone example, run from the project directory:

```ruby
require_relative "ruby_client/ollama_game_client"

client = OllamaGameClient::Client.new(
  server_url: "https://your-host.example",
  team: "arcade",
  player: "player-42"
)
puts client.health.fetch("status")
puts client.chat("Give one short quest idea.").fetch("response")
turn = client.turn(
  action: "open the north door",
  game_prompt: "Fantasy dungeon. Keep choices concise.",
  state: {"inventory" => ["lantern", "silver key"]}
)
puts turn.fetch("directive").fetch("narrative")
p client.state
# client.reset  # Explicitly discard this player's saved game state/history.
```

Inside `/7 ruby` or a Ruby-only card, omit `require_relative` because the client is preloaded. Supply the URL/team/player explicitly: it does not automatically inherit the controller's URL, team, session, or Ruby token. Team/player are percent-encoded path components. Responses are JSON; the SL Magnus route itself still returns plain text when you print or return client results.

The client sends `Accept: application/json` and JSON content type for payloads, uses TLS when the URL is HTTPS, and does not attach authentication tokens or implement retries/polling/custom timeouts. Non-success JSON responses raise `OllamaGameClient::Error`; invalid JSON is wrapped in the same error class. Network/TLS errors may propagate from the standard library. Client calls are synchronous. Slow generation inside SL Ruby may exceed its default 10-second evaluation budget and occupy the shared VM; use the asynchronous `/7 ask` path for ordinary in-world questions. The [raylib game-loop example](ruby_client/raylib_game_loop_example.rb) demonstrates consuming directives as data while keeping rendering/input/physics/validation in application code.

Game `state` must be a JSON object when supplied. Omitting it uses the player's stored state. The relay persists the complete next state and player history separately from Ruby variables. Validate model-proposed actions in the game application rather than executing response text as Ruby.

The relay's remaining public routes, discoverable with `GET /ollama`, are `POST /ollama` (standard non-streaming Ollama chat JSON with `messages`; streaming is rejected), `POST /game/:team/:player` (`message` and optional `game_prompt`, without the structured turn API), `GET /history/:team` (history/team), and `GET`/`POST /teams/:team/prompt` (Bearer `OLLAMA_TEAM_PROMPT_TOKEN`; POST body `{"prompt":"..."}`, empty prompt clears it). There are no matching dedicated `/7` verbs or public convenience methods in this Ruby client for these routes.

## RubyForth mini compiler

The finite RubyForth language is implemented in Rust in [src/main.rs](src/main.rs), without invoking Magnus. Its syntax includes:

- Integer/string expressions, `true`, `false`, `nil`, parentheses, arithmetic/comparisons/boolean operations supported by the compiler; negative literals have restricted unary-minus handling.
- Assignment `=`, `+=`, `-=`, `*=`, `/=` to identifier-like shared variables.
- `puts <expression>` and `p <expression>`; bare `return` compiles to Forth `bye`.
- `if`, `unless`, `elsif`, `else`, `while`, `until`, `n.times do ... end`, and matching `end` blocks.
- One-line arrays and hashes; `get(collection, key)`, `set(collection, key, value)`, `push(array, value)`, `pop(array)`, `delete(collection, key)`, `keys(hash)`, `length(collection)` / `len(collection)`, `has(collection, key)`.
- Limited `#{name}` interpolation, comment handling, and semicolon-separated statements outside strings/collections.
- `forth "<source>"` and `forth do ... end` bridges into Forth. Forth's `ruby "<source>"` compiles RubyForth, not real Ruby.

It does not support arbitrary Ruby method calls, classes, user-defined Ruby methods, `require`, gems, `eval`, arbitrary interpolation expressions, or multiline collection literals. Do not assume Ruby truthiness, numeric semantics, mutation semantics, or library compatibility from its Ruby-like appearance; it executes Forth's integer/collection operations and step limits.

```sh
curl --silent --show-error "$TIADE_BASE_URL/ruby/eval" \
  -H 'Content-Type: application/json' \
  --data-binary '{"session_id":"shared_demo","source":"count = 0; while count < 3; puts count; count += 1; end","max_steps":1000}'
```

This changes the shared `count` variable directly. In Magnus, `count = 0` creates a Ruby local; `var_set("count", 0)` explicitly changes/persists the shared value. RubyForth success returns JSON describing the run and compiled Forth. Its changes are not immediately persisted by the eval handler. `max_steps` defaults to 10,000 and must be 1–1,000,000; it is not the Magnus time limit.

The complete companion execution route set is `GET /forth`, `GET /forth/ui`, `GET /forth/example`, `GET /ruby`, `POST /forth/eval`, `POST /ruby/eval`, `POST /sessions/:session_id/forth/eval`, and `POST /sessions/:session_id/ruby/eval`, alongside the shared-storage routes above. `POST /forth/algebra` handles the separate algebra helper. `POST /forth/bridge/enqueue` and `/forth/bridge/poll` queue/deliver Forth or RubyForth work for the separate LSL bridge using its own token. No queue command in this two-script client polls that bridge. Use the dedicated [complete documentation](COMPLETE_DOCUMENTATION.md) for Forth stack words, algebra, bridge payloads, and unrelated server APIs.

## Server console and other Magnus routes

The server stdin dispatcher has exactly these commands:

| Console input | Action |
| --- | --- |
| `rustby <Ruby code>` | Evaluate in the embedded VM top-level context; print `Ruby output: <result.to_s>` or an error. No automatic SL stdout/last-result wrapper or selected SL session is applied. |
| `rustby` | Evaluate the demo string `'RustbySpace'`. |
| `exit` | Attempt a memory snapshot save, report a failure if any, then exit. |
| `restart` | Spawn `killall -HUP tiade-maeepers-saerver-all`. This affects matching processes; it does not itself spawn a replacement server. A supervisor is needed for automatic relaunch. |

Unknown input prints an “Unknown command” message. Generic VM calls wait up to 120 seconds; reaching that host wait limit does not terminate the Ruby job. This differs from the SL prelude's Ruby-level timeout. Ruby's startup signal handlers are reset to system defaults so the host's signal behavior applies. Do not treat every possible termination signal as a guaranteed final save.

The server also preloads the game client and calendar/weather definitions and exposes these fixed Ruby-backed text routes:

| GET route | Preloaded Ruby entry points |
| --- | --- |
| `/time` | `Calendar.new.gregorian`, `.julian`, `.julian_primitive`, `formatted_pst_time` |
| `/ae` | `AECalendar.new.ae_date(DateTime.now)` |
| `/tiade/moon` | `MoonPhaseDetails2.print_text_details_for_date(Date.today)` |
| `/weather` | `ForecastByLongitude.new.fetch_forecast(...)`; the implementation uses its fixed weather.gov gridpoint URL. |
| `/tiade/sun` | `SolarDance2.sun_dance_message` |

They evaluate fixed programs, return result `to_s` as plain text, use HTTP 500 on Ruby failure and 504 on the generic host timeout, and share the VM with SL Ruby. Their formatting and domain calculations are project-specific, not additional storage commands. They depend on the route prelude having loaded successfully.

## Troubleshooting

| Symptom | Check / action |
| --- | --- |
| `/7` does nothing | Speak as object owner; check the combined main script is compiled/running and its positive control channel matches the one used in chat. No separate Chat script is needed. |
| Help missing | Install Help with the exact script name in the same prim; use `help`, `help all`, or `help` followed by one digit 1–8. |
| Typo produces an Ollama answer | Unknown verbs intentionally fall through to ask. Check the command table; `wait` is card-only. |
| “Set RUBY_TOKEN” | Configure the main script token; there is no runtime token chat command. |
| HTTP 401 / 403 | Check token match / owner allowlist. Do not publish the token while diagnosing. |
| HTTP 503 | Distinguish disabled Ruby from VM unavailability or Ollama failure; read the response and server startup logs. |
| TLS/connection error | Check HTTPS URL, DNS, listener/port, valid certificate chain, firewall, and certificate paths. `url` requires `https://`. |
| `health` succeeds but asks fail | Inspect installed models; check upstream model availability, load time, queue limits, and server error. Health alone does not verify generation. |
| Successful Ruby appears silent | `debug off` hides return values; use `puts`/`p`, or `debug on`. |
| `NameError` after reset/restart | Ruby locals are ephemeral. Select the right session and reload with `var_get`. A failed startup prelude can also leave classes/client definitions missing. |
| `LoadError` for a library | Install it for the Ruby linked into Magnus and rebuild/restart as needed; a different shell Ruby's gems may not be available. |
| Variable exists but has wrong type | Chat `var-set` saves strings. Check `.class`; use typed Ruby or parse JSON explicitly. |
| Variable missing / empty `var_view` | Check base URL, session case, prim UUID changes, correct `session` versus `session_id`, and snapshot path. `team` does not select storage. |
| `session_id must be ...` during Ruby | Eval accepted a broader session string; helpers require 1–64 safe ASCII characters. |
| Variable mutation/save error | Check snapshot directory access, disk space, and serialization/storage errors. Ruby mutation/history rollback means success was not acknowledged; do not hide the error. |
| Startup rejects snapshot | Preserve the broken snapshot for diagnosis and recover a known-good backup or correct its schema/version. Do not replace it with an empty object to make startup appear successful. |
| Data written through `/vars/*` or Forth disappears after abrupt stop | Those mutation handlers lack immediate persistence. Use Ruby's acknowledged `var_set`/`var_delete` where that guarantee is needed. |
| Editing a returned hash does not update store | `var_get` and `matrix_get` return copies. Explicitly save the changed data. |
| “file name must be a safe basename” | Use a name such as `notes.txt`, no slash/leading dot/`..`; maximum 128 characters. |
| File content too large / invalid text | Writes are limited to 65,536 bytes and helper reads require UTF-8. Use `.bytesize`, not just `.length`. |
| Matrix missing/invalid | Create it in the same session, use a valid identifier and valid dimensions/integer data; ordinary arrays do not become matrix records automatically. |
| No local input | Check the combined main script is running, say text on channel 0 within listener range, then use `local-input`. Reset clears capture; no separate LocalInput script is needed. |
| Wrong local message processed | The slot holds only the latest delivered text. Other nearby speech can replace it; cards bind text when sent. There is no speaker filter/history in the controller. |
| Local chat was not saved | Capture is only LSL memory. Explicitly use `local-input-ruby var_set(...)` or `file_write(...)`. |
| `local-input`, `ruby-begin`, etc. cause Ruby errors | A command card was run as a Ruby-only card. Use `notecard` for controller lines and `ruby-notecard` for Ruby source. |
| Notecard missing, changed, or stuck reading | Check exact inventory name, save it, avoid edits during execution; check the 30-second line timeout. |
| “Already running notecard” | Stop the existing card first; nesting is unsupported. |
| Insufficient script memory / truncated output | Shorten the card/program, reset local buffers when safe, and print smaller results. Server acceptance does not bypass LSL memory or response limits. |
| Busy Ruby / too many requests / region throttle | Wait for replies. Cards retry local throttle conditions after five seconds; manual commands must be retried by the owner. |
| Ruby HTTP 499/504 or no response | The server may still run/queue the program. Wait and verify state before resubmitting writes; reset is not cancellation. |
| Error after an earlier successful `var_set` | Program-level errors do not roll back already completed helper calls. Only the individual failing mutation's rollback applies. |

For a controlled restart check, save a known typed value in a named test session, restart through the deployment's normal process, select that same session, and read it back. Test deletion the same way. Perform such checks against a test deployment when interrupting the running server would disrupt users. This guide describes the source behavior; a stale deployed binary must be rebuilt before relying on the persistence changes.
