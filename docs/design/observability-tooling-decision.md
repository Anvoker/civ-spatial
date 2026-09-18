# Observability tooling decision — viewing & analyzing LLM traces

**Status:** decided 2026-07-30. The Rust eval now persists per-question JSON traces (`civ-eval/src/trace.rs`,
default-on). This records the choice for the *human viewing/analysis* layer on top. Constraints (from the
project owner): **no third-party cloud** (fully local / self-hostable), **open-source preferred**, and
**resume/career value matters** (this doubles as a portfolio — see memory `user-career-and-privacy`).

## Decision
**Emit OpenTelemetry GenAI-semconv spans from a small offline adapter, and view them in a self-hosted
Langfuse (local Docker, MIT). Arize Phoenix is the drop-in alternative backend.**

## Why (the key landscape fact)
Every serious self-hostable trace **viewer** in this space (Langfuse, Phoenix, OpenLIT, Langtrace, Laminar) is
**OTLP/OpenTelemetry-native — none ingests an arbitrary custom JSON schema directly**. So the real fork is not
"which tool reads my JSON" but:
- **Route A (chosen):** a ~150-line offline adapter replays our per-question JSON into **OTel GenAI spans** over
  OTLP → any local backend. No lock-in; the same emitter feeds Langfuse *or* Phoenix.
- **Route B (rejected):** wire to one vendor's native SDK — saves little code, costs portability, teaches a
  vendor-specific skill instead of a portable one.

Route A wins on all three constraints: (a) local-only (OTLP → `localhost`, nothing leaves the machine);
(b) least effort — data is already persisted, so it's a **batch replay**, not live instrumentation, and the
adapter is backend-agnostic; (c) best resume value — you can claim **OpenTelemetry GenAI instrumentation +
self-hosted Langfuse**, two recognized/hireable skills (OTel is CNCF-graduated; Langfuse is the most-cited
self-host LLM-observability platform).

## Candidate comparison (verified July 2026)
| Tool | Local self-host | License | Adoption | Ingest | Multi-turn tool-use + eval | Resume signal |
|---|---|---|---|---|---|---|
| **Langfuse** (chosen backend) | Yes, Docker | **MIT** core | ~30k★; ClickHouse-acquired Jan 2026, self-host committed | OTLP endpoint + own SDK | Excellent trace→span→generation tree; evals/datasets/prompts | **Highest** |
| **Arize Phoenix** (alt) | Yes, `phoenix serve` | Elastic v2 (source-available) | Arize-backed, ~10k★ | OTLP (OpenInference) | Excellent, esp. eval/experiments | High |
| **OTel GenAI semconv** (the standard) | Yes | Apache 2.0 (CNCF) | OTel graduated; **semconv pre-stable** | you emit | the vocabulary for agent/tool spans | **Highest transferable** |
| Laminar (lmnr) | Yes, Docker | Apache 2.0 | newer/smaller; Rust core | OTLP | agent-focused | Rising |
| OpenLLMetry / OpenLIT / Langtrace | mixed | Apache/AGPL | smaller | OTLP emitters | ok | lower |
| **Helicone (self-host)** | ~ | Apache 2.0 | **maintenance mode after Mar-2026 acquisition** | — | — | **excluded** (+ owner wary of Helicone) |

## Integration sketch (implemented in `observability/`, adapter build)
1. Stand up **Langfuse locally** (`docker compose up` → UI `localhost:3000`; OTLP ingest at
   `/api/public/otel/v1/traces`, project key as auth header). Or Phoenix (`phoenix serve` → `:6006`, no auth).
2. **Adapter** (`observability/trace_to_otel.py`): per question, a **root span**; per turn a **`chat` span**
   (model, `gen_ai.usage.input/output_tokens`, latency, messages); per tool call an **`execute_tool` span**
   nested under the turn (tool name/args + result matched via `tool_call_id` in the next turn's tool message).
   Export via OTLP/HTTP + `BatchSpanProcessor`; `--console` mode for backend-free testing.
   Timeline is **synthesized from per-turn `latency_ms`** (traces carry no absolute timestamps).
3. View the trace tree in the UI; attach eval scores (Langfuse datasets / Phoenix experiments).

**Caveat:** GenAI semconv is pre-stable ("Development") — pin the targeted version; expect minor attribute
renames across OTel releases. Not blocking for a local research setup.

## Sources
Langfuse (MIT/self-host/ClickHouse): github.com/langfuse/langfuse, langfuse.com/self-hosting,
langfuse.com/integrations/native/opentelemetry. Phoenix (ELv2/OTLP): github.com/Arize-ai/openinference.
OTel GenAI semconv (pre-stable, CNCF): open-telemetry/semantic-conventions-genai, opentelemetry.io GenAI
observability. OpenLLMetry (Apache, emitter): github.com/traceloop/openllmetry. Laminar (Apache/Rust):
github.com/lmnr-ai/lmnr. Helicone maintenance mode: openobserve.ai LLM observability tools 2026.
