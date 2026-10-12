#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
pid_file="$project_dir/server.pid"
binary="$project_dir/target/release/tiade-maeepers-saerver-all"

if [[ ! -f "$pid_file" ]]; then
	printf 'Server is not running.\n'
	exit 0
fi

pid="$(<"$pid_file")"
# Never let an empty, negative, or process-group PID reach kill.
if [[ ! "$pid" =~ ^[1-9][0-9]*$ ]] || [[ "$pid" == 1 ]]; then
	printf 'Removed invalid server PID\n'
	rm -f "$pid_file"
	exit 0
fi

server_running() {
	local state
	kill -0 "$pid" 2>/dev/null || return 1
	state="$(ps -p "$pid" -o stat= 2>/dev/null)" || return 1
	[[ -n "$state" && "$state" != Z* ]]
}

if ! server_running; then
	printf 'Removed stale PID %s\n' "$pid"
	rm -f "$pid_file"
	exit 0
fi

# On Linux, reject a stale PID that now belongs to an unrelated executable.
# A running binary can be marked deleted after cargo replaces it during a build.
if [[ -d /proc/$pid ]]; then
	executable="$(readlink "/proc/$pid/exe" || true)"
	if [[ "$executable" != "$binary" && "$executable" != "$binary (deleted)" ]]; then
		printf 'PID %s is not the managed server; refusing to stop it.\n' "$pid" >&2
		exit 1
	fi
fi

kill -TERM "$pid" 2>/dev/null || ! server_running
for ((attempt = 0; attempt < 100; attempt++)); do
	server_running || break
	sleep 0.1
done

if server_running; then
	printf 'Server PID %s did not stop within 10 seconds; sending SIGKILL\n' "$pid"
	kill -KILL "$pid" 2>/dev/null || ! server_running
	for ((attempt = 0; attempt < 50; attempt++)); do
		server_running || break
		sleep 0.1
	done
fi

if server_running; then
	printf 'Server PID %s is still running; restart aborted.\n' "$pid" >&2
	exit 1
fi

rm -f "$pid_file"
printf 'Stopped server with PID %s\n' "$pid"
