#!/usr/bin/env python3
"""Pick a fog perspective per corpus board, then measure per-kind question yield.

Two jobs, one pass over a chosen set of boards:

1. **Fog perspective** — for each board, tally per-player city/unit counts from our
   parser's `dump-board` DETAIL section, drop barbarian/pirate/animal + dead players,
   and pick the *largest sustained civ*: max by (n_cities, n_units, -id). Cities are
   the durable footprint; the max is substantial by construction (excludes "tiny");
   and since these games kept 5-13 civs to T221 there is no runaway hegemon, so the
   pick is a major power with meaningful fog — not an omniscient one. Winners with
   < MIN_CITIES cities are FLAGGED (possible degenerate board), not auto-dropped.

2. **Yield matrix** — run `gen-questions --board B --fog <id> --per-kind K` (the exact
   fogged experiment condition: board masked to that player + player-relative kinds
   pinned to it) and parse the "[kind] kept N instances" lines into a kind x board
   matrix, so corpus adequacy rests on real per-kind counts, not eyeballing.

Player id == the order of the `players=[...]` header (verified: id 0 = the first tuple).
Owner names in DETAIL are LEADER names, matched back to id via that header order.

Usage:
  python scripts/corpus-yield.py --boards-file <file-of-savenames> [--saves-dir data/corpus/saves]
  python scripts/corpus-yield.py --board data/corpus/saves/foo.sav   # single board
"""
from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

MIN_CITIES = 3  # winners below this are flagged as possibly-degenerate
PER_KIND = 12   # question quota per kind when measuring yield
NON_CIV_NATIONS = {"Barbarian", "Pirate", "Animal Kingdom", "Animal"}

RE_HEADER = re.compile(r'\("((?:[^"\\]|\\.)*)",\s*"((?:[^"\\]|\\.)*)"\)')  # (leader, nation)
RE_CITY_OWNER = re.compile(r"\{city\b[^}]*\bowner ([^}]+)\}")
RE_UNIT_OWNER = re.compile(r"\{unit\b[^}]*\bowner ([^}]+)\}")
RE_KEPT = re.compile(r"\[([a-z0-9-]+)\] kept (\d+) instance")


def find_cli() -> str:
    for cand in ("target/debug/civ.exe", "target/debug/civ",
                 "target/release/civ.exe", "target/release/civ"):
        if Path(cand).exists():
            return str(Path(cand).resolve())
    sys.exit("civ CLI not built. Run: cargo build -p civ-cli")


def dump(cli: str, board: Path) -> str:
    return subprocess.run([cli, "dump-board", "--board", str(board)],
                          capture_output=True, text=True, check=True).stdout


def pick_perspective(dump_text: str):
    """Return (id, leader, nation, n_cities, n_units, flags, roster) for the chosen civ."""
    header = dump_text.splitlines()[0]
    players = RE_HEADER.findall(header)  # ordered -> index is the player id
    leader_to_id = {leader: i for i, (leader, _nation) in enumerate(players)}
    city_ct = {i: 0 for i in range(len(players))}
    unit_ct = {i: 0 for i in range(len(players))}
    for owner in RE_CITY_OWNER.findall(dump_text):
        if owner in leader_to_id:
            city_ct[leader_to_id[owner]] += 1
    for owner in RE_UNIT_OWNER.findall(dump_text):
        if owner in leader_to_id:
            unit_ct[leader_to_id[owner]] += 1

    roster = []
    candidates = []
    for i, (leader, nation) in enumerate(players):
        c, u = city_ct[i], unit_ct[i]
        roster.append((i, leader, nation, c, u))
        if nation in NON_CIV_NATIONS:
            continue                       # barbarian/pirate/animal
        if c == 0 or u == 0:
            continue                       # dead (no city or no unit)
        candidates.append((i, leader, nation, c, u))

    if not candidates:
        return None, None, None, 0, 0, ["NO-ELIGIBLE-CIV"], roster
    # max by cities, then units, then LOWEST id (determinism)
    best = max(candidates, key=lambda r: (r[3], r[4], -r[0]))
    i, leader, nation, c, u = best
    flags = []
    if c < MIN_CITIES:
        flags.append(f"WINNER<{MIN_CITIES}-CITIES")
    return i, leader, nation, c, u, flags, roster


def yield_for(cli: str, board: Path, fog_id: int) -> dict[str, int]:
    r = subprocess.run(
        [cli, "gen-questions", "--board", str(board), "--fog", str(fog_id),
         "--per-kind", str(PER_KIND)],
        capture_output=True, text=True)
    text = r.stdout + r.stderr
    kept = {}
    for kind, n in RE_KEPT.findall(text):
        kept[kind] = int(n)
    if not kept:
        print(f"  WARN {board.name}: no yield parsed (fog {fog_id}). stderr tail:\n"
              f"    {text.strip().splitlines()[-1] if text.strip() else '(empty)'}",
              file=sys.stderr)
    return kept


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--saves-dir", default="data/corpus/saves")
    ap.add_argument("--boards-file", default=None,
                    help="file with one savename (or path) per line; # comments ok")
    ap.add_argument("--board", default=None, help="single board path")
    ap.add_argument("--out", default="data/corpus/yield")
    args = ap.parse_args()

    cli = find_cli()
    if args.board:
        boards = [Path(args.board)]
    elif args.boards_file:
        names = [ln.strip() for ln in Path(args.boards_file).read_text().splitlines()
                 if ln.strip() and not ln.startswith("#")]
        boards = [Path(n) if "/" in n or "\\" in n else Path(args.saves_dir) / n
                  for n in names]
    else:
        # full-pool sweep: every .sav in the saves dir
        boards = sorted(Path(args.saves_dir).glob("*.sav"))
        if not boards:
            sys.exit(f"no .sav found in {args.saves_dir}")

    # Optional band/size/seed/aifill from a prebuilt manifest (keyed by savename).
    meta_by_name: dict[str, dict] = {}
    man = Path("data/corpus/manifest.json")
    if man.exists():
        import json as _json
        for r in _json.loads(man.read_text()).get("boards", []):
            meta_by_name[r["source"]] = r

    rows = []
    all_kinds: list[str] = []
    for n, b in enumerate(boards, 1):
        if not b.exists():
            print(f"  MISSING {b}", file=sys.stderr)
            continue
        d = dump(cli, b)
        fog_id, leader, nation, c, u, flags, _roster = pick_perspective(d)
        if fog_id is None:
            print(f"  SKIP {b.name}: {flags}", file=sys.stderr)
            continue
        kept = yield_for(cli, b, fog_id)
        for k in kept:
            if k not in all_kinds:
                all_kinds.append(k)
        m = meta_by_name.get(b.name, {})
        rows.append({"board": b.name, "fog_id": fog_id, "leader": leader,
                     "nation": nation, "cities": c, "units": u,
                     "flags": flags, "yield": kept,
                     "band": m.get("band"), "size_name": m.get("size_name"),
                     "seed": m.get("seed"), "aifill": m.get("aifill"),
                     "turn": m.get("turn"), "density": m.get("density")})
        fl = (" " + " ".join(flags)) if flags else ""
        print(f"[{n}/{len(boards)}] {b.name:52} fog={fog_id} {leader!r} "
              f"({nation}) c{c} u{u}{fl}")

    all_kinds.sort()
    # --- JSON (per-board records + global per-kind support ceiling) ---
    import json as _json
    ceiling = {k: sum(1 for r in rows if r["yield"].get(k, 0) >= 1) for k in all_kinds}
    Path(args.out + ".json").write_text(_json.dumps(
        {"meta": {"n_boards": len(rows), "per_kind": PER_KIND,
                  "support_ceiling": ceiling}, "boards": rows},
        indent=2), encoding="utf-8")
    # --- markdown matrix ---
    md = ["# Corpus fog perspective + per-kind yield", "",
          f"- boards: **{len(rows)}**  | per-kind quota **{PER_KIND}**  "
          f"| perspective = largest sustained civ (max cities, then units)  "
          f"| min-cities flag = **{MIN_CITIES}**", ""]
    md.append("## Fog perspective per board\n")
    md.append("| board | fog id | leader | nation | cities | units | flags |")
    md.append("|---|---|---|---|---|---|---|")
    for r in rows:
        md.append(f"| {r['board']} | {r['fog_id']} | {r['leader']} | {r['nation']} "
                  f"| {r['cities']} | {r['units']} | {' '.join(r['flags'])} |")
    md += ["", "## Per-kind yield (kept / %d)" % PER_KIND, ""]
    head = "| kind | " + " | ".join(str(i) for i in range(len(rows))) + " | min | boards≥1 |"
    md.append(head)
    md.append("|" + "---|" * (len(rows) + 3))
    for k in all_kinds:
        vals = [r["yield"].get(k, 0) for r in rows]
        nz = sum(1 for v in vals if v >= 1)
        md.append(f"| {k} | " + " | ".join(str(v) for v in vals)
                  + f" | {min(vals)} | {nz}/{len(rows)} |")
    md += ["", "board index → name:", ""]
    for i, r in enumerate(rows):
        md.append(f"{i}. {r['board']}  (fog {r['fog_id']} {r['leader']})")
    Path(args.out + ".md").write_text("\n".join(md) + "\n", encoding="utf-8")
    print(f"\nwrote {args.out}.md  ({len(rows)} boards x {len(all_kinds)} kinds)")

    # thin-support summary
    thin = [k for k in all_kinds if sum(1 for r in rows if r["yield"].get(k, 0) >= 1) < len(rows)]
    if thin:
        print("kinds NOT supported on every board:")
        for k in thin:
            nz = sum(1 for r in rows if r["yield"].get(k, 0) >= 1)
            print(f"  {k}: {nz}/{len(rows)} boards")


if __name__ == "__main__":
    main()
