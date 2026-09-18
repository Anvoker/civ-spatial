#!/usr/bin/env bash
# scan_region consolidation A/B/C REPLICATION on a 2nd board (2026-08-11).
# Original board: corpus_medium_a6_s1337-T0200 (mid-late medium). This run: corpus_large_a3_s42-T0220
# (late LARGE) -- biggest contrast on the size axis, the lever most likely to stress the perception surface.
# Same arms/kinds/fog/seed as exp-scanregion-abc.sh so the two boards are directly comparable.
#   A full = scan_region + scan + scan_grid + get_tile   (control)
#   B safe = scan_region + scan_grid                     (drop pinpoint get_tile+scan)
#   C bold = scan_region only                            (drop scan_grid too)
set -u
cd "$(dirname "$0")/.." || exit 1
set -a; source .env; set +a

echo "=== building civ-cli --features remote ==="
cargo build -p civ-cli --features remote || { echo "BUILD FAILED"; exit 1; }

BOARD=data/corpus/saves/corpus_large_a3_s42-T0220-Y01595-auto.sav
KINDS=forward-posting,fogged-assault,hidden-force,region-count,surprise-strike
COMMON=(--board "$BOARD" --seed 1 --per-kind 8 --difficulty hard --think off
        --cache on --concurrency 6 --fog 1 --kinds "$KINDS"
        --encoding interactive-maxops
        --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1
        --api-key "$OPENROUTER_API_KEY")

run_arm () { local label="$1"; local out="$2"; shift 2
  echo "=== ARM $label -> $out (flags: $*) ==="
  ./target/debug/civ.exe run "${COMMON[@]}" "$@" --out "$out"
  echo "=== ARM $label exit=$? ==="; }

run_arm SR-A-full results-sr-board2-A-full.jsonl
run_arm SR-B-safe results-sr-board2-B-safe.jsonl        --withhold get_tile --withhold scan
run_arm SR-C-only results-sr-board2-C-scanregiononly.jsonl --withhold get_tile --withhold scan --withhold scan_grid
echo "=== SCAN_REGION A/B/C BOARD2 DONE ==="
