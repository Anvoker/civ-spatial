#!/usr/bin/env python3
"""Combined analysis across one or more results.jsonl files.

Usage: python analysis/summarize.py results-*.jsonl

Prints, per (model x encoding): accuracy, mean total tokens (prompt+completion), mean
completion tokens, and mean latency — plus an accuracy breakdown by question category.
Deliberately dependency-free (stdlib only); swap in pandas/DuckDB later if desired.
"""
import glob
import json
import math
import sys
from collections import defaultdict

# The statuses that count as scored trials (the accuracy denominator). Anything else — notably
# "error" (a failed/empty provider call, B4) — is excluded so a run can't be silently deflated by
# dropped calls. Unknown statuses are treated as non-scored too (fail-safe).
SCORED_STATUSES = {"correct", "wrong", "invalid"}


def is_scored(row):
    """Whether a row is a scored trial (in the accuracy denominator)."""
    return row.get("status") in SCORED_STATUSES


def wilson(k, n, z=1.959963984540054):
    """95% Wilson score interval for k successes out of n trials.

    Returns (lo, hi). z defaults to the two-sided 95% normal quantile.
    Stdlib only — no numpy/scipy. For n==0 returns (0.0, 0.0).
    """
    if n == 0:
        return (0.0, 0.0)
    phat = k / n
    z2 = z * z
    denom = 1.0 + z2 / n
    center = (phat + z2 / (2 * n)) / denom
    margin = (z / denom) * math.sqrt(phat * (1 - phat) / n + z2 / (4 * n * n))
    return (max(0.0, center - margin), min(1.0, center + margin))


def _self_check():
    # Known case: 8/10 successes -> ~[0.49, 0.94] (Wilson, 95%).
    lo, hi = wilson(8, 10)
    assert abs(lo - 0.4901) < 0.01, f"wilson lo off: {lo}"
    assert abs(hi - 0.9432) < 0.01, f"wilson hi off: {hi}"
    # Degenerate perfect case still yields a finite, sane upper bound < 1 for small n.
    lo, hi = wilson(10, 10)
    assert 0.6 < lo < 0.75 and abs(hi - 1.0) < 1e-9, f"wilson 10/10 off: {lo},{hi}"
    # Zero trials must not divide-by-zero.
    assert wilson(0, 0) == (0.0, 0.0)
    # B4: scored-status classification — error/unknown are NOT scored trials.
    assert is_scored({"status": "correct"}) and is_scored({"status": "wrong"})
    assert is_scored({"status": "invalid"})
    assert not is_scored({"status": "error"})
    assert not is_scored({"status": "capped"})  # unknown status -> non-scored (fail-safe)


def load(paths):
    rows = []
    for pat in paths:
        for path in glob.glob(pat):
            with open(path, encoding="utf-8") as f:
                for line in f:
                    line = line.strip()
                    if line:
                        rows.append(json.loads(line))
    return rows


def mean(xs):
    xs = list(xs)
    return sum(xs) / len(xs) if xs else 0.0


def main(argv):
    paths = argv or ["results*.jsonl"]
    rows = load(paths)
    if not rows:
        print("no rows found for", paths)
        return 1

    # (model, encoding) -> list of rows
    cells = defaultdict(list)
    for r in rows:
        cells[(r["model"], r["encoding"])].append(r)

    print(f"{len(rows)} trials across {len(cells)} (model x encoding) conditions")
    print("acc shown with 95% Wilson score interval [lo, hi]\n")
    header = (f"{'model':<28}{'enc':<10}{'acc':>8}{'  95% Wilson CI':<18}"
              f"{'n':>5}{'err':>5}{'tot_tok':>10}{'gen_tok':>9}{'lat_s':>8}")
    print(header)
    print("-" * len(header))
    for (model, enc) in sorted(cells):
        rs = cells[(model, enc)]
        # B4: only scored trials go in the denominator; "error" (failed/empty call) is counted
        # separately and never folded into accuracy or the token/latency means.
        scored = [r for r in rs if is_scored(r)]
        n_err = len(rs) - len(scored)
        k = sum(1 for r in scored if r["status"] == "correct")
        n = len(scored)
        acc = k / n if n else 0.0
        lo, hi = wilson(k, n)
        tot = mean(r["prompt_tokens"] + r["completion_tokens"] for r in scored)
        gen = mean(r["completion_tokens"] for r in scored)
        lat = mean(r["latency_ms"] for r in scored) / 1000.0
        ci = f"  [{lo:.3f}, {hi:.3f}]"
        print(f"{model:<28}{enc:<10}{acc:>8.3f}{ci:<18}{n:>5}{n_err:>5}{tot:>10.0f}{gen:>9.0f}{lat:>8.1f}")

    # Accuracy by category, per condition.
    print("\nAccuracy by category:")
    cats = sorted({r["category"] for r in rows})
    conds = sorted(cells)
    print(f"  {'category':<14}" + "".join(f"{m.split(' ')[0][:10] + '/' + e[:3]:>24}" for (m, e) in conds))
    by = defaultdict(lambda: defaultdict(list))
    for r in rows:
        if not is_scored(r):  # B4: keep non-scored (error) rows out of the category denominators too
            continue
        by[(r["model"], r["encoding"])][r["category"]].append(r["status"] == "correct")
    for cat in cats:
        line = f"  {cat:<14}"
        for cond in conds:
            vals = by[cond].get(cat, [])
            if vals:
                k, n = sum(vals), len(vals)
                lo, hi = wilson(k, n)
                cell = f"{k}/{n} [{lo:.2f},{hi:.2f}]"
            else:
                cell = "-"
            line += f"{cell:>24}"
        print(line)
    return 0


if __name__ == "__main__":
    _self_check()
    sys.exit(main(sys.argv[1:]))
