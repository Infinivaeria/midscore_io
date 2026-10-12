#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
pidfile="$root/tmp/run-wasm-https.pid"
logfile="$root/log/run-wasm-https.log"

mkdir -p "$root/tmp" "$root/log"

stop_process() {
  process_pidfile=$1
  process_name=$2

  [ -s "$process_pidfile" ] || return 0
  process_pid=$(cat "$process_pidfile")
  case "$process_pid" in
    ''|*[!0-9]*|0|1) return 0 ;;
  esac
  [ "$process_pid" -gt 1 ] 2>/dev/null || return 0
  kill -0 "$process_pid" 2>/dev/null || return 0

  printf '%s\n' "Stopping $process_name (PID $process_pid)."
  kill "$process_pid" 2>/dev/null || true
  attempts=0
  while kill -0 "$process_pid" 2>/dev/null; do
    if [ "$attempts" -eq 10 ]; then
      printf '%s\n' "Force-stopping $process_name (PID $process_pid)."
      kill -KILL "$process_pid" 2>/dev/null || true
    elif [ "$attempts" -ge 15 ]; then
      printf '%s\n' "Could not stop $process_name (PID $process_pid)." >&2
      return 1
    fi
    sleep 1
    attempts=$((attempts + 1))
  done
}

stop_process "$pidfile" 'Wasm HTTPS deployment'
stop_process "$root/config/puma.pid" 'Puma HTTPS server'

rm -f "$pidfile"
nohup "$root/bin/run-wasm-https" >"$logfile" 2>&1 < /dev/null &
printf '%s\n' "$!" > "$pidfile"
printf '%s\n' "Wasm HTTPS deployment started in the background (PID $!)."
printf '%s\n' "Follow progress with: tail -f $logfile"
