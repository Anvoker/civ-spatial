#!/usr/bin/env bash
# scan_grid KEEP/DROP re-test AFTER the scan_region fog fix (2026-08-11).
# scan_region now LISTS fogged-but-explored tiles (terrain/owner/(fogged), units hidden) instead of
# dropping them as "Unknown". Question: does scan_region-only (DROP, 16 tools) now MATCH KEEP (17 tools)
# on hidden-force? If yes, scan_grid's hidden-force value was really a scan_region fidelity gap and
# scan_grid becomes genuinely droppable. Same conditions as exp-scangrid-drop-budgetaware.sh (budget-
# aware prompt), fresh output files (-sgd2-) so the PRE-fix run stays as the baseline.
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

run_arm MED-KEEP "$BOARD_MED" results-sgd2-med-keep.jsonl
run_arm MED-DROP "$BOARD_MED" results-sgd2-med-drop.jsonl --withhold scan_grid
run_arm LRG-KEEP "$BOARD_LRG" results-sgd2-lrg-keep.jsonl
run_arm LRG-DROP "$BOARD_LRG" results-sgd2-lrg-drop.jsonl --withhold scan_grid
echo "=== SCAN_GRID DROP (post fog-fix) DONE ==="
