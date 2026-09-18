#!/usr/bin/env bash
# Haiku 4.5 subscription RE-RUN — a faithful repeat of the 2026-08-24 exploratory 3-arm run, to
# account for the subscription path's non-determinism ("temperature"). Same boards, seeds, per-kind,
# arms, difficulty, fog, think-off as the original — EXCEPT cf-vacate is dropped (excluded from
# publication as corpus-degenerate; owner decision 2026-09-15).
#
# Runs N fully-independent passes into SEPARATE output stems (results-subs-arm-rerun-p1/p2/p3-...),
# so every item gets exactly N fresh attempts → clean majority vote, no ties. Does NOT touch the
# original results-subs-arm-*.jsonl (different stem). Each pass resumes ITS OWN files if re-invoked.
#
# Prereq: proxy up + Claude Max logged in (scripts/subscription-proxy-setup.md). An agent can RUN
# this but CANNOT start the proxy (interactive OAuth is user-only).
#
# Usage:
#   bash scripts/run-subs-arm-rerun.sh            # 3 passes (default)
#   PASSES=1 bash scripts/run-subs-arm-rerun.sh   # a single fresh pass
#
# Env overrides: PASSES(3) · MODEL · BASE · KEY · PK(8) · CONC(4).

set -u
cd "$(dirname "$0")/.." || exit 1

echo "[build] cargo build -p civ-cli --features remote"
cargo build -p civ-cli --features remote || { echo "build failed"; exit 1; }

[ -f .env ] && { set -a; source .env; set +a; }
CIV=./target/debug/civ.exe
BASE="${BASE:-http://localhost:8317/v1}"
KEY="${KEY:-${CLIPROXY_API_KEY:-civspatial-local}}"
MODEL="${MODEL:-claude-haiku-4-5-20251001}"
PK="${PK:-8}"
CONC="${CONC:-4}"
PASSES="${PASSES:-3}"

# Faithful reproduction of the original run's conditions:
THINK="off"                                     # recorded rows are "[nothink]"
CACHE="on"                                      # caching recovered (c0b8cd6); flat-rate anyway
ENC="--encoding raw --encoding raw-maxops --encoding interactive-maxops"   # ALL THREE arms
SEEDS=(1 2)
# cf-vacate REMOVED from both lists (owner: exclude from publication).
MID_KINDS="region-count,reachability,nearest-owned,reachable-nearest,city-defense,constraint-site,compare-two-attacks,t3-retreat,triage-reinforce,forward-posting"
LATE_KINDS="region-count,reachability,city-defense,constraint-site,compare-two-attacks,t3-retreat,triage-reinforce,forward-posting"
BOARDS=(
  "mid|corpus_medium_a3_s1337-T0120-Y00380-auto.sav|1|$MID_KINDS"
  "late|corpus_large_a6_s42-T0221-Y01600-final.sav|4|$LATE_KINDS"
)

# --- preflight: proxy MUST be up, or every call is a silent empty reply ----------------------------
echo "[preflight] probing proxy at $BASE ..."
models_json=$(curl -s -m 5 "$BASE/models" -H "Authorization: Bearer $KEY" 2>/dev/null)
if [ -z "$models_json" ]; then
  echo "[preflight] FAILED — nothing answering at $BASE. Start the proxy first:"
  echo "  cli-proxy-api --config scripts/cliproxy-config.yaml --claude-login   # one-time, browser"
  echo "  cli-proxy-api --config scripts/cliproxy-config.yaml                   # leave running"
  exit 1
fi
echo "[preflight] proxy up. Exposed models:"
printf '%s' "$models_json" | grep -o '"id":"[^"]*"' | sed 's/"id":"/  /; s/"$//'
printf '%s' "$models_json" | grep -q "\"$MODEL\"" \
  || echo "[preflight] note: '$MODEL' not in the list — swap an exact id if calls come back empty."
echo

mtag=$(printf '%s' "$MODEL" | sed 's#.*/##; s/[^a-zA-Z0-9]/_/g')
echo "[run] $PASSES passes · model=$MODEL · think=$THINK · cache=$CACHE · conc=$CONC · 3 arms · cf-vacate EXCLUDED"

for pass in $(seq 1 "$PASSES"); do
  STEM="results-subs-arm-rerun-p${pass}"
  echo "############## PASS $pass/$PASSES  (stem=$STEM) ##############"
  for spec in "${BOARDS[@]}"; do
    IFS='|' read -r label save fog kinds <<< "$spec"
    board="data/corpus/saves/$save"
    [ -f "$board" ] || { echo "!! board missing: $board — skipping $label"; continue; }
    for seed in "${SEEDS[@]}"; do
      inv_out="${STEM}-${label}-s${seed}-${mtag}.jsonl"
      if [ -s "$inv_out" ]; then mode="--resume"; else mode="--fresh"; fi
      echo "=== pass=$pass board=$label seed=$seed fog=$fog -> $inv_out ($mode) ==="
      "$CIV" run \
        --board "$board" --fog "$fog" \
        --seed "$seed" --per-kind "$PK" --kinds "$kinds" \
        $ENC --difficulty hard --think "$THINK" --cache "$CACHE" --concurrency "$CONC" \
        --model "$MODEL" --base-url "$BASE" --api-key "$KEY" \
        $mode --out "$inv_out" \
        || echo "!! invocation failed (pass=$pass board=$label seed=$seed) — re-run; it resumes this file"
    done
  done
done

echo "[done] files: results-subs-arm-rerun-p*-*.jsonl   (full LLM I/O under traces/)"
