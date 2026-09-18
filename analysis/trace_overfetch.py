import json, glob, sys, collections, itertools, os

KNOWN_TILES = 319          # non-Unknown tiles on T677 masked to p4 (measured from raw trace)
BOARD_W, BOARD_H = 84, 56
RAW_BOARD_CHARS = 13893    # raw whole-board block chars (319 tiles)
RAW_BOARD_TOK = 5254       # raw prompt_tokens (system+board+question) ~ "whole-board block ~5.4k"

TILE_VERBS = {"scan", "scan_grid", "get_tile"}
NOTILE_VERBS = {"region_summary", "list_cities", "list_units"}

KNOWN = set(tuple(c) for c in json.load(open("traces/known_coords_p4.json")))

def parse_args(a):
    if isinstance(a, dict):
        return a
    try:
        return json.loads(a) if a else {}
    except Exception:
        return {}

def bbox_tiles(x0, y0, x1, y1):
    xa, xb = sorted((x0, x1)); ya, yb = sorted((y0, y1))
    xa = max(0, xa); ya = max(0, ya); xb = min(BOARD_W-1, xb); yb = min(BOARD_H-1, yb)
    return {(x, y) for x in range(xa, xb+1) for y in range(ya, yb+1)}, (xa, ya, xb, yb)

def boxes_overlap(a, b):
    ax0, ay0, ax1, ay1 = a; bx0, by0, bx1, by1 = b
    return not (ax1 < bx0 or bx1 < ax0 or ay1 < by0 or by1 < ay0)

def analyze(path):
    d = json.load(open(path, encoding="utf-8"))
    turns = d["turns"]
    seq = []            # (verb, args)
    verb_ct = collections.Counter()
    gross = 0
    union = set()
    tile_multiplicity = collections.Counter()
    boxes = []          # clipped bboxes of scan/scan_grid
    first_answer_turn = None
    for i, t in enumerate(turns):
        resp = t["response"]
        txt = resp.get("text") or ""
        if first_answer_turn is None and "Answer:" in txt:
            first_answer_turn = i
        for tc in (resp.get("tool_calls") or []):
            v = tc["name"]; a = parse_args(tc.get("args"))
            seq.append((v, a)); verb_ct[v] += 1
            if v in ("scan", "scan_grid"):
                try:
                    tiles, clip = bbox_tiles(int(a["x0"]), int(a["y0"]), int(a["x1"]), int(a["y1"]))
                except Exception:
                    continue
                gross += len(tiles); union |= tiles
                for tt in tiles: tile_multiplicity[tt] += 1
                boxes.append(clip)
            elif v == "get_tile":
                try:
                    x, y = int(a["x"]), int(a["y"])
                except Exception:
                    continue
                gross += 1; union.add((x, y)); tile_multiplicity[(x, y)] += 1

    net = len(union)
    net_known = len(union & KNOWN)          # fetched tiles that are actually explored
    net_unknown = net - net_known           # fetched tiles that are fog (wasted)
    refetch = sum(1 for k, c in tile_multiplicity.items() if c > 1)
    overlap_pairs = sum(1 for a, b in itertools.combinations(boxes, 2) if boxes_overlap(a, b))

    # fetched board-content size: sum of tool-result message chars in the most-complete transcript
    best_msgs = max((t["request"].get("messages", []) for t in turns), key=len, default=[])
    tool_chars = sum(len(m["content"]) for m in best_msgs
                     if m.get("role") == "tool" and isinstance(m.get("content"), str))

    n_calls = sum(verb_ct.values())
    hit_cap = len(turns) >= 16
    status = d["result"].get("status")
    return {
        "item": d["item_id"], "kind": d["category"], "status": status,
        "n_turns": len(turns), "n_calls": n_calls, "verbs": dict(verb_ct),
        "gross": gross, "net": net, "net_known": net_known, "net_unknown": net_unknown,
        "redundancy": (gross/net) if net else 0.0,
        "board_frac": net_known/KNOWN_TILES,
        "refetch": refetch, "overlap_pairs": overlap_pairs,
        "tool_chars": tool_chars,
        "first_answer_turn": first_answer_turn, "hit_cap": hit_cap,
        "seq": seq,
    }

def main(pattern):
    files = sorted(glob.glob(pattern))
    rows = [analyze(f) for f in files]
    print(f"# {len(rows)} trajectories from {pattern}\n")
    hdr = ["kind","status","turns","calls","gross","net","nKnown","nFog","redund","kboardX","refetch","ovlpBox","toolK","1stAns","cap"]
    print("  ".join(f"{h:>8}" for h in hdr))
    for r in rows:
        print("  ".join(f"{str(x):>8}" for x in [
            r["kind"][:8], r["status"][:8], r["n_turns"], r["n_calls"],
            r["gross"], r["net"], r["net_known"], r["net_unknown"],
            f'{r["redundancy"]:.2f}', f'{r["board_frac"]:.2f}',
            r["refetch"], r["overlap_pairs"], f'{r["tool_chars"]/1000:.1f}',
            r["first_answer_turn"], "Y" if r["hit_cap"] else "-"]))

    def agg(sub, label):
        n = len(sub)
        if not n: return
        gv = sum(r["gross"] for r in sub); nv = sum(r["net"] for r in sub)
        vb = collections.Counter()
        for r in sub: vb.update(r["verbs"])
        print(f"\n## {label} (n={n})")
        print(f"  mean tool-calls: {sum(r['n_calls'] for r in sub)/n:.1f}   verbs: {dict(vb)}")
        print(f"  mean gross tiles: {gv/n:.0f}   mean net unique: {nv/n:.0f}   "
              f"pooled redundancy gross/net: {gv/nv:.2f}   mean per-traj redund: {sum(r['redundancy'] for r in sub)/n:.2f}")
        print(f"  mean net-known/319: {sum(r['board_frac'] for r in sub)/n:.2f}   "
              f"mean net-fog(wasted) tiles: {sum(r['net_unknown'] for r in sub)/n:.0f}   "
              f"mean refetch tiles: {sum(r['refetch'] for r in sub)/n:.0f}   mean overlap box-pairs: {sum(r['overlap_pairs'] for r in sub)/n:.1f}")
        tc = sum(r["tool_chars"] for r in sub)/n
        # est model tokens for fetched content: 0.3734 model-tok/char (calibrated from raw trace)
        est_tok = tc*0.3734
        print(f"  mean fetched tool-content: {tc/1000:.1f}k chars  ~{est_tok/1000:.1f}k model-tok  "
              f"= {est_tok/RAW_BOARD_TOK:.2f}x raw board block ({RAW_BOARD_TOK} tok); "
              f"gross-tiles/319 = {gv/n/KNOWN_TILES:.2f}x board")
        print(f"  mean turns: {sum(r['n_turns'] for r in sub)/n:.1f}   hit cap: {sum(r['hit_cap'] for r in sub)}/{n}   "
              f"status: {collections.Counter(r['status'] for r in sub)}")

    kinds = sorted(set(r["kind"] for r in rows))
    for k in kinds:
        agg([r for r in rows if r["kind"] == k], f"KIND {k}")
    agg(rows, "OVERALL")

    # correctness correlation
    corr = [r for r in rows if r["status"] == "correct"]
    bad = [r for r in rows if r["status"] != "correct"]
    def m(sub, key): return (sum(r[key] for r in sub)/len(sub)) if sub else 0
    print("\n## OVER-FETCH vs CORRECTNESS")
    print(f"  correct (n={len(corr)}): gross {m(corr,'gross'):.0f}  net {m(corr,'net'):.0f}  "
          f"redund {m(corr,'redundancy'):.2f}  calls {m(corr,'n_calls'):.1f}  refetch {m(corr,'refetch'):.0f}  cap {sum(r['hit_cap'] for r in corr)}")
    print(f"  wrong+invalid (n={len(bad)}): gross {m(bad,'gross'):.0f}  net {m(bad,'net'):.0f}  "
          f"redund {m(bad,'redundancy'):.2f}  calls {m(bad,'n_calls'):.1f}  refetch {m(bad,'refetch'):.0f}  cap {sum(r['hit_cap'] for r in bad)}")

    return rows

if __name__ == "__main__":
    main(sys.argv[1])
