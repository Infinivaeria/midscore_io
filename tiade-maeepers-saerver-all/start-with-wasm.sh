#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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
"$project_dir/start.sh"