#!/usr/bin/env python3
"""Region-count accuracy as a function of the TRUE count.

Tests the "multiplicity cliff" prediction from *Why Do LLMs Struggle to Count
Letters?* at the board level: counting accuracy is high when the true count is
0-1 and falls off a cliff as the count rises. Here we ask whether our
"how many <terrain> within radius R of (x,y)" questions show the same cliff,
and whether the ENCODING shifts where the cliff starts.

Usage:
    python analysis/count_curve.py results-*.jsonl
    python analysis/count_curve.py C:/Projects/CivSpatial/results-deepseek-fullboard-hard.jsonl ...

Reads only rows with category=="region-count". True count = int(expected).
Correct trial = status=="correct". Stdlib only.
"""
import glob
import json
import math
import sys
from collections import defaultdict


def wilson(k, n, z=1.959963984540054):
    """95% Wilson score interval for k successes out of n trials. (lo, hi)."""
    if n == 0:
        return (0.0, 0.0)
    phat = k / n
    z2 = z * z
    denom = 1.0 + z2 / n
    center = (phat + z2 / (2 * n)) / denom
    margin = (z / denom) * math.sqrt(phat * (1 - phat) / n + z2 / (4 * n * n))
    return (max(0.0, center - margin), min(1.0, center + margin))


# (label, predicate on true count c). Ordered low -> high.
BINS = [
    ("0", lambda c: c == 0),
    ("1", lambda c: c == 1),
    ("2", lambda c: c == 2),
    ("3", lambda c: c == 3),
    ("4", lambda c: c == 4),
    ("5-6", lambda c: 5 <= c <= 6),
    ("7-9", lambda c: 7 <= c <= 9),
    ("10+", lambda c: c >= 10),
]


def bin_of(c):
    for label, pred in BINS:
        if pred(c):
            return label
    return "?"


def load(paths):
    rows = []
    for pat in paths:
        matches = glob.glob(pat) or [pat]
        for path in matches:
            try:
                f = open(path, encoding="utf-8")
            except OSError as e:
                print(f"skip {path}: {e}", file=sys.stderr)
                continue
            with f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    r = json.loads(line)
                    if r.get("category") != "region-count":
                        continue
                    rows.append(r)
    return rows


def acc_cell(k, n):
    if n == 0:
        return "     -      "
    lo, hi = wilson(k, n)
    return f"{k/n:>4.2f}[{lo:.2f},{hi:.2f}]"


def print_curve(title, groups, order=None):
    """groups: dict groupkey -> dict binlabel -> [k, n]. Prints a table."""
    print(f"\n### {title}\n")
    labels = [lbl for lbl, _ in BINS]
    keys = order if order is not None else sorted(groups)
    head = f"{'condition \\ count':<30}" + "".join(f"{lbl:>16}" for lbl in labels) + f"{'ALL':>16}"
    print("```")
    print(head)
    print("-" * len(head))
    for key in keys:
        binmap = groups[key]
        row = f"{key:<30}"
        tk = tn = 0
        for lbl in labels:
            k, n = binmap.get(lbl, [0, 0])
            tk += k
            tn += n
            row += f"{acc_cell(k, n):>16}"
        row += f"{acc_cell(tk, tn):>16}"
        print(row)
    # n row per bin
    nrow = f"{'(n per bin, this table)':<30}"
    for lbl in labels:
        tot = sum(binmap.get(lbl, [0, 0])[1] for binmap in groups.values())
        nrow += f"{('n='+str(tot)):>16}"
    grand = sum(sum(v[1] for v in binmap.values()) for binmap in groups.values())
    nrow += f"{('n='+str(grand)):>16}"
    print(nrow)
    print("```")


def main(argv):
    if not argv:
        argv = ["results-*.jsonl"]
    rows = load(argv)
    if not rows:
        print("no region-count rows found for", argv)
        return 1

    overall = defaultdict(lambda: [0, 0])          # binlabel -> [k, n]
    by_enc = defaultdict(lambda: defaultdict(lambda: [0, 0]))
    by_model = defaultdict(lambda: defaultdict(lambda: [0, 0]))
    raw_counts = defaultdict(lambda: [0, 0])       # exact count -> [k, n]

    for r in rows:
        c = int(r["expected"])
        ok = 1 if r["status"] == "correct" else 0
        lbl = bin_of(c)
        overall[lbl][0] += ok
        overall[lbl][1] += 1
        by_enc[r["encoding"]][lbl][0] += ok
        by_enc[r["encoding"]][lbl][1] += 1
        by_model[r["model"]][lbl][0] += ok
        by_model[r["model"]][lbl][1] += 1
        raw_counts[c][0] += ok
        raw_counts[c][1] += 1

    print("# Region-count accuracy vs. true count")
    print(f"\n{len(rows)} region-count trials pooled across the supplied files.")
    print(f"Encodings: {sorted(by_enc)}")
    print(f"Conditions (model/think): {len(by_model)}")

    print("\n### n per true-count bin (READ THIS FIRST)\n```")
    print(f"{'bin':<8}{'n':>6}{'k(correct)':>12}{'acc':>8}{'  95% Wilson CI':<18}")
    for lbl, _ in BINS:
        k, n = overall[lbl]
        if n:
            lo, hi = wilson(k, n)
            print(f"{lbl:<8}{n:>6}{k:>12}{k/n:>8.2f}  [{lo:.2f}, {hi:.2f}]")
        else:
            print(f"{lbl:<8}{n:>6}{k:>12}{'-':>8}")
    print("```")

    print_curve("Overall accuracy-vs-count curve (all encodings, all models pooled)",
                {"ALL trials": overall}, order=["ALL trials"])

    enc_groups = {enc: by_enc[enc] for enc in by_enc}
    print_curve("Per ENCODING (does encoding shift the cliff?)", enc_groups,
                order=sorted(enc_groups, key=lambda e: -sum(v[1] for v in enc_groups[e].values())))

    model_groups = {m: by_model[m] for m in by_model}
    print_curve("Per MODEL / think-mode", model_groups,
                order=sorted(model_groups, key=lambda m: -sum(v[1] for v in model_groups[m].values())))

    print("\n### Exact per-count accuracy (unbinned)\n```")
    print(f"{'count':>6}{'n':>6}{'k':>6}{'acc':>8}")
    for c in sorted(raw_counts):
        k, n = raw_counts[c]
        print(f"{c:>6}{n:>6}{k:>6}{k/n:>8.2f}")
    print("```")
    return 0


if __name__ == "__main__":
    lo, hi = wilson(8, 10)
    assert abs(lo - 0.4901) < 0.01 and abs(hi - 0.9432) < 0.01
    sys.exit(main(sys.argv[1:]))
