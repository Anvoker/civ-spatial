#!/usr/bin/env bash
# The 3-arm clean run — frozen spec: analysis/run-preregistration-3arm-2026-08-17.md
#
# raw / raw-maxops / roster-maxops(=interactive-maxops) x 11 kinds x 2 boards x
# 2 models x think-off x K=8 x 2 seeds  (~1,920 trials).
#
# Usage:
#   PK=1 bash scripts/run-3arm-2026-08-18.sh     # PRE-FLIGHT (per-kind 1, tiny cost, calibrates spend/wall-clock)
#   bash scripts/run-3arm-2026-08-18.sh          # FULL RUN (per-kind 8)
# Re-run either verbatim after a crash: --resume skips completed rows (6-key: encoding,item,board,model,think,effort).
#
# Env overrides: PK (per-kind, default 8) · OUT (results file) · CONC (concurrency, default 4) ·
#   MODELS (space-separated slugs, default deepseek+gemini).

set -u
cd "$(dirname "$0")/.." || exit 1

# RESUME GOTCHA: `just check`/plain cargo build strip --features remote → the binary then dies
# "needs a live provider". Always rebuild remote right before the run.
echo "[build] cargo build -p civ-cli --features remote"
cargo build -p civ-cli --features remote || { echo "build failed"; exit 1; }

# Key from the gitignored .env (never on the command line).
set -a; source .env; set +a
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY missing from .env}"

CIV=./target/debug/civ.exe
BASE=https://openrouter.ai/api/v1
PK="${PK:-8}"
CONC="${CONC:-4}"
MODELS="${MODELS:-deepseek/deepseek-v4-flash google/gemini-2.5-flash}"
OUT="${OUT:-results-3arm-2026-08-18.jsonl}"
[ "$PK" != "8" ] && OUT="${OUT%.jsonl}-pk${PK}.jsonl"   # keep pre-flight rows out of the full-run file
OUT_STEM="${OUT%.jsonl}"   # per-invocation files derive from this stem (glob it for analysis)

ENC="--encoding raw --encoding raw-maxops --encoding interactive-maxops"
MID_KINDS="region-count,reachability,nearest-owned,reachable-nearest,city-defense,constraint-site,compare-two-attacks,t3-retreat,cf-vacate,triage-reinforce,forward-posting"
LATE_KINDS="region-count,reachability,city-defense,constraint-site,compare-two-attacks,t3-retreat,cf-vacate,triage-reinforce,forward-posting"

# board_label | save | fog-player | kinds   (movement kinds excluded on the late board — decision E)
BOARDS=(
  "mid|corpus_medium_a3_s1337-T0120-Y00380-auto.sav|1|$MID_KINDS"
  "late|corpus_large_a6_s42-T0221-Y01600-final.sav|4|$LATE_KINDS"
)
SEEDS=(1 2)

echo "[run] PK=$PK  stem=$OUT_STEM  concurrency=$CONC  models: $MODELS"
for spec in "${BOARDS[@]}"; do
  IFS='|' read -r label save fog kinds <<< "$spec"
  for seed in "${SEEDS[@]}"; do
    for model in $MODELS; do
      # One results file PER (board, seed, model): each is an isolated resume scope, so the
      # sink's truth table behaves — Fresh on a new file, Resume to fill a crashed one. A shared
      # file would truncate across seeds (differing item ids => 0 key matches => Default wipes).
      mtag=$(printf '%s' "$model" | sed 's#.*/##; s/-.*//')     # deepseek-v4-flash -> deepseek
      inv_out="${OUT_STEM}-${label}-s${seed}-${mtag}.jsonl"
      if [ -s "$inv_out" ]; then mode="--resume"; else mode="--fresh"; fi
      echo "=== board=$label seed=$seed model=$model fog=$fog -> $inv_out ($mode) ==="
      "$CIV" run \
        --board "data/corpus/saves/$save" --fog "$fog" \
        --seed "$seed" --per-kind "$PK" --kinds "$kinds" \
        $ENC --difficulty hard --think off --cache on --concurrency "$CONC" \
        --model "$model" --base-url "$BASE" --api-key "$OPENROUTER_API_KEY" \
        $mode --out "$inv_out" \
        || echo "!! invocation failed (board=$label seed=$seed model=$model) — re-run the script; it resumes this file"
    done
  done
done

echo "[done] files: ${OUT_STEM}-*.jsonl"
echo "Analyze: python analysis/summarize.py ${OUT_STEM}-*.jsonl"
