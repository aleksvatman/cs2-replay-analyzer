#!/usr/bin/env bash
# Load test of the release server: uploads several demos at once and reports how long each
# upload and the whole run took, and the peak memory (RSS) of the server process.
#
#   cargo build --release -p server
#   server/scripts/load-test.sh PARALLEL_PARSES DEM [DEM...]
#
# Demos with identical bytes are one match (duplicate check), so pass different files.
# `append-byte` below shows how to make a distinct copy of a demo.
set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 PARALLEL_PARSES DEM [DEM...]" >&2
  exit 2
fi
parallel="$1"; shift
root="$(cd "$(dirname "$0")/../.." && pwd)"
bin="$root/target/release/server"
[ -x "$bin" ] || { echo "build first: cargo build --release -p server" >&2; exit 1; }

work="$(mktemp -d)"
port=$((20000 + RANDOM % 20000))
base="http://127.0.0.1:$port"
cleanup() {
  [ -n "${pid:-}" ] && kill -TERM "$pid" 2>/dev/null || true
  [ -n "${sampler:-}" ] && wait "$sampler" 2>/dev/null || true  # it ends when the server does
  rm -rf "$work"
}
trap cleanup EXIT

BIND_ADDR="127.0.0.1:$port" DATA_DIR="$work/data" MAX_PARALLEL_PARSES="$parallel" \
  MAX_UPLOADS=8 MAX_QUEUE=8 RATE_LIMIT_PER_MINUTE=0 "$bin" >"$work/log" 2>&1 &
pid=$!
for _ in $(seq 1 50); do curl -sf "$base/health" >/dev/null 2>&1 && break; sleep 0.1; done

( while kill -0 "$pid" 2>/dev/null; do ps -o rss= -p "$pid"; sleep 0.1; done >"$work/rss" ) &
sampler=$!

start=$(python3 -c 'import time;print(time.time())')
n=0
for dem in "$@"; do
  n=$((n + 1))
  (
    t0=$(python3 -c 'import time;print(time.time())')
    out=$(curl -s -H "Content-Type: application/octet-stream" --data-binary @"$dem" "$base/matches")
    t1=$(python3 -c 'import time;print(time.time())')
    echo "$out" | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])' >"$work/id.$n"
    python3 -c "print(round($t1 - $t0, 2))" >"$work/up.$n"
  ) &
done
wait_uploads() { while [ "$(ls "$work"/up.* 2>/dev/null | wc -l)" -lt "$n" ]; do sleep 0.1; done; }
wait_uploads
uploaded=$(python3 -c "import time;print(round(time.time() - $start, 2))")

while :; do
  states=$(curl -s "$base/matches" | python3 -c 'import sys,json;print(" ".join(m["status"] for m in json.load(sys.stdin)))')
  case " $states " in *" queued "*|*" parsing "*) sleep 0.5 ;; *) break ;; esac
done
total=$(python3 -c "import time;print(round(time.time() - $start, 2))")
peak_kb=$(sort -n "$work/rss" | tail -1 | tr -d ' ')

echo "parallel parses: $parallel, demos: $n"
for i in $(seq 1 "$n"); do
  id=$(cat "$work/id.$i")
  echo "  demo $i ($(basename "${!i}")): upload $(cat "$work/up.$i") s, status $(curl -s "$base/matches/$id" | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["status"],d.get("rounds"),d.get("score"))')"
done
echo "all uploads received after: $uploaded s"
echo "everything finished after:  $total s"
echo "peak server RSS: $((peak_kb / 1024)) MB"
sed 's/\x1b\[[0-9;]*m//g' "$work/log" | grep -E "parsing (done|failed)" | sed 's/^.*INFO //'
