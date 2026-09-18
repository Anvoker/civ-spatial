#!/usr/bin/env bash
set -a; source .env; set +a
cd /c/Projects/CivSpatial
KINDS="settle-site best-site region-count terrain nearest-owned reachable-nearest"
first=1
for k in $KINDS; do
  if [ "$first" -eq 0 ]; then echo "sleeping 60s to respect new-account rpm..."; sleep 60; fi
  first=0
  echo "=== running kind $k ==="
  ./target/debug/civ.exe run --board data/saves/testcontroller_T677.sav \
    --fog 4 --seed 1 --per-kind 8 --difficulty hard --think off --cache on --concurrency 4 \
    --kinds "$k" --encoding raw \
    --model openai/gpt-5.6-sol --base-url https://openrouter.ai/api/v1 \
    --provider OpenAI --api-key "$OPENROUTER_API_KEY" \
    --out "results-frontier-k-$k.jsonl" 2>&1 | tail -3
done
echo "=== BATCH COMPLETE ==="
