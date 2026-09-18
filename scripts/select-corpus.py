#!/usr/bin/env python3
"""Coverage-optimized corpus selection from the full-pool yield sweep.

Reads `corpus-yield.py --saves-dir ... ` JSON (per-board fog perspective + per-kind
yield over ALL boards) and greedily picks N boards to MAXIMIZE per-kind support,
weighting rare (frontier) kinds highest — instead of guessing by band/size.

Objective (greedy, marginal-gain):  sum_k  W_k * min(support_selected_k, CAP_k)
  W_k    = 1 / ceiling_k   (ceiling_k = # of ALL boards that support kind k)
           -> rarer kinds dominate the objective, so they get covered first.
  CAP_k  = min(CAP, ceiling_k)   (diminishing returns: beyond CAP boards a kind
           has enough statistical power, so stop rewarding it and spread elsewhere.)

Constraints:
  * HARD one-board-per-game (a game = size x aifill x seed): snapshots of the same
    game at different turns are correlated, so at most one enters the corpus. With
    18 games and N=15 that yields 15 distinct games (matches the prior design, now
    coverage-driven rather than hand-picked).
  * SOFT band floor: >= MIN_PER_BAND early/mid/late, force-filled near the end so the
    perception-kind regime diversity (sparse early ... dense late) is preserved even
    though the objective itself doesn't care about bands.

Prints the chosen set, its per-kind support vs the global ceiling (how close to the
best achievable), and band/size/game composition. Writes <out>.txt (savename list,
ready for corpus-yield.py --boards-file) + <out>.md (the report).
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

CAP = 8            # per-kind diminishing-returns cap (enough boards for decent power)
MIN_PER_BAND = 3   # soft floor: >= this many early / mid / late in the final set
# Perception kinds are supported ~everywhere; list them so the report can separate
# "free" floor kinds from the discriminating kinds the selection actually optimizes.
PERCEPTION = {"best-site", "constraint-site", "nearest-owned", "reachable-nearest"}


def game_of(r: dict) -> tuple:
    return (r.get("size_name"), r.get("aifill"), r.get("seed"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--yield-json", default="data/corpus/yield-fullpool.json")
    ap.add_argument("--n", type=int, default=15)
    ap.add_argument("--out", default="data/corpus/selection-optimized")
    args = ap.parse_args()

    data = json.loads(Path(args.yield_json).read_text())
    boards = data["boards"]
    ceiling = data["meta"]["support_ceiling"]           # kind -> # boards supporting it
    kinds = sorted(ceiling)
    disc = [k for k in kinds if k not in PERCEPTION]     # discriminating kinds

    # weights + per-kind caps
    W = {k: 1.0 / max(1, ceiling[k]) for k in kinds}
    KCAP = {k: min(CAP, ceiling[k]) for k in kinds}

    def supported(r, k):
        return r["yield"].get(k, 0) >= 1

    selected: list[dict] = []
    used_games: set = set()
    supp = {k: 0 for k in kinds}                          # current support counts

    def marginal(r):
        g = 0.0
        for k in kinds:
            if supported(r, k) and supp[k] < KCAP[k]:
                g += W[k]
        return g

    band_count = {"early": 0, "mid": 0, "late": 0}
    for _ in range(args.n):
        slots_left = args.n - len(selected)
        unmet = {b: MIN_PER_BAND - band_count.get(b, 0)
                 for b in ("early", "mid", "late")}
        total_unmet = sum(v for v in unmet.values() if v > 0)
        pool = [r for r in boards if game_of(r) not in used_games]
        if not pool:
            break
        # force band floors when remaining slots are just enough to meet them
        if slots_left <= total_unmet:
            need = {b for b, v in unmet.items() if v > 0}
            forced = [r for r in pool if r.get("band") in need]
            if forced:
                pool = forced
        # tie-break: higher marginal, then more discriminating kinds, then more units
        best = max(pool, key=lambda r: (marginal(r),
                                        sum(1 for k in disc if supported(r, k)),
                                        r.get("units") or 0))
        selected.append(best)
        used_games.add(game_of(best))
        band_count[best.get("band")] = band_count.get(best.get("band"), 0) + 1
        for k in kinds:
            if supported(best, k):
                supp[k] += 1

    # --- report ---
    sizes = {}
    for r in selected:
        sizes[r.get("size_name")] = sizes.get(r.get("size_name"), 0) + 1
    disc_min = min(supp[k] for k in disc)
    disc_min_kind = min(disc, key=lambda k: supp[k])

    lines = ["# Coverage-optimized corpus selection", "",
             f"- N = **{len(selected)}** boards | distinct games = "
             f"**{len(used_games)}** | CAP {CAP} | band floor {MIN_PER_BAND}",
             f"- bands: {band_count} | sizes: {sizes}",
             f"- weakest discriminating kind: **{disc_min_kind} = {disc_min}** "
             f"(ceiling {ceiling[disc_min_kind]})", "",
             "## Per-kind support: chosen / global ceiling", "",
             "| kind | chosen | ceiling | | kind | chosen | ceiling |"]
    lines.append("|---|---|---|---|---|---|---|")
    half = (len(kinds) + 1) // 2
    for i in range(half):
        left = kinds[i]
        cell = f"| {left} | {supp[left]} | {ceiling[left]} |"
        j = i + half
        if j < len(kinds):
            rk = kinds[j]
            cell += f" | {rk} | {supp[rk]} | {ceiling[rk]} |"
        else:
            cell += " | | | |"
        lines.append(cell)
    lines += ["", "## Chosen boards", "",
              "| # | board | band | size | aifill | seed | fog | leader | disc-kinds |",
              "|---|---|---|---|---|---|---|---|---|"]
    for i, r in enumerate(selected):
        nd = sum(1 for k in disc if supported(r, k))
        lines.append(f"| {i} | {r['board']} | {r.get('band')} | {r.get('size_name')} "
                     f"| {r.get('aifill')} | {r.get('seed')} | {r['fog_id']} "
                     f"| {r['leader']} | {nd}/{len(disc)} |")
    Path(args.out + ".md").write_text("\n".join(lines) + "\n", encoding="utf-8")

    txt = ["# Coverage-optimized 15-board corpus (select-corpus.py).",
           f"# {len(used_games)} distinct games; bands {band_count}; sizes {sizes}.",
           f"# weakest discriminating kind: {disc_min_kind}={disc_min}."]
    order = {"early": 0, "mid": 1, "late": 2}
    for r in sorted(selected, key=lambda r: (order.get(r.get("band"), 9), r["board"])):
        txt.append(r["board"])
    Path(args.out + ".txt").write_text("\n".join(txt) + "\n", encoding="utf-8")

    print(f"selected {len(selected)} boards ({len(used_games)} games); "
          f"bands={band_count} sizes={sizes}")
    print(f"weakest discriminating kind: {disc_min_kind}={disc_min} "
          f"(ceiling {ceiling[disc_min_kind]})")
    print("discriminating-kind support (chosen/ceiling):")
    for k in sorted(disc, key=lambda k: supp[k]):
        print(f"  {k:22} {supp[k]:2}/{ceiling[k]}")
    print(f"wrote {args.out}.md + {args.out}.txt")


if __name__ == "__main__":
    main()
