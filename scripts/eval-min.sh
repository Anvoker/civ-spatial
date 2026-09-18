#!/usr/bin/env bash
# The MINIMAL harness — the 80/20 daily-iteration loop (see CONTINUITY / RESUME).
#
# The minimal harness cuts COST by running FEWER QUESTIONS (the saturated/duplicate kinds), while
# keeping the encoders we actually care about. So a run costs a fraction of the full battery but still
# exercises every encoder of interest and every discriminating comparison:
#   * encoders : raw + ascii + hierarchical   (raw = frontier; ascii = model-flipper; hierarchical =
#                the aggregation/scale contender we keep probing). Add more here as we build them.
#                Adjacency (dominated, ~4.3x raw's tokens) and egocentric are opt-in, not in the loop.
#   * kinds    : terrain, region-count, nearest   (encoding-discriminator, the counting wall, the
#                cross-model flipper). The six saturated/duplicate kinds are skipped via --kinds — THIS
#                is where the cost saving comes from (fewer questions, not fewer encoders).
#   * reasoning: think OFF (where encodings separate; think-on collapses the gap AND is ~6.4x slower).
#   * model    : DeepSeek V4 Flash on OpenRouter (cheapest; the only clean reasoning toggle).
#
# Full-battery runs (5 encoders x 9 kinds x multi-model x think on/off) become an occasional
# MILESTONE confirmation — not this loop. For those keep using scripts/live-eval.sh / a manual run.
#
# Usage:
#   OPENROUTER_API_KEY=sk-or-... bash scripts/eval-min.sh
# Override any default via env, e.g. a think-on confirmation on the T50 fixture:
#   OPENROUTER_API_KEY=sk-or-... THINK=on BOARD=data/saves/myagent_T50.sav bash scripts/eval-min.sh
#
# Launch with run_in_background: true — even minimal cloud runs take a few minutes.
set -u
cd "$(dirname "$0")/.." || exit 1

# Load the gitignored .env (OPENROUTER_API_KEY=...) if present, unless already in the environment.
if [ -f .env ] && [ -z "${OPENROUTER_API_KEY:-}" ]; then
  set -a; . ./.env; set +a
fi

# --- the minimal profile (all overridable) --------------------------------
MODEL="${MODEL:-deepseek/deepseek-v4-flash}"
BASE="${BASE:-https://openrouter.ai/api/v1}"
BOARD="${BOARD:-data/saves/testcontroller_T677.sav}"   # the dense headline board
KINDS="${KINDS:-terrain,region-count,nearest}"          # the discriminating set only (fewer questions)
ENCODINGS="${ENCODINGS:-raw ascii hierarchical}"        # the working set of interest (space-separated)
THINK="${THINK:-off}"                                   # off = cleanest encoding separation
DIFFICULTY="${DIFFICULTY:-hard}"
PK="${PK:-12}"                                          # questions per kind
SEED="${SEED:-1}"
CACHE="${CACHE:-on}"                                    # board-prefix caching (harmless on DeepSeek)
CONCURRENCY="${CONCURRENCY:-8}"                          # cloud parallelism (~5x wall-clock)
PROVIDER="${PROVIDER:-}"                                # optional: pin an OpenRouter upstream
OUT="${OUT:-results-min-${THINK}.jsonl}"
API_KEY="${OPENROUTER_API_KEY:-${API_KEY:-}}"

if [ -z "$API_KEY" ]; then
  echo "ERROR: no API key. Set OPENROUTER_API_KEY=sk-or-... (or API_KEY=...)." >&2
  exit 1
fi

cargo build -q -p civ-cli --features remote || exit 1
BIN="target/debug/civ.exe"

ENC_ARGS=()
for e in $ENCODINGS; do ENC_ARGS+=(--encoding "$e"); done
PROVIDER_ARG=()
[ -n "$PROVIDER" ] && PROVIDER_ARG=(--provider "$PROVIDER")

echo "### MIN  model=$MODEL think=$THINK kinds=$KINDS enc=[$ENCODINGS] board=$(basename "$BOARD") pk=$PK -> $OUT"
"$BIN" run --board "$BOARD" --seed "$SEED" --per-kind "$PK" \
  --kinds "$KINDS" "${ENC_ARGS[@]}" \
  --model "$MODEL" --base-url "$BASE" --api-key "$API_KEY" \
  --think "$THINK" --difficulty "$DIFFICULTY" --cache "$CACHE" \
  --concurrency "$CONCURRENCY" "${PROVIDER_ARG[@]}" \
  --out "$OUT" 2>&1 | tail -40

echo "=== ANALYSIS ($OUT) ==="
python analysis/summarize.py "$OUT"
