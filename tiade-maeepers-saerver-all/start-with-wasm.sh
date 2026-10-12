#!/usr/bin/env bash
set -euo pipefail

# Kept for callers of the old command. start.sh now builds the browser WASM
# client, builds the release server, and then restarts it using stop.sh.
project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$project_dir/start.sh"
