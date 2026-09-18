#!/usr/bin/env bash
# Driver: run the generated corpus autogames headless, collect the autosaves.
#
# For each data/corpus/scripts/*.serv this launches:
#     freeciv-server -e -q 30 --Announce none -p <port> -s <savedir> -r <script>
#   -e            exit when the game ends (endturn reached)
#   -q 30         also quit if idle 30s with no players (safety net)
#   --Announce none   don't touch the metaserver (fully offline)
#   -p <port>     a fixed port per run so parallel/retried runs don't clash
#   -s <savedir>  where autosaves land
#   -r <script>   the startup script (ends in `start`)
#
# The .serv scripts already set rulesetdir/seeds/saveturns/endturn, so this driver
# is intentionally dumb: find the binary, run each script, report. After it finishes,
# run scripts/build-manifest.py to characterize the collected boards.
#
# Usage:
#   bash scripts/run-corpus.sh                     # run every script in the scripts dir
#   SCRIPTS_DIR=data/corpus/scripts SAVES_DIR=data/corpus/saves bash scripts/run-corpus.sh
#   FREECIV_SERVER="/c/Program Files/Freeciv/freeciv-server.exe" bash scripts/run-corpus.sh
#   ONLY=corpus_small_a3_s42 bash scripts/run-corpus.sh   # run just one cell
#
# Launch with run_in_background: true -- a full sweep to endturn 220 is many minutes.
set -u
cd "$(dirname "$0")/.." || exit 1

SCRIPTS_DIR="${SCRIPTS_DIR:-data/corpus/scripts}"
SAVES_DIR="${SAVES_DIR:-data/corpus/saves}"
BASE_PORT="${BASE_PORT:-5560}"
ONLY="${ONLY:-}"

# --- locate the server binary (env override wins, else PATH, else known install dirs) ---
find_server() {
  if [ -n "${FREECIV_SERVER:-}" ]; then echo "$FREECIV_SERVER"; return; fi
  for name in freeciv-server civserver; do
    if command -v "$name" >/dev/null 2>&1; then command -v "$name"; return; fi
  done
  # The Windows installer uses a VERSIONED dir (e.g. Freeciv-3.2.5-win64-10-client-gtk3.22),
  # not a bare Freeciv/. Glob both, newest first, under Program Files (+ x86).
  for base in "/c/Program Files" "/c/Program Files (x86)"; do
    for cand in $(ls -d "$base"/Freeciv*/freeciv-server.exe "$base"/Freeciv*/civserver.exe \
                        "$base"/Freeciv/freeciv-server.exe 2>/dev/null | sort -r); do
      if [ -x "$cand" ]; then echo "$cand"; return; fi
    done
  done
  echo ""
}

SERVER="$(find_server)"
if [ -z "$SERVER" ]; then
  echo "ERROR: freeciv-server not found. Set FREECIV_SERVER=/path/to/freeciv-server.exe" >&2
  echo "       (checked PATH + C:\\Program Files\\Freeciv*\\). Freeciv install pending?" >&2
  exit 2
fi
echo "server: $SERVER"

# Freeciv must find its rulesets. When launched from our cwd it won't look in the install
# dir automatically, so point FREECIV_DATA_PATH at the binary's sibling data/ unless already
# set. (Symptom if missing: "Could not find a readable game.ruleset ...".)
if [ -z "${FREECIV_DATA_PATH:-}" ]; then
  data_dir="$(dirname "$SERVER")/data"
  if [ -d "$data_dir" ]; then
    export FREECIV_DATA_PATH="$data_dir"
    echo "data:   $FREECIV_DATA_PATH"
  fi
fi

mkdir -p "$SAVES_DIR"
shopt -s nullglob
scripts=("$SCRIPTS_DIR"/*.serv)
if [ ${#scripts[@]} -eq 0 ]; then
  echo "ERROR: no .serv scripts in $SCRIPTS_DIR -- run scripts/gen-corpus-scripts.py first." >&2
  exit 2
fi

port=$BASE_PORT
ran=0
for script in "${scripts[@]}"; do
  stem="$(basename "$script" .serv)"
  if [ -n "$ONLY" ] && [ "$stem" != "$ONLY" ]; then continue; fi
  echo "=== [$stem] port=$port -> $SAVES_DIR ==="
  # -e exit-on-end (clean exit at endturn); --Announce none offline; per-script port.
  # NOTE: no -q/--quitidle -- an all-AI autogame has zero client connections, so quitidle
  # would kill the game mid-run; -e handles the clean exit at endturn instead.
  "$SERVER" -e --Announce none -p "$port" -s "$SAVES_DIR" -r "$script"
  rc=$?
  echo "    exit=$rc  saves now: $(ls -1 "$SAVES_DIR"/${stem}* 2>/dev/null | wc -l)"
  port=$((port + 1))
  ran=$((ran + 1))
done

echo "ran $ran autogame(s). Autosaves in $SAVES_DIR/"
echo "next: python scripts/build-manifest.py --saves-dir $SAVES_DIR"
