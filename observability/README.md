# CivSpatial observability: OTel GenAI traces → local Langfuse

Turn the per-question JSON trace files the CivSpatial Rust eval writes into browsable
**OpenTelemetry GenAI** traces, viewable in a **fully local, self-hosted Langfuse**
(Phoenix works as a drop-in). Everything here is Python tooling in this subdir — it does
**not** touch the dependency-free Rust core.

The adapter speaks OTel GenAI semconv over **OTLP/HTTP**, so the backend is swappable: we
never bind to a vendor's native SDK. Point it at Langfuse today, Phoenix tomorrow, by
changing one URL.

## Quickstart (turnkey)

Five steps from nothing to browsable traces. Steps 1 and 3 are one-time, manual, and
cannot be automated (they need the Docker Desktop GUI / the browser).

1. **Start Docker Desktop** and wait for the whale icon to go steady (daemon up). On a
   fresh machine this also needs WSL2 with a Linux distro installed — if `docker info`
   errors, run `wsl --install` in an elevated PowerShell, reboot, then start Docker Desktop.
2. **Bring the stack up (one command):**
   ```bash
   bash observability/langfuse-up.sh        # or:  just langfuse-up
   ```
   First run pulls several GB of images; the script waits until the web UI is healthy,
   then prints the URL and the next steps.
3. **Create an account + API keys** at http://localhost:3000 (the first user is admin):
   create/open a project → **Project Settings → API Keys → Create new key** → copy the
   `pk-lf-...` (public) and `sk-lf-...` (secret) pair.
4. **Push your existing traces:**
   ```bash
   export LANGFUSE_PUBLIC_KEY=pk-lf-...
   export LANGFUSE_SECRET_KEY=sk-lf-...
   python observability/trace_to_otel.py traces/results-tracediag-1785429846953 \
       --endpoint http://localhost:3000/api/public/otel/v1/traces
   ```
5. **View them** in the UI under **Tracing → Traces** — each question is one trace tree.

Tear down when done: `bash observability/langfuse-down.sh` (add `--volumes` to also wipe
all data for a fresh start), or `just langfuse-down` / `just langfuse-down args="--volumes"`.

## Why this design

- **Backend-neutral.** Spans follow the OpenTelemetry GenAI semantic conventions and ship
  over standard OTLP/HTTP. Langfuse and Phoenix both ingest OTLP, so swapping is a flag.
- **Faithful trajectory tree.** Interactive runs render as `root → chat turn → execute_tool`,
  reconstructed from the conversation structure in the trace file.

## The input: trace file schema

The Rust eval (`civ-eval/src/trace.rs`) writes one JSON file per question to
`traces/<run>/<encoding>__<item_id>.json`:

```json
{ "mode": "interactive|static", "model": "...", "encoding": "...", "board": "...",
  "item_id": "...", "category": "...", "tier": "...",
  "turns": [
    { "request": { "messages": [ {"role":"system|user|assistant|tool", "content":"...",
                                   "tool_calls":[{"id","name","args"}]?, "tool_call_id":"..."?} ],
                   "tools": ["region_summary", "..."] },
      "response": { "text": "...|null", "tool_calls":[{"id","name","args"}],
                    "usage": {"prompt_tokens","completion_tokens","cached_tokens"}, "latency_ms": N } }
  ],
  "result": { "status":"correct|wrong|invalid", "expected":"...", "got":"..." } }
```

For a **static** (one-shot) trial the single turn's `request` is `{cacheable_prefix, tail}`
and its `response` has no `tool_calls`.

## Span mapping

| Trace element | OTel span | Key attributes |
|---|---|---|
| One question | **root** span, `eval <category>` | `gen_ai.operation.name=<category>`, `gen_ai.request.model`, and `civspatial.*` for encoding / board / item_id / category / tier / mode + `civspatial.result.{status,expected,got}` |
| One turn | **chat** span, `chat <model>`, child of root | `gen_ai.operation.name=chat`, `gen_ai.request.model`, `gen_ai.usage.input_tokens` (=`prompt_tokens`), `gen_ai.usage.output_tokens` (=`completion_tokens`), `gen_ai.usage.cache_read_input_tokens` (=`cached_tokens`), `gen_ai.input.messages`, `gen_ai.output.messages` |
| One tool call in a turn | **execute_tool** span, `execute_tool <name>`, child of that turn's chat span | `gen_ai.operation.name=execute_tool`, `gen_ai.tool.name`, `gen_ai.tool.call.id`, `gen_ai.tool.call.arguments`, `gen_ai.tool.call.result` |

**Tool-result matching.** A turn's assistant reply lists `tool_calls`, each with an `id`.
The eval runs the tool and feeds the output back as a `{"role":"tool","tool_call_id":…}`
message in the **following** turn's request messages. The adapter attaches a result to its
`execute_tool` span by matching that `tool_call_id` in `turns[i+1].request.messages`. This
is what makes interactive trajectories render as a proper tree. (If a call has no matching
result — e.g. the final turn — the span carries `civspatial.tool.result_missing=true`.)

**Static vs interactive.** Static traces have exactly one turn and no tool calls, so they
produce `root → one chat span`. The static prompt is reconstructed from its cache split
(`cacheable_prefix` + `tail`) into the `gen_ai.input.messages` attribute.

## Timeline synthesis (important assumption)

Trace files carry a **per-turn `latency_ms` but no absolute timestamps**. The adapter
therefore *synthesizes* a monotonic timeline:

- The base epoch defaults to **the current time at import** (so traces land "now" and are
  visible in the backend's default recent-time window — otherwise a fixed-in-the-past base
  hides them behind the time filter). Pass `--base-time <epoch-seconds | ISO-8601>` for a
  reproducible fixed base. A batch of traces is laid one second apart, ending at the base.
- Turn 0's chat span starts at that base; each chat span lasts exactly its `latency_ms`.
- The next turn's chat span starts where the previous one ended (no overlap).
- `execute_tool` spans are packed at the tail of their parent chat span with a nominal
  1 ms duration, because **real tool-execution latency is not recorded** by the Rust core.

Consequence: absolute wall-clock values are meaningless (the base is arbitrary); only
**ordering and relative chat durations** are faithful. Do not read tool-span durations as
real timings.

## Semconv version pin

Targets **OpenTelemetry GenAI semantic conventions v1.36.0** (see `SEMCONV_VERSION` in
`trace_to_otel.py`). The GenAI semconv is **experimental / not yet stable**, so attribute
names may shift in future releases. The attribute strings are pinned as literals in the
adapter (matching `opentelemetry-semantic-conventions==0.65b0`) so an SDK bump can't
silently rename them out from under us.

## Install

```bash
cd observability
python -m pip install -r requirements.txt
```

## End-to-end: run → view

### 1. Produce traces (Rust eval)

Run any live eval; traces are auto-written to `traces/<run>/` (nothing extra to enable).
See the repo root README / `RESUME.md` for the eval invocation. You end up with a dir of
`<encoding>__<item_id>.json` files.

### 2. Stand up Langfuse locally (nothing leaves the machine)

```bash
docker compose -f observability/docker-compose.yml up -d
# open http://localhost:3000 , create a local account (first user is the admin),
# then create/open a project and mint an API key pair under Project Settings → API Keys.
```

The compose file is a pinned snapshot of Langfuse's official self-host stack (web +
worker + postgres + clickhouse + redis + minio) with throwaway dev secrets. If an image
bump breaks it, fall back to the canonical file:
`git clone https://github.com/langfuse/langfuse && cd langfuse && docker compose up`.

### 3. Run the adapter at your local endpoint

Langfuse ingests OTLP at `/api/public/otel/v1/traces` with **HTTP Basic auth** over your
key pair. Export the keys; the adapter builds the `Authorization` header for you:

```bash
export LANGFUSE_PUBLIC_KEY=pk-lf-...
export LANGFUSE_SECRET_KEY=sk-lf-...
python observability/trace_to_otel.py traces/<run> \
    --endpoint http://localhost:3000/api/public/otel/v1/traces
```

`http://localhost:3000/api/public/otel/v1/traces` is the default `--endpoint`, so you can
omit it when targeting local Langfuse.

### 4. View the trace tree

In the Langfuse UI open **Tracing → Traces**. Each question is one trace; expand it to see
`chat` turns and their nested `execute_tool` calls, with token counts on each chat span and
tool arguments/results on each tool span.

## Phoenix as a drop-in alternative (no auth header)

[Arize Phoenix](https://github.com/Arize-ai/phoenix) also ingests OTLP and needs no auth:

```bash
python -m pip install arize-phoenix
phoenix serve                        # UI + OTLP on http://localhost:6006
python observability/trace_to_otel.py traces/<run> \
    --endpoint http://localhost:6006/v1/traces
# then open http://localhost:6006
```

Leave `LANGFUSE_*` env vars unset and no auth header is attached.

## Verify without a backend (console / smoke test)

- **Console dump** (no Docker, no network): prints every span as JSON.
  ```bash
  python observability/trace_to_otel.py observability/tests/fixtures --console --no-batch
  ```
- **Smoke test** against the hand-written fixtures using an in-memory exporter — asserts the
  full `root → chat turns → execute_tool` tree, token attributes, and tool-result matching:
  ```bash
  python -m pytest observability/tests            # with pytest
  python observability/tests/test_smoke.py        # or as a plain script (no pytest needed)
  ```

## Files

- `trace_to_otel.py` — the adapter + CLI (`--endpoint`, `--console`, `--no-batch`).
- `requirements.txt` — pinned OTel SDK + OTLP/HTTP exporter.
- `docker-compose.yml` — local Langfuse v3 stack.
- `tests/fixtures/` — one interactive + one static sample trace.
- `tests/test_smoke.py` — backend-free span-tree assertions.

## CLI reference

```
python trace_to_otel.py <traces_dir> [--endpoint URL] [--console] [--no-batch]
```

- `<traces_dir>` — globbed recursively for `*.json` trace files.
- `--endpoint` — OTLP/HTTP traces URL. Default: local Langfuse.
- `--console` — export to stdout instead of a backend (no network).
- `--no-batch` — use a simple (immediate) span processor instead of batching.
- Env: `LANGFUSE_PUBLIC_KEY` / `LANGFUSE_SECRET_KEY` → adds `Authorization: Basic …`.
