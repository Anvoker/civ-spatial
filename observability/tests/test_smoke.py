"""Smoke test for the OTel GenAI trace adapter using an in-memory exporter.

No Docker, no network, no API key: it runs ``emit_dir`` against the hand-written fixtures
and asserts the expected span tree (root -> chat turns -> execute_tool children) with the
right GenAI token attributes and tool-result matching.

Run with pytest:  ``python -m pytest observability/tests``
Run as a script:  ``python observability/tests/test_smoke.py``
"""
from __future__ import annotations

import os
import sys

# Make ``trace_to_otel`` importable whether run from repo root or from tests/.
_HERE = os.path.dirname(os.path.abspath(__file__))
_PKG = os.path.dirname(_HERE)
if _PKG not in sys.path:
    sys.path.insert(0, _PKG)

from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter

import trace_to_otel as t2o

FIXTURES = os.path.join(_HERE, "fixtures")


def _collect_spans():
    exporter = InMemorySpanExporter()
    provider = t2o.build_provider(exporter=exporter, batch=False)
    n = t2o.emit_dir(provider, FIXTURES)
    provider.force_flush()
    provider.shutdown()
    spans = list(exporter.get_finished_spans())
    return n, spans


def _by_name(spans):
    d: dict[str, list] = {}
    for s in spans:
        d.setdefault(s.name, []).append(s)
    return d


def _children(spans, parent):
    pid = parent.context.span_id
    return [s for s in spans if s.parent and s.parent.span_id == pid]


def test_span_tree():
    n, spans = _collect_spans()
    assert n == 2, f"expected 2 trace files processed, got {n}"

    # --- roots: one per question, both parentless ---------------------------------
    roots = [s for s in spans if s.parent is None]
    assert len(roots) == 2, f"expected 2 root spans, got {len(roots)}"
    root_names = sorted(s.name for s in roots)
    assert root_names == ["eval region_summary", "eval terrain"], root_names

    interactive_root = next(s for s in roots if s.name == "eval region_summary")
    static_root = next(s for s in roots if s.name == "eval terrain")

    # roots share a trace with their children (each question -> one trace id)
    for root in roots:
        assert all(
            c.context.trace_id == root.context.trace_id for c in _children(spans, root)
        )

    # --- interactive: root -> 3 chat spans -> 2 execute_tool spans -----------------
    chats = _children(spans, interactive_root)
    assert len(chats) == 3, f"expected 3 chat turns, got {len(chats)}"
    for c in chats:
        assert c.name == "chat qwen3-35b-a3b", c.name
        assert c.attributes[t2o.GEN_AI_OPERATION_NAME] == "chat"
        # token attributes present on every chat span
        assert t2o.GEN_AI_USAGE_INPUT_TOKENS in c.attributes
        assert t2o.GEN_AI_USAGE_OUTPUT_TOKENS in c.attributes
        assert t2o.GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS in c.attributes

    # turn 0 token values pulled straight from the fixture
    turn0 = min(chats, key=lambda s: s.start_time)
    assert turn0.attributes[t2o.GEN_AI_USAGE_INPUT_TOKENS] == 512
    assert turn0.attributes[t2o.GEN_AI_USAGE_OUTPUT_TOKENS] == 24
    assert turn0.attributes[t2o.GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS] == 448

    # exactly two tool calls across the interactive turns
    tool_spans = [
        s for s in spans if s.attributes.get(t2o.GEN_AI_OPERATION_NAME) == "execute_tool"
    ]
    assert len(tool_spans) == 2, f"expected 2 execute_tool spans, got {len(tool_spans)}"
    tool_by_name = {s.attributes[t2o.GEN_AI_TOOL_NAME]: s for s in tool_spans}
    assert set(tool_by_name) == {"region_summary", "get_tile"}, set(tool_by_name)

    # each tool span nests under a chat span (not the root)
    chat_ids = {c.context.span_id for c in chats}
    for ts in tool_spans:
        assert ts.parent is not None and ts.parent.span_id in chat_ids, (
            f"execute_tool {ts.name} not parented by a chat span"
        )

    # tool-result matching: results come from the FOLLOWING turn's tool message
    rs = tool_by_name["region_summary"]
    assert rs.attributes[t2o.GEN_AI_TOOL_CALL_ID] == "call_a1"
    assert "Rome:3 cities" in rs.attributes[t2o.GEN_AI_TOOL_CALL_RESULT]
    gt = tool_by_name["get_tile"]
    assert gt.attributes[t2o.GEN_AI_TOOL_CALL_ID] == "call_b2"
    assert "Antium" in gt.attributes[t2o.GEN_AI_TOOL_CALL_RESULT]

    # arguments captured verbatim (as the raw args string from the trace)
    assert '"radius":3' in rs.attributes[t2o.GEN_AI_TOOL_CALL_ARGUMENTS]

    # root result attributes
    assert interactive_root.attributes[t2o.CIV_RESULT_STATUS] == "correct"
    assert interactive_root.attributes[t2o.CIV_RESULT_GOT] == "Rome:3"
    assert interactive_root.attributes[t2o.CIV_ENCODING] == "interactive"

    # --- static: root -> 1 chat span, no tool spans -------------------------------
    static_chats = _children(spans, static_root)
    assert len(static_chats) == 1, f"expected 1 static chat span, got {len(static_chats)}"
    sc = static_chats[0]
    assert sc.attributes[t2o.GEN_AI_USAGE_INPUT_TOKENS] == 1840
    assert sc.attributes[t2o.GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS] == 1792
    # the static prompt is reconstructed from the cache split into the input attribute
    assert "cacheable_prefix" in sc.attributes[t2o.GEN_AI_INPUT_MESSAGES]
    assert not _children(spans, sc), "static turn must have no execute_tool children"
    assert static_root.attributes[t2o.CIV_MODE] == "static"

    # --- timeline synthesis: chat turns are strictly ordered, non-overlapping ------
    ordered = sorted(chats, key=lambda s: s.start_time)
    for a, b in zip(ordered, ordered[1:]):
        assert a.end_time <= b.start_time, "interactive chat turns overlap in time"
    # turn 0 duration equals its latency_ms (640ms) in nanoseconds
    assert turn0.end_time - turn0.start_time == 640 * 1_000_000


def main() -> int:
    test_span_tree()
    print("smoke test PASSED: root -> chat turns -> execute_tool tree verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
