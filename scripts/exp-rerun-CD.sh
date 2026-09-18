#!/usr/bin/env bash
# Re-run arms C and D ONLY, after the withhold prose-scrub fix (f1ef766).
# A/B withheld nothing → they were unconfounded and stand. C/D re-run with scan_grid now
# scrubbed from the PROMPT (not just the tools array). Same config as the original sweep.
set -u
cd "$(dirname "$0")/.." || exit 1
set -a; source .env; set +a

echo "=== building civ-cli --features remote ==="
cargo build -p civ-cli --features remote || { echo "BUILD FAILED"; exit 1; }

BOARD=data/corpus/saves/corpus_medium_a6_s1337-T0200-Y01490-auto.sav
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

run_arm C-fixed results-arm-C-noscangrid-fixed.jsonl --withhold scan_grid
run_arm D-fixed results-arm-D-roster-fixed.jsonl     --withhold scan_grid --frontload-roster
echo "=== C/D RE-RUN DONE ==="
