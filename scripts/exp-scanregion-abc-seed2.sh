#!/usr/bin/env bash
# scan_region consolidation A/B/C — SEED 2 replication of the seed-1 result.
# Same arms/board/kinds/fog; only --seed changes, for a clean replication.
set -u
cd "$(dirname "$0")/.." || exit 1
set -a; source .env; set +a

echo "=== building civ-cli --features remote ==="
cargo build -p civ-cli --features remote || { echo "BUILD FAILED"; exit 1; }

BOARD=data/corpus/saves/corpus_medium_a6_s1337-T0200-Y01490-auto.sav
KINDS=forward-posting,fogged-assault,hidden-force,region-count,surprise-strike
COMMON=(--board "$BOARD" --seed 2 --per-kind 8 --difficulty hard --think off
        --cache on --concurrency 6 --fog 1 --kinds "$KINDS"
        --encoding interactive-maxops
        --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1
        --api-key "$OPENROUTER_API_KEY")

run_arm () { local label="$1"; local out="$2"; shift 2
  echo "=== ARM $label -> $out (flags: $*) ==="
  ./target/debug/civ.exe run "${COMMON[@]}" "$@" --out "$out"
  echo "=== ARM $label exit=$? ==="; }

run_arm SR2-A-full results-sr2-A-full.jsonl
run_arm SR2-B-safe results-sr2-B-safe.jsonl        --withhold get_tile --withhold scan
run_arm SR2-C-only results-sr2-C-scanregiononly.jsonl --withhold get_tile --withhold scan --withhold scan_grid
echo "=== SCAN_REGION SEED2 A/B/C DONE ==="
