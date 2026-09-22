#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
binary="$project_dir/target/release/tiade-maeepers-saerver-all"
pid_file="$project_dir/server.pid"
log_file="$project_dir/server.log"

if [[ ! -x "$binary" ]]; then
	printf 'Release binary not found: %s\n' "$binary" >&2
	exit 1
fi

if [[ -f "$pid_file" ]] && kill -0 "$(<"$pid_file")" 2>/dev/null; then
	printf 'Server is already running with PID %s\n' "$(<"$pid_file")"
	exit 0
fi

rm -f "$pid_file"
cd "$project_dir"
nohup "$binary" >>"$log_file" 2>&1 &
printf '%s\n' "$!" >"$pid_file"
printf 'Started server with PID %s\n' "$!"

