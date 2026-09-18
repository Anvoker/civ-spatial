#!/usr/bin/env bash
# Subscription arm runner — mirrors run-3arm-2026-08-18.sh, but bills against your Claude Max +
# ChatGPT/Codex subscriptions through the local CLIProxyAPI (:8317) instead of OpenRouter.
#
# This is an EXPLORATORY arm, not a preregistered one (subscription backends inject a hidden system
# prompt, may ignore temperature, and may not honor --think off — see scripts/subscription-proxy-setup.md).
#
# Default invocation = a CHEAP SMOKE TEST: cheapest model per provider, 1 small board, 1 seed,
# per-kind 1, the 3 discriminating kinds, static encoders only. Scale to a fuller arm via env vars.
#
# Prereq: proxy running + both subscriptions logged in (subscription-proxy-setup.md).
# Usage:
#   bash scripts/run-subs-arm.sh              # cheap smoke (default)
#   SMOKE=0 PK=8 bash scripts/run-subs-arm.sh # fuller arm (both boards, 2 seeds, per-kind 8)
#
# Env overrides: SMOKE(1) · SUB_MODELS · BASE · KEY · PK · THINK(on) · CONC(2) · ENC · OUT_STEM.

set -u
cd "$(dirname "$0")/.." || exit 1

echo "[build] cargo build -p civ-cli --features remote"
cargo build -p civ-cli --features remote || { echo "build failed"; exit 1; }

[ -f .env ] && { set -a; source .env; set +a; }
CIV=./target/debug/civ.exe
BASE="${BASE:-http://localhost:8317/v1}"
KEY="${KEY:-${CLIPROXY_API_KEY:-civspatial-local}}"
# Claude Max only for now (OpenAI/Codex not set up — add later: --codex-login, SUB_MODELS+=" gpt-5-mini").
# Cheapest Claude tier = the most recent Haiku (4.5). Confirm the exact id the subscription exposes
# against `curl $BASE/models` — the preflight below prints it. If Haiku isn't listed, use the
# `claude-code` alias (routes to the subscription default) or whatever Claude id IS listed.
SUB_MODELS="${SUB_MODELS:-claude-haiku-4-5-20251001}"
THINK="${THINK:-on}"          # safe default for reasoning-only subscription backends
CONC="${CONC:-2}"
SMOKE="${SMOKE:-1}"
# Cache OFF by default: the proxy translates to the native Anthropic Messages API, which rejects
# cache_control inside tool_result.content (the harness's moving breakpoint) → the interactive arms
# 400. Subscription billing is flat-rate anyway, so prompt caching (a cost-axis optimization) is moot.
CACHE="${CACHE:-off}"

# --- preflight: the proxy MUST be up, or every call is a silent empty reply -----------------------
echo "[preflight] probing proxy at $BASE ..."
models_json=$(curl -s -m 5 "$BASE/models" -H "Authorization: Bearer $KEY" 2>/dev/null)
if [ -z "$models_json" ]; then
  cat <<EOF
[preflight] FAILED — nothing answering at $BASE.
Start the proxy and log in to your subscriptions first (one-time, browser OAuth):
  cli-proxy-api --config scripts/cliproxy-config.yaml --claude-login
  cli-proxy-api --config scripts/cliproxy-config.yaml --codex-login
  cli-proxy-api --config scripts/cliproxy-config.yaml       # leave running, then re-run this script
See scripts/subscription-proxy-setup.md.
EOF
  exit 1
fi
echo "[preflight] proxy is up. Models it exposes:"
printf '%s' "$models_json" | grep -o '"id":"[^"]*"' | sed 's/"id":"/  /; s/"$//'
# Warn (don't block) if a requested model id isn't in the list — routing aliases may still work.
for model in $SUB_MODELS; do
  printf '%s' "$models_json" | grep -q "\"$model\"" \
    || echo "[preflight] note: '$model' not in the model list — swap in an exact id above if calls come back empty."
done
echo

# --- experiment matrix ----------------------------------------------------------------------------
ENC="${ENC:---encoding raw --encoding raw-maxops}"   # static arms; skip the interactive tool loop to stay cheap
OUT_STEM="${OUT_STEM:-results-subs-arm}"

if [ "$SMOKE" = "1" ]; then
  PK="${PK:-1}"
  SEEDS=(1)
  # board_label | save | fog | kinds
  BOARDS=( "mid|corpus_medium_a3_s1337-T0120-Y00380-auto.sav|1|terrain,region-count,nearest" )
  echo "[mode] CHEAP SMOKE — cheapest models, 1 board, 1 seed, per-kind $PK, 3 discriminating kinds."
else
  PK="${PK:-8}"
  SEEDS=(1 2)
  MID_KINDS="region-count,reachability,nearest-owned,reachable-nearest,city-defense,constraint-site,compare-two-attacks,t3-retreat,cf-vacate,triage-reinforce,forward-posting"
  LATE_KINDS="region-count,reachability,city-defense,constraint-site,compare-two-attacks,t3-retreat,cf-vacate,triage-reinforce,forward-posting"
  BOARDS=(
    "mid|corpus_medium_a3_s1337-T0120-Y00380-auto.sav|1|$MID_KINDS"
    "late|corpus_large_a6_s42-T0221-Y01600-final.sav|4|$LATE_KINDS"
  )
  echo "[mode] FULL ARM — per-kind $PK, ${#BOARDS[@]} boards, ${#SEEDS[@]} seeds."
fi

echo "[run] proxy=$BASE  models: $SUB_MODELS  think=$THINK  concurrency=$CONC  stem=$OUT_STEM"
for spec in "${BOARDS[@]}"; do
  IFS='|' read -r label save fog kinds <<< "$spec"
  board="data/corpus/saves/$save"
  [ -f "$board" ] || { echo "!! board missing: $board — skipping $label"; continue; }
  for seed in "${SEEDS[@]}"; do
    for model in $SUB_MODELS; do
      # One results file per (board, seed, model) — isolated resume scope, same as run-3arm.
      mtag=$(printf '%s' "$model" | sed 's#.*/##; s/[^a-zA-Z0-9]/_/g')
      inv_out="${OUT_STEM}-${label}-s${seed}-${mtag}.jsonl"
      if [ -s "$inv_out" ]; then mode="--resume"; else mode="--fresh"; fi
      echo "=== board=$label seed=$seed model=$model fog=$fog think=$THINK -> $inv_out ($mode) ==="
      "$CIV" run \
        --board "$board" --fog "$fog" \
        --seed "$seed" --per-kind "$PK" --kinds "$kinds" \
        $ENC --difficulty hard --think "$THINK" --cache "$CACHE" --concurrency "$CONC" \
        --model "$model" --base-url "$BASE" --api-key "$KEY" \
        $mode --out "$inv_out" \
        || echo "!! invocation failed (board=$label seed=$seed model=$model) — re-run; it resumes this file"
    done
  done
done

echo "[done] files: ${OUT_STEM}-*.jsonl   (full LLM I/O traced under traces/)"
echo "Analyze: python analysis/summarize.py ${OUT_STEM}-*.jsonl"
