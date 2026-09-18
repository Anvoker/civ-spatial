#!/usr/bin/env bash
# Run the three encodings for ONE (model, think) condition against a live OpenAI-compatible
# server (LM Studio). Parameterized via env vars. Persistent home for the run recipe so it
# survives across sessions (the job tmp dir does not).
#
#   MODEL=qwen3.6-35b-a3b-mtp THINK=off CROP=22,20,20,18 PK=6 OUT=results-big-off.jsonl \
#     bash scripts/live-eval.sh
#
# Launch with run_in_background: true — think-on can take an hour or more.
set -u
cd "$(dirname "$0")/.." || exit 1

MODEL="${MODEL:-qwen3.6-35b-a3b-mtp}"
THINK="${THINK:-off}"                 # off (reasoning_effort=none) | on
CROP="${CROP:-22,20,20,18}"           # x0,y0,w,h ; empty string = full board
PK="${PK:-6}"                         # questions per category
SEED="${SEED:-1}"
BASE="${BASE:-http://localhost:1234/v1}"
BOARD="${BOARD:-data/saves/myagent_T50.sav}"
OUT="${OUT:-results-${MODEL}-${THINK}.jsonl}"

cargo build -q -p civ-cli --features remote || exit 1
BIN="target/debug/civ.exe"

CROP_ARG=()
[ -n "$CROP" ] && CROP_ARG=(--crop "$CROP")

echo "### model=$MODEL think=$THINK crop=${CROP:-full} per-kind=$PK -> $OUT"
"$BIN" run --board "$BOARD" "${CROP_ARG[@]}" --seed "$SEED" --per-kind "$PK" \
  --encoding ascii --encoding raw --encoding adjacency \
  --model "$MODEL" --base-url "$BASE" --think "$THINK" --out "$OUT" 2>&1 | tail -30

echo "=== ANALYSIS ($OUT) ==="
python analysis/summarize.py "$OUT"
