#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
binary="$project_dir/target/release/tiade-maeepers-saerver-all"
pid_file="$project_dir/server.pid"
log_file="$project_dir/server.log"
wasm_target="wasm32-unknown-unknown"
wasm_bindgen_version="0.2.128"
wasm_binary="$project_dir/wasm_client/target/$wasm_target/release/ollama_game_client.wasm"
wasm_output_dir="$project_dir/src/assets/wasm"

if ! rustup target list --installed | grep -qx "$wasm_target"; then
	printf 'Installing Rust target %s\n' "$wasm_target"
	rustup target add "$wasm_target"
fi

if ! command -v wasm-bindgen >/dev/null 2>&1 || [[ "$(wasm-bindgen --version)" != "wasm-bindgen $wasm_bindgen_version" ]]; then
	printf 'Installing wasm-bindgen-cli %s\n' "$wasm_bindgen_version"
	cargo install wasm-bindgen-cli --version "$wasm_bindgen_version" --locked --force
fi

printf 'Building browser WASM client\n'
cargo build --manifest-path "$project_dir/wasm_client/Cargo.toml" --target "$wasm_target" --release
mkdir -p "$wasm_output_dir"
wasm-bindgen --target web --out-dir "$wasm_output_dir" --out-name ollama_game_client "$wasm_binary"

printf 'Building release server\n'
cd "$project_dir"
cargo build --release

# Optional secrets/config (e.g. TIADE_RUBY_EVAL_TOKEN, OLLAMA_KEEP_ALIVE); not committed.
if [[ -f "$project_dir/server.env" ]]; then
	set -a
	# shellcheck disable=SC1091
	source "$project_dir/server.env"
	set +a
fi

# Keep the current server available until both builds and configuration loading
# succeed. Wait for it to release its sockets before launching the replacement.
"$project_dir/stop.sh"
nohup "$binary" >>"$log_file" 2>&1 &
pid=$!
printf '%s\n' "$pid" >"$pid_file"
sleep 1
if ! kill -0 "$pid" 2>/dev/null; then
	rm -f "$pid_file"
	printf 'Server exited during startup. Check %s\n' "$log_file" >&2
	exit 1
fi
printf 'Started server with PID %s (log: %s)\n' "$pid" "$log_file"
