#!/usr/bin/env bash
# 16-tool scan_grid drop + budget-aware re-baseline (2026-08-11).
# Two things at once, both under the NEW budget-aware prompt (up-front turn limit + [turn n/N] stamps):
#   1. scan_grid drop: board2 replication showed scan_grid's ONLY accuracy justification (hidden-force
#      -1/8 on the medium board) did NOT replicate. Test a 16-tool "scan_region only" surface vs the
#      current 17-tool surface on hidden-force (+ region-count control) across BOTH boards.
#   2. re-baseline: forward-posting went budget-marginal at scale and timed out with empty answers.
#      With the model now told the turn limit and given a running counter, do those non-answers convert
#      to real (scored) answers? Include forward-posting to measure the error-rate change.
# Arms: KEEP = 17 tools (scan_grid present) ; DROP = `--withhold scan_grid` (scan_region only, 16 tools).
set -u
cd "$(dirname "$0")/.." || exit 1
set -a; source .env; set +a

echo "=== building civ-cli --features remote ==="
cargo build -p civ-cli --features remote || { echo "BUILD FAILED"; exit 1; }

BOARD_MED=data/corpus/saves/corpus_medium_a6_s1337-T0200-Y01490-auto.sav
BOARD_LRG=data/corpus/saves/corpus_large_a3_s42-T0220-Y01595-auto.sav
KINDS=hidden-force,forward-posting,region-count
COMMON=(--seed 1 --per-kind 8 --difficulty hard --think off --cache on --concurrency 6 --fog 1
        --kinds "$KINDS" --encoding interactive-maxops
        --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1
        --api-key "$OPENROUTER_API_KEY")

run_arm () { local label="$1"; local board="$2"; local out="$3"; shift 3
  echo "=== ARM $label (board=$(basename "$board")) -> $out (flags: $*) ==="
  ./target/debug/civ.exe run --board "$board" "${COMMON[@]}" "$@" --out "$out"
  echo "=== ARM $label exit=$? ==="; }

run_arm MED-KEEP "$BOARD_MED" results-sgd-med-keep.jsonl
run_arm MED-DROP "$BOARD_MED" results-sgd-med-drop.jsonl --withhold scan_grid
run_arm LRG-KEEP "$BOARD_LRG" results-sgd-lrg-keep.jsonl
run_arm LRG-DROP "$BOARD_LRG" results-sgd-lrg-drop.jsonl --withhold scan_grid
echo "=== SCAN_GRID DROP + BUDGET-AWARE DONE ==="
