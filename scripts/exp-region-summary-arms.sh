#!/usr/bin/env bash
# region_summary uptake experiment — 4-arm A/B on interactive-maxops (items 1/2/4).
# A=control(legacy desc) B=enriched-desc(default) C=drop scan_grid D=drop scan_grid + front-load roster.
# Same board/seed/kinds/fog as the 2026-08-07 smoke so arm A lines up with that baseline.
set -u
cd "$(dirname "$0")/.." || exit 1

# key from gitignored .env
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

run_arm () {  # $1=label  $2=out  $3..=extra flags
  local label="$1"; local out="$2"; shift 2
  echo "=== ARM $label -> $out  (flags: $*) ==="
  ./target/debug/civ.exe run "${COMMON[@]}" "$@" --out "$out"
  echo "=== ARM $label exit=$? ==="
}

run_arm A results-arm-A-legacy.jsonl      --legacy-rs-desc
run_arm B results-arm-B-redescribe.jsonl
run_arm C results-arm-C-noscangrid.jsonl  --withhold scan_grid
run_arm D results-arm-D-roster.jsonl      --withhold scan_grid --frontload-roster

echo "=== ALL ARMS DONE ==="
python analysis/summarize.py results-arm-A-legacy.jsonl results-arm-B-redescribe.jsonl \
  results-arm-C-noscangrid.jsonl results-arm-D-roster.jsonl || true
