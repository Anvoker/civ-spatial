#!/usr/bin/env bash
# Fidelity smoke-test for the subscription path (CLIProxyAPI on :8317).
#
# Answers the two questions that decide whether subscription results are trustworthy BEFORE you
# spend a real run on them:
#   1. Does the model reply at all through the proxy? (base-url / login / model-id sanity)
#   2. Is it deterministic at temperature 0, and does --think off work?  Subscription models are
#      reasoning models (GPT-5/Codex, Claude via Claude Code); the proxy may ignore temperature and
#      may reject reasoning_effort:"none" (what the harness emits on --think off). If either is true,
#      these results are NOT comparable to the OpenRouter arms (which pin temp 0 + think off).
#
# Prereq: the proxy is running and BOTH subscriptions are logged in — see
#         scripts/subscription-proxy-setup.md. Then:  bash scripts/subs-ping.sh
#
# Env overrides: SUB_MODELS (space-separated ids) · BASE · KEY · N (determinism repeats, default 3).

set -u
cd "$(dirname "$0")/.." || exit 1

echo "[build] cargo build -p civ-cli --features remote"
cargo build -p civ-cli --features remote || { echo "build failed"; exit 1; }

# Downstream proxy key (NOT a subscription secret): matches api-keys in cliproxy-config.yaml.
[ -f .env ] && { set -a; source .env; set +a; }
CIV=./target/debug/civ.exe
BASE="${BASE:-http://localhost:8317/v1}"
KEY="${KEY:-${CLIPROXY_API_KEY:-civspatial-local}}"
N="${N:-3}"
# Claude Max only for now. Swap in the exact id from `curl $BASE/models` (printed below) for a clean
# label; `claude-code` also works as an alias that routes to the subscription default.
SUB_MODELS="${SUB_MODELS:-claude-haiku-4-5-20251001}"

echo "[models] available through the proxy:"
curl -s "$BASE/models" -H "Authorization: Bearer $KEY" \
  | grep -o '"id":"[^"]*"' | sed 's/"id":"/  /; s/"$//' || echo "  (couldn't list — is the proxy up?)"
echo

for model in $SUB_MODELS; do
  echo "=================================================================="
  echo "MODEL: $model"
  for think in on off; do
    echo "--- think=$think : pong + determinism x$N (temperature 0) ---"
    prev=""; stable=1
    for i in $(seq 1 "$N"); do
      out=$("$CIV" ping-model --base-url "$BASE" --model "$model" --api-key "$KEY" --think "$think" 2>&1)
      reply=$(printf '%s' "$out" | sed -n 's/^reply: //p')
      echo "  run $i: $reply"
      [ -z "$reply" ] || [ "$reply" = "\"\"" ] && { echo "    !! EMPTY reply — think=$think may be rejected by this backend"; stable=0; }
      [ -n "$prev" ] && [ "$reply" != "$prev" ] && stable=0
      prev="$reply"
    done
    [ "$stable" = 1 ] && echo "    => deterministic + non-empty at think=$think" \
                      || echo "    => NON-deterministic or empty at think=$think (see caveats in setup.md)"
  done
done

echo
echo "[done] If think=off is empty/unstable for a model, that arm can't match the OpenRouter"
echo "       prereg condition — treat its subscription results as exploratory only."
