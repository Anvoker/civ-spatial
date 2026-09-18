#!/usr/bin/env bash
# Turnkey bring-up for the local, fully self-hosted Langfuse stack used to view
# CivSpatial OTel GenAI traces. Nothing here leaves the machine.
#
# Usage:   bash observability/langfuse-up.sh
#
# Idempotent: re-running just reconciles the stack (already-running containers stay).
# First run pulls several GB of images and can take a few minutes; the script waits
# until the web UI reports healthy before printing the next steps.
#
# Tear down with:  bash observability/langfuse-down.sh   (add --volumes to wipe data)
set -euo pipefail

# Resolve paths relative to this script so it works from any CWD.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="$SCRIPT_DIR/docker-compose.yml"
UI_PORT="${LANGFUSE_UI_PORT:-3000}"   # host port for the web UI (see docker-compose.yml)

# Preflight: the Docker daemon must be reachable.
if ! docker info >/dev/null 2>&1; then
  echo "ERROR: the Docker daemon is not reachable." >&2
  echo "       Start Docker Desktop and wait for the whale icon to settle, then re-run." >&2
  exit 1
fi

echo "Bringing up the Langfuse stack (web + worker + postgres + clickhouse + redis + minio)..."
echo "First run pulls images (several GB) and may take a few minutes."
docker compose -f "$COMPOSE_FILE" up -d

# Poll until the web container is healthy (or fail loudly after ~5 min).
echo -n "Waiting for langfuse-web to become healthy"
deadline=$(( $(date +%s) + 300 ))
while true; do
  status="$(docker compose -f "$COMPOSE_FILE" ps --format '{{.Service}} {{.Health}}' 2>/dev/null \
             | awk '$1=="langfuse-web"{print $2}')"
  if [ "$status" = "healthy" ]; then echo " OK"; break; fi
  # If the image has no healthcheck, fall back to probing the port directly.
  if [ -z "$status" ] && curl -fsS "http://localhost:${UI_PORT}" >/dev/null 2>&1; then
    echo " OK (port responds)"; break
  fi
  if [ "$(date +%s)" -ge "$deadline" ]; then
    echo " TIMEOUT"
    echo "langfuse-web did not become healthy in time. Current status:" >&2
    docker compose -f "$COMPOSE_FILE" ps >&2
    echo "Check logs with: docker compose -f \"$COMPOSE_FILE\" logs langfuse-web" >&2
    exit 1
  fi
  echo -n "."
  sleep 5
done

echo
docker compose -f "$COMPOSE_FILE" ps
echo
echo "=============================================================================="
echo " Langfuse is up:  http://localhost:${UI_PORT}"
echo "=============================================================================="
echo "Next (one-time, in the browser -- cannot be automated):"
echo "  1. Open http://localhost:${UI_PORT} and create an account (the FIRST user is admin)."
echo "  2. Create/open a project, then: Project Settings -> API Keys -> Create new key."
echo "  3. Copy the public (pk-lf-...) and secret (sk-lf-...) keys."
echo
echo "Then push your existing traces:"
echo "  export LANGFUSE_PUBLIC_KEY=pk-lf-..."
echo "  export LANGFUSE_SECRET_KEY=sk-lf-..."
echo "  python observability/trace_to_otel.py traces/results-tracediag-1785429846953 \\"
echo "      --endpoint http://localhost:${UI_PORT}/api/public/otel/v1/traces"
echo
echo "Finally, view them in the UI under: Tracing -> Traces"
