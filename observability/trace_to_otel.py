#!/usr/bin/env python3
"""Adapt CivSpatial per-question JSON trace files into OpenTelemetry GenAI spans.

The CivSpatial Rust eval writes one JSON file per question to
``traces/<run>/<encoding>__<item_id>.json`` (see ``civ-eval/src/trace.rs``). This
adapter globs those files and, per question, emits a tree of OTel spans following the
**OpenTelemetry GenAI semantic conventions** so the backend is swappable: Langfuse is
the primary target (OTLP/HTTP ingestion), Phoenix is a drop-in (different endpoint, no
auth header). We deliberately do NOT use any vendor's native SDK.

Span tree per question::

    root  (gen_ai.operation.name = <category>)          one per question
    +-- chat  (gen_ai.operation.name = "chat")          one per turn
    |     +-- execute_tool  (gen_ai.operation.name = "execute_tool")   one per tool call
    |     +-- execute_tool ...
    +-- chat ...

Tool-result matching
--------------------
A turn's assistant reply may contain ``tool_calls`` (each with an ``id``). The eval runs
the tool and feeds the result back as a ``{"role":"tool","tool_call_id":...}`` message in
the **following** turn's request messages. So to attach a result to an ``execute_tool``
span we look up the matching ``tool_call_id`` in ``turns[i+1].request.messages``.

Timeline synthesis
-------------------
Traces carry a per-turn ``latency_ms`` but **no absolute timestamps**. We therefore
synthesize a monotonic timeline: turn 0 starts at a fixed base epoch, each turn's chat
span lasts ``latency_ms``, and the next turn's chat span starts where the previous ended.
``execute_tool`` spans are placed at the end of their parent chat span with a nominal
duration (real tool latency is not recorded by the Rust core). Absolute wall-clock values
are meaningless; only the *ordering and relative durations* are faithful.

Semconv version pin
-------------------
Targets the OpenTelemetry **GenAI semantic conventions v1.36.0** (experimental /
incubating -- the GenAI semconv is not yet stable, so attribute names may shift in future
releases). The attribute string literals below are pinned to that version; they match the
constants in ``opentelemetry-semantic-conventions==0.65b0``.
"""
from __future__ import annotations

import argparse
import base64
import glob
import json
import os
import sys
import time
from typing import Any, Iterable, Optional

from opentelemetry import trace as ot_trace
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import (
    BatchSpanProcessor,
    ConsoleSpanExporter,
    SimpleSpanProcessor,
    SpanExporter,
)
from opentelemetry.trace import SpanKind

# --- GenAI semantic conventions v1.36.0 (experimental) -----------------------------
# Pinned as string literals so the adapter does not depend on the private
# ``opentelemetry.semconv._incubating`` import path (which can move between SDK
# releases). These match opentelemetry-semantic-conventions==0.65b0.
GEN_AI_OPERATION_NAME = "gen_ai.operation.name"
GEN_AI_PROVIDER_NAME = "gen_ai.provider.name"
GEN_AI_REQUEST_MODEL = "gen_ai.request.model"
GEN_AI_RESPONSE_MODEL = "gen_ai.response.model"
GEN_AI_USAGE_INPUT_TOKENS = "gen_ai.usage.input_tokens"
GEN_AI_USAGE_OUTPUT_TOKENS = "gen_ai.usage.output_tokens"
GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS = "gen_ai.usage.cache_read_input_tokens"
GEN_AI_TOOL_NAME = "gen_ai.tool.name"
GEN_AI_TOOL_CALL_ID = "gen_ai.tool.call.id"
GEN_AI_TOOL_CALL_ARGUMENTS = "gen_ai.tool.call.arguments"
GEN_AI_TOOL_CALL_RESULT = "gen_ai.tool.call.result"
GEN_AI_INPUT_MESSAGES = "gen_ai.input.messages"
GEN_AI_OUTPUT_MESSAGES = "gen_ai.output.messages"
GEN_AI_TOOL_DEFINITIONS = "gen_ai.tool.definitions"

# CivSpatial-specific attributes (namespaced so they never collide with semconv).
CIV_ENCODING = "civspatial.encoding"
CIV_BOARD = "civspatial.board"
CIV_ITEM_ID = "civspatial.item_id"
CIV_CATEGORY = "civspatial.category"
CIV_TIER = "civspatial.tier"
CIV_MODE = "civspatial.mode"
CIV_RESULT_STATUS = "civspatial.result.status"
CIV_RESULT_EXPECTED = "civspatial.result.expected"
CIV_RESULT_GOT = "civspatial.result.got"
CIV_LATENCY_MS = "civspatial.latency_ms"

SEMCONV_VERSION = "1.36.0"  # GenAI semantic conventions target (experimental)

# Deterministic fallback base epoch (2020-01-01T00:00:00Z) in nanoseconds — used by the
# unit tests. At runtime `main` defaults the base to the current time instead (see
# `parse_base_time`), so imported traces land "now" and stay visible in the backend's
# default recent-time window rather than being hidden back in 2020. Turn timelines are
# built relative to whichever base is chosen; only ordering/relative durations are faithful.
BASE_EPOCH_NS = 1_577_836_800 * 1_000_000_000
_MS_NS = 1_000_000
# Gap between consecutive traces in a batch import, so a directory of traces lists in a
# stable order (and ends at the base) rather than all sharing one instant.
_TRACE_GAP_NS = 1_000_000_000  # 1s
# Nominal duration for a synthesized execute_tool span (tool latency is not recorded).
TOOL_SPAN_NS = 1 * _MS_NS

# Defaults for the two supported local backends.
DEFAULT_LANGFUSE_ENDPOINT = "http://localhost:3000/api/public/otel/v1/traces"
DEFAULT_PHOENIX_ENDPOINT = "http://localhost:6006/v1/traces"


# ----------------------------------------------------------------------------------
# Exporter / provider wiring
# ----------------------------------------------------------------------------------
def langfuse_headers_from_env() -> dict[str, str]:
    """Build the Langfuse OTLP auth header from env vars, if present.

    Langfuse authenticates OTLP ingestion with HTTP Basic auth over
    ``base64(public_key:secret_key)``. Returns ``{}`` when keys are absent (e.g. Phoenix,
    or an unauthenticated local Langfuse), so the caller can merge unconditionally.
    """
    pub = os.environ.get("LANGFUSE_PUBLIC_KEY")
    sec = os.environ.get("LANGFUSE_SECRET_KEY")
    if pub and sec:
        token = base64.b64encode(f"{pub}:{sec}".encode()).decode()
        return {"Authorization": f"Basic {token}"}
    return {}


def build_provider(
    *,
    exporter: Optional[SpanExporter] = None,
    endpoint: Optional[str] = None,
    headers: Optional[dict[str, str]] = None,
    console: bool = False,
    batch: bool = True,
) -> TracerProvider:
    """Construct a TracerProvider with a single span processor/exporter.

    Precedence: an explicit ``exporter`` (used by the smoke test to inject an in-memory
    exporter) wins; then ``console``; otherwise an OTLP/HTTP exporter to ``endpoint``.
    ``batch`` selects BatchSpanProcessor (production) vs SimpleSpanProcessor (tests/console,
    for deterministic, immediately-flushed output).
    """
    resource = Resource.create(
        {
            "service.name": "civspatial-eval",
            "telemetry.sdk.name": "civspatial-trace-to-otel",
            "civspatial.semconv.genai_version": SEMCONV_VERSION,
        }
    )
    provider = TracerProvider(resource=resource)

    if exporter is None:
        if console:
            exporter = ConsoleSpanExporter()
        else:
            # Imported lazily so `--console`/tests work even if the OTLP exporter's
            # transitive deps (protobuf, etc.) are missing.
            from opentelemetry.exporter.otlp.proto.http.trace_exporter import (
                OTLPSpanExporter,
            )

            exporter = OTLPSpanExporter(endpoint=endpoint, headers=headers or {})

    processor_cls = BatchSpanProcessor if batch else SimpleSpanProcessor
    provider.add_span_processor(processor_cls(exporter))
    return provider


# ----------------------------------------------------------------------------------
# Trace -> span mapping
# ----------------------------------------------------------------------------------
def _turn_input_output(turn: dict[str, Any]) -> tuple[str, str]:
    """Return (input_json, output_json) strings for a turn's chat span.

    Handles both interactive (``messages``/``tools``) and static
    (``cacheable_prefix``/``tail``) request shapes.
    """
    req = turn.get("request", {})
    resp = turn.get("response", {})
    if "messages" in req:
        inp = json.dumps(req.get("messages", []), ensure_ascii=False)
    else:  # static one-shot: reconstruct the prompt from its cache split
        inp = json.dumps(
            {
                "cacheable_prefix": req.get("cacheable_prefix", ""),
                "tail": req.get("tail", ""),
            },
            ensure_ascii=False,
        )
    out = json.dumps(
        {"text": resp.get("text"), "tool_calls": resp.get("tool_calls", [])},
        ensure_ascii=False,
    )
    return inp, out


def _tool_results_from_next_turn(next_turn: Optional[dict[str, Any]]) -> dict[str, str]:
    """Map tool_call_id -> tool result content, harvested from the next turn's messages."""
    results: dict[str, str] = {}
    if not next_turn:
        return results
    for msg in next_turn.get("request", {}).get("messages", []):
        if msg.get("role") == "tool" and msg.get("tool_call_id"):
            results[msg["tool_call_id"]] = msg.get("content", "")
    return results


def emit_trace(
    provider: TracerProvider, doc: dict[str, Any], base_epoch_ns: int = BASE_EPOCH_NS
) -> None:
    """Emit the span tree for a single question document (parsed trace JSON)."""
    tracer = provider.get_tracer("civspatial-trace-to-otel")
    model = doc.get("model", "unknown")
    category = doc.get("category", "eval")
    turns = doc.get("turns", [])

    # Root span covers the whole question; its window spans all turns.
    total_ns = sum(int(t.get("response", {}).get("latency_ms", 0)) for t in turns) * _MS_NS
    total_ns = max(total_ns, 1)  # a zero-latency (e.g. oracle) run still needs a window
    root_start = base_epoch_ns
    root = tracer.start_span(
        name=f"eval {category}",
        kind=SpanKind.INTERNAL,
        start_time=root_start,
        attributes={
            GEN_AI_OPERATION_NAME: category,
            GEN_AI_PROVIDER_NAME: "civspatial",
            GEN_AI_REQUEST_MODEL: model,
            CIV_MODE: doc.get("mode", ""),
            CIV_ENCODING: doc.get("encoding", ""),
            CIV_BOARD: doc.get("board", ""),
            CIV_ITEM_ID: doc.get("item_id", ""),
            CIV_CATEGORY: category,
            CIV_TIER: doc.get("tier", ""),
        },
    )
    result = doc.get("result", {})
    root.set_attribute(CIV_RESULT_STATUS, result.get("status", ""))
    root.set_attribute(CIV_RESULT_EXPECTED, result.get("expected", ""))
    root.set_attribute(CIV_RESULT_GOT, result.get("got", ""))

    cursor = root_start
    for i, turn in enumerate(turns):
        resp = turn.get("response", {})
        usage = resp.get("usage", {})
        latency_ms = int(resp.get("latency_ms", 0))
        chat_start = cursor
        chat_end = chat_start + max(latency_ms, 0) * _MS_NS
        inp, out = _turn_input_output(turn)
        req = turn.get("request", {})

        chat = tracer.start_span(
            name=f"chat {model}",
            kind=SpanKind.CLIENT,
            start_time=chat_start,
            context=ot_trace.set_span_in_context(root),
            attributes={
                GEN_AI_OPERATION_NAME: "chat",
                GEN_AI_PROVIDER_NAME: "civspatial",
                GEN_AI_REQUEST_MODEL: model,
                GEN_AI_RESPONSE_MODEL: model,
                GEN_AI_USAGE_INPUT_TOKENS: int(usage.get("prompt_tokens", 0)),
                GEN_AI_USAGE_OUTPUT_TOKENS: int(usage.get("completion_tokens", 0)),
                GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS: int(usage.get("cached_tokens", 0)),
                GEN_AI_INPUT_MESSAGES: inp,
                GEN_AI_OUTPUT_MESSAGES: out,
                CIV_LATENCY_MS: latency_ms,
            },
        )
        if "tools" in req and req.get("tools"):
            chat.set_attribute(GEN_AI_TOOL_DEFINITIONS, json.dumps(req["tools"]))

        # Tool results for this turn's calls live in the *next* turn's messages.
        next_turn = turns[i + 1] if i + 1 < len(turns) else None
        results = _tool_results_from_next_turn(next_turn)

        tool_calls = resp.get("tool_calls", []) or []
        for j, call in enumerate(tool_calls):
            # Nominal placement: pack tool spans at the tail of the chat span.
            t_start = chat_end + j * TOOL_SPAN_NS
            t_end = t_start + TOOL_SPAN_NS
            call_id = call.get("id", "")
            tool_span = tracer.start_span(
                name=f"execute_tool {call.get('name', 'tool')}",
                kind=SpanKind.INTERNAL,
                start_time=t_start,
                context=ot_trace.set_span_in_context(chat),
                attributes={
                    GEN_AI_OPERATION_NAME: "execute_tool",
                    GEN_AI_TOOL_NAME: call.get("name", ""),
                    GEN_AI_TOOL_CALL_ID: call_id,
                    GEN_AI_TOOL_CALL_ARGUMENTS: call.get("args", ""),
                },
            )
            if call_id in results:
                tool_span.set_attribute(GEN_AI_TOOL_CALL_RESULT, results[call_id])
            else:
                tool_span.set_attribute("civspatial.tool.result_missing", True)
            tool_span.end(end_time=t_end)

        chat.end(end_time=chat_end)
        # Advance the clock past this turn's chat and any tool spans it spawned.
        cursor = chat_end + len(tool_calls) * TOOL_SPAN_NS

    root.end(end_time=max(root_start + total_ns, cursor))


def load_trace(path: str) -> dict[str, Any]:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def iter_trace_files(traces_dir: str) -> Iterable[str]:
    """Yield trace JSON files under ``traces_dir`` (recursively), sorted for determinism."""
    pattern = os.path.join(traces_dir, "**", "*.json")
    return sorted(glob.glob(pattern, recursive=True))


def emit_dir(
    provider: TracerProvider, traces_dir: str, base_epoch_ns: int = BASE_EPOCH_NS
) -> int:
    """Emit spans for every trace file under ``traces_dir``. Returns the count processed.

    Traces are laid out one ``_TRACE_GAP_NS`` apart, ending at ``base_epoch_ns`` (so the
    newest is at the base and none land in the future), giving a stable, ordered listing.
    """
    files = list(iter_trace_files(traces_dir))
    n = len(files)
    count = 0
    for i, path in enumerate(files):
        try:
            doc = load_trace(path)
        except (json.JSONDecodeError, OSError) as e:
            print(f"warning: skipping {path}: {e}", file=sys.stderr)
            continue
        trace_base = base_epoch_ns - (n - 1 - i) * _TRACE_GAP_NS
        emit_trace(provider, doc, base_epoch_ns=trace_base)
        count += 1
    return count


def parse_base_time(s: Optional[str]) -> int:
    """Resolve the base epoch (ns). ``None`` -> now; else epoch seconds or ISO-8601."""
    if not s:
        return time.time_ns()
    try:
        return int(float(s) * 1_000_000_000)  # epoch seconds
    except ValueError:
        pass
    from datetime import datetime, timezone

    dt = datetime.fromisoformat(s)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return int(dt.timestamp() * 1_000_000_000)


# ----------------------------------------------------------------------------------
# CLI
# ----------------------------------------------------------------------------------
def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Convert CivSpatial JSON traces into OTel GenAI spans (OTLP/HTTP)."
    )
    parser.add_argument("traces_dir", help="Directory of trace JSON files (globbed recursively).")
    parser.add_argument(
        "--endpoint",
        default=DEFAULT_LANGFUSE_ENDPOINT,
        help=f"OTLP/HTTP traces endpoint (default: {DEFAULT_LANGFUSE_ENDPOINT}). "
        f"For Phoenix use {DEFAULT_PHOENIX_ENDPOINT}.",
    )
    parser.add_argument(
        "--console",
        action="store_true",
        help="Print spans to stdout via ConsoleSpanExporter (no network, no backend).",
    )
    parser.add_argument(
        "--no-batch",
        action="store_true",
        help="Use a SimpleSpanProcessor (immediate export) instead of BatchSpanProcessor.",
    )
    parser.add_argument(
        "--base-time",
        default=None,
        help="Base timestamp for the synthesized timeline: epoch seconds or ISO-8601. "
        "Default: current time, so traces land 'now' and show in the backend's default "
        "recent-time view. Pass a fixed value for reproducible timestamps.",
    )
    args = parser.parse_args(argv)

    headers = langfuse_headers_from_env()
    provider = build_provider(
        endpoint=args.endpoint,
        headers=headers,
        console=args.console,
        batch=not args.no_batch,
    )

    base_ns = parse_base_time(args.base_time)
    dest = "console" if args.console else args.endpoint
    n = emit_dir(provider, args.traces_dir, base_epoch_ns=base_ns)
    provider.force_flush()
    provider.shutdown()
    print(
        f"emitted spans for {n} trace file(s) -> {dest} "
        f"(base {base_ns // 1_000_000_000}s epoch)",
        file=sys.stderr,
    )
    return 0 if n > 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
