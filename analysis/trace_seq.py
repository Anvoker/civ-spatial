import json, sys, collections

def parse_args(a):
    if isinstance(a, dict): return a
    try: return json.loads(a) if a else {}
    except Exception: return {}

def fmt(v, a):
    if v == "scan_grid" or v == "scan":
        return f"{v}({a.get('x0')},{a.get('y0')}-{a.get('x1')},{a.get('y1')})"
    if v == "get_tile":
        return f"get_tile({a.get('x')},{a.get('y')})"
    if v == "region_summary":
        return f"rs({a.get('sector')})"
    return f"{v}()"

def show(path):
    d = json.load(open(path, encoding="utf-8"))
    print(f"### {d['item_id']}  [{d['category']}]  status={d['result'].get('status')}  turns={len(d['turns'])}")
    print(f"    result: {json.dumps(d['result'])}")
    for i, t in enumerate(d["turns"]):
        tcs = t["response"].get("tool_calls") or []
        cnt = collections.Counter(c["name"] for c in tcs)
        calls = [fmt(c["name"], parse_args(c.get("args"))) for c in tcs]
        txt = (t["response"].get("text") or "").strip().replace("\n", " ")
        ans = ""
        if "Answer:" in txt:
            ans = "  >>> " + txt[txt.index("Answer:"):txt.index("Answer:")+80]
        # compress long call lists
        shown = calls if len(calls) <= 10 else calls[:8] + [f"...(+{len(calls)-8} more)"]
        print(f"  T{i:2} [{len(tcs):3} calls {dict(cnt)}] {'; '.join(shown)}{ans}")

if __name__ == "__main__":
    for p in sys.argv[1:]:
        show(p); print()
