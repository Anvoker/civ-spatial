#!/usr/bin/env bash
# Turnkey tear-down for the local Langfuse stack.
#
# Usage:
#   bash observability/langfuse-down.sh              # stop + remove containers, KEEP data
#   bash observability/langfuse-down.sh --volumes    # ALSO delete volumes (full reset)
#
# Without --volumes the named volumes (postgres/clickhouse/minio data) survive, so a
# later `langfuse-up.sh` restores your account, project, keys and traces.
# With --volumes everything is wiped -- you start fresh (new account + new API keys).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"

WIPE=""
case "${1:-}" in
  --volumes|-v)
    WIPE="--volumes"
    echo "WARNING: --volumes given: postgres/clickhouse/minio data will be DELETED."
    ;;
  "")
    ;;
  *)
    echo "Unknown argument: $1 (expected nothing or --volumes)" >&2
    exit 2
    ;;
esac

if ! docker info >/dev/null 2>&1; then
  echo "Docker daemon not reachable -- nothing to tear down (or start Docker Desktop first)." >&2
  exit 0
fi

echo "Stopping and removing the Langfuse stack..."
docker compose -f "$COMPOSE_FILE" down ${WIPE}

if [ -n "$WIPE" ]; then
  echo "Done. Containers removed and volumes wiped (fresh start next time)."
else
  echo "Done. Containers removed; data volumes preserved."
  echo "For a full reset (delete data too), re-run with:  --volumes"
fi
