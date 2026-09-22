#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
pid_file="$project_dir/server.pid"

if [[ ! -f "$pid_file" ]]; then
	printf 'Server is not running.\n'
	exit 0
fi

pid="$(<"$pid_file")"
if kill -0 "$pid" 2>/dev/null; then
	kill -TERM "$pid"
	printf 'Stopped server with PID %s\n' "$pid"
else
	printf 'Removed stale PID %s\n' "$pid"
fi

rm -f "$pid_file"
