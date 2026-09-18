#!/usr/bin/env python3
"""Characterize the collected corpus saves into a manifest, using OUR parser.

For every save in --saves-dir this records
  {turn, w, h, n_units, n_cities, n_players, density, ruleset, band, seed, aifill,
   size_name, source}
so the manifest reflects exactly what our decoder sees -- board dims / unit / city /
player counts come from `civ dump-board` (our Rust parser), turn + ruleset are read
straight from the save text, and seed/aifill/size are joined from the generator's
`sweep.json` (falling back to parsing the savename prefix).

Bands: early (turn <= 60), mid (61..140), late (>140).  [overridable via sweep.json meta]

Outputs:
  <out>.json  -- machine-readable list of per-board records
  <out>.md    -- a human-readable table + the early/mid/late yield tally

Reuses the parser only through the CLI (never imports/edits civ-core). Also works
standalone on any saves dir (e.g. data/saves) for a dry run -- seed/aifill just show
as null when there's no sweep.json.

Usage:
  python scripts/build-manifest.py --saves-dir data/corpus/saves
  python scripts/build-manifest.py --saves-dir data/saves --out data/corpus/manifest-smoke
"""
from __future__ import annotations

import argparse
import gzip
import json
import lzma
import re
import shutil
import subprocess
import sys
from pathlib import Path

# dump-board line 1:  "<source>  <W>x<H>  players=[(...), (...)]"
RE_DIMS = re.compile(r"\s(\d+)x(\d+)\s")
RE_PLAYER_TUPLE = re.compile(r"\(")            # count '(' in the players=[...] list
RE_COUNTS = re.compile(r"cities=(\d+)\s+units=(\d+)")
RE_RULESET = re.compile(r'^\s*rulesetdir\s*=\s*"?([^"\r\n]+)"?', re.MULTILINE)
RE_TURN = re.compile(r"^\s*turn\s*=\s*(\d+)", re.MULTILINE)
# savename prefix fallback:  corpus_<size>_a<aifill>_s<seed>
RE_SAVENAME = re.compile(r"^(corpus_(?P<size>[a-z]+)_a(?P<aifill>\d+)_s(?P<seed>\d+))")


def find_cli() -> str:
    for cand in ("target/debug/civ.exe", "target/debug/civ",
                 "target/release/civ.exe", "target/release/civ"):
        p = Path(cand)
        if p.exists():
            return str(p.resolve())
    sys.exit("civ CLI not built. Run: cargo build -p civ-cli")


def decompress_if_needed(path: Path) -> Path:
    """Our parser wants plaintext. The scripts force compress=0, but handle the
    common compressed extensions anyway (stdlib gzip/xz; .zst is skipped w/ warning)."""
    if path.suffix == ".sav":
        return path
    plain = path.with_suffix("")  # strip the outer compression suffix
    if plain.exists():
        return plain
    if path.suffix == ".gz":
        with gzip.open(path, "rb") as src, open(plain, "wb") as dst:
            shutil.copyfileobj(src, dst)
        return plain
    if path.suffix == ".xz":
        with lzma.open(path, "rb") as src, open(plain, "wb") as dst:
            shutil.copyfileobj(src, dst)
        return plain
    print(f"  WARN: cannot decompress {path.name} (unsupported); "
          f"set compress 0 / compresstype PLAIN in the .serv", file=sys.stderr)
    return path


def dump_board(cli: str, sav: Path) -> dict:
    out = subprocess.run([cli, "dump-board", "--board", str(sav)],
                         capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip() or "dump-board failed")
    lines = out.stdout.splitlines()
    head = lines[0] if lines else ""
    dims = RE_DIMS.search(head)
    if not dims:
        raise RuntimeError(f"could not parse dims from: {head!r}")
    w, h = int(dims.group(1)), int(dims.group(2))
    players_part = head.split("players=", 1)[1] if "players=" in head else ""
    n_players = len(RE_PLAYER_TUPLE.findall(players_part))
    counts = RE_COUNTS.search(out.stdout)
    n_cities = int(counts.group(1)) if counts else 0
    n_units = int(counts.group(2)) if counts else 0
    return {"w": w, "h": h, "n_players": n_players,
            "n_cities": n_cities, "n_units": n_units}


def read_save_meta(sav: Path) -> dict:
    """turn + ruleset straight from the save text (authoritative)."""
    text = sav.read_text(encoding="utf-8", errors="replace")
    rs = RE_RULESET.search(text)
    turn = RE_TURN.search(text)
    return {"ruleset": rs.group(1).strip() if rs else None,
            "turn": int(turn.group(1)) if turn else None}


def band_of(turn: int | None, early_max: int, mid_max: int) -> str:
    if turn is None:
        return "unknown"
    if turn <= early_max:
        return "early"
    if turn <= mid_max:
        return "mid"
    return "late"


def load_sweep(saves_dir: Path, explicit: str | None) -> tuple[dict, int, int]:
    """Return (cells_by_savename, early_max, mid_max)."""
    candidates = []
    if explicit:
        candidates.append(Path(explicit))
    candidates += [saves_dir / "sweep.json",
                   saves_dir.parent / "scripts" / "sweep.json"]
    for c in candidates:
        if c and c.exists():
            data = json.loads(c.read_text(encoding="utf-8"))
            meta = data.get("meta", {})
            thr = meta.get("band_thresholds", {})
            return (data.get("cells", {}),
                    int(thr.get("early_max", 60)),
                    int(thr.get("mid_max", 140)))
    return {}, 60, 140


def cell_for(savename_stem: str, cells: dict) -> dict:
    """Join a save to its sweep cell by savename prefix; fall back to the filename."""
    for name, cell in cells.items():
        if savename_stem.startswith(name):
            return {"seed": cell.get("mapseed"), "aifill": cell.get("aifill"),
                    "size_name": cell.get("size_name")}
    m = RE_SAVENAME.match(savename_stem)
    if m:
        return {"seed": int(m.group("seed")), "aifill": int(m.group("aifill")),
                "size_name": m.group("size")}
    return {"seed": None, "aifill": None, "size_name": None}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--saves-dir", default="data/corpus/saves")
    ap.add_argument("--out", default="data/corpus/manifest",
                    help="output stem; writes <out>.json and <out>.md")
    ap.add_argument("--sweep", default=None, help="explicit path to sweep.json")
    ap.add_argument("--expect-ruleset", default="classic",
                    help="flag any board whose ruleset != this (default classic)")
    args = ap.parse_args()

    cli = find_cli()
    saves_dir = Path(args.saves_dir)
    if not saves_dir.exists():
        sys.exit(f"saves dir not found: {saves_dir}")

    cells, early_max, mid_max = load_sweep(saves_dir, args.sweep)

    # Collect saves: plaintext .sav plus compressed variants (decompressed on the fly).
    raw = sorted([p for p in saves_dir.iterdir()
                  if p.suffix in (".sav", ".gz", ".xz")])
    records: list[dict] = []
    mismatches: list[str] = []
    failures: list[str] = []

    for path in raw:
        sav = decompress_if_needed(path)
        if sav.suffix != ".sav":
            continue
        try:
            board = dump_board(cli, sav)
            meta = read_save_meta(sav)
        except Exception as e:  # noqa: BLE001 -- report, keep going
            failures.append(f"{path.name}: {e}")
            print(f"  FAIL {path.name}: {e}", file=sys.stderr)
            continue
        stem = sav.stem
        cell = cell_for(stem, cells)
        density = round((board["n_units"] + board["n_cities"]) / (board["w"] * board["h"]), 5)
        rec = {
            "source": sav.name,
            "turn": meta["turn"],
            "band": band_of(meta["turn"], early_max, mid_max),
            "w": board["w"], "h": board["h"],
            "n_units": board["n_units"], "n_cities": board["n_cities"],
            "n_players": board["n_players"],
            "density": density,
            "ruleset": meta["ruleset"],
            "seed": cell["seed"], "aifill": cell["aifill"],
            "size_name": cell["size_name"],
        }
        records.append(rec)
        if args.expect_ruleset and meta["ruleset"] != args.expect_ruleset:
            mismatches.append(f"{sav.name}: ruleset={meta['ruleset']!r}")

    records.sort(key=lambda r: (r["size_name"] or "", r["aifill"] or 0, r["turn"] or 0))

    # --- band tally ---
    bands = {"early": 0, "mid": 0, "late": 0, "unknown": 0}
    for r in records:
        bands[r["band"]] = bands.get(r["band"], 0) + 1

    out_json = Path(args.out + ".json")
    out_json.parent.mkdir(parents=True, exist_ok=True)
    out_json.write_text(json.dumps({
        "meta": {"early_max": early_max, "mid_max": mid_max,
                 "expect_ruleset": args.expect_ruleset,
                 "band_counts": bands, "n_boards": len(records),
                 "ruleset_mismatches": mismatches, "parse_failures": failures},
        "boards": records,
    }, indent=2), encoding="utf-8")

    # --- markdown table ---
    md = ["# CivSpatial multi-board corpus manifest", "",
          f"- boards: **{len(records)}**  |  "
          f"early(<= {early_max}): **{bands['early']}**  "
          f"mid(<= {mid_max}): **{bands['mid']}**  "
          f"late(> {mid_max}): **{bands['late']}**",
          f"- ruleset expected: `{args.expect_ruleset}`  |  "
          f"mismatches: **{len(mismatches)}**  |  parse failures: **{len(failures)}**",
          f"- band target for the experiment: >= 3 early / >= 3 mid / >= 3 late -> "
          f"**{'MET' if bands['early'] >= 3 and bands['mid'] >= 3 and bands['late'] >= 3 else 'NOT yet met'}**",
          "",
          "| source | band | turn | size | WxH | players | cities | units | density | ruleset | seed | aifill |",
          "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in records:
        md.append(
            f"| {r['source']} | {r['band']} | {r['turn']} | {r['size_name']} | "
            f"{r['w']}x{r['h']} | {r['n_players']} | {r['n_cities']} | {r['n_units']} | "
            f"{r['density']} | {r['ruleset']} | {r['seed']} | {r['aifill']} |")
    if mismatches:
        md += ["", "## Ruleset mismatches (EXCLUDE from the corpus)", ""]
        md += [f"- {m}" for m in mismatches]
    if failures:
        md += ["", "## Parse failures", ""]
        md += [f"- {f}" for f in failures]
    Path(args.out + ".md").write_text("\n".join(md) + "\n", encoding="utf-8")

    print(f"manifest: {len(records)} boards -> {out_json} + {args.out}.md")
    print(f"  bands: early={bands['early']} mid={bands['mid']} late={bands['late']} "
          f"unknown={bands['unknown']}")
    if mismatches:
        print(f"  WARN: {len(mismatches)} non-{args.expect_ruleset} board(s) -- exclude them:")
        for m in mismatches:
            print(f"    {m}")
    if failures:
        print(f"  WARN: {len(failures)} parse failure(s).")


if __name__ == "__main__":
    main()
