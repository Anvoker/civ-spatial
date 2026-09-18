#!/usr/bin/env bash
# constraint-site Variant C LIVE confirmation (2026-08-11).
# Question: does Variant C actually move the model OFF the "call site_check and read off" shortcut?
#   site_check reports 3 of the 4 constraints but NOT the >=CITY_MIN_SPACING spacing rule, so a model
#   that just trusts site_check.ok will place cities too close and answer wrong on the spacing-binding
#   items. Offline can't tell; this needs a live trajectory.
# Single arm, interactive-maxops, constraint-site only. Inspect accuracy + whether trajectories reason
# about spacing (list_cities/roster + Chebyshev) beyond site_check.
set -u
cd "$(dirname "$0")/.." || exit 1
set -a; source .env; set +a

echo "=== building civ-cli --features remote ==="
cargo build -p civ-cli --features remote || { echo "BUILD FAILED"; exit 1; }

BOARD=data/corpus/saves/corpus_medium_a6_s1337-T0200-Y01490-auto.sav
./target/debug/civ.exe run \
  --board "$BOARD" --seed 1 --per-kind 16 --difficulty hard --think off \
  --cache on --concurrency 6 --fog 1 --kinds constraint-site \
  --encoding interactive-maxops \
  --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1 \
  --api-key "$OPENROUTER_API_KEY" \
  --out results-constraint-site-live.jsonl
echo "=== constraint-site live exit=$? ==="
