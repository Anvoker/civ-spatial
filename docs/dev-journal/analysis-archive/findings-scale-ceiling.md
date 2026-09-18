# Findings — the ~0.75 scale ceiling is a LOCATING wall, not a computing wall

> **⚠️ REVISED 2026-08-01 — this analysis was run on the BUGGY harness; its central conclusions are partly
> superseded by `findings-clean-ablation.md`.** Two bugs found *after* this doc materially affected these numbers:
> (1) the board render dropped all-but-one of each tile's stacked units (B1 — a *perception* bug), and
> (2) the operators resolved the wrong stacked unit / the extractor fabricated `got` (B1/B2). On the fixed harness
> the ~0.75 ceiling **lifts to ~0.83–0.85**, so a real fraction of what this doc attributed to a fundamental
> "locating wall" was actually fixable harness error. What survives: locating IS the dominant *mechanism* on
> nearest-owned (now shown to be exclusion+argmin over-engagement, not perception), and enumeration helps
> locating — but its *accuracy* benefit was largely the render bug in disguise (see clean-ablation Finding 3).
> Read this doc as the buggy-harness snapshot; read `findings-clean-ablation.md` for the corrected picture.

**Date:** 2026-08-01 · offline failure-mode analysis of the scale test in `findings-calc.md` §Scale test.
Board: **T677 masked to player2 (`--fog 2`, 98% explored)** — a large, dense, well-explored known map
(raw board block ~148k chars in the system prompt). Arms: `raw-ops` (full board + 4 calculator operators)
vs `interactive-ops` (query loop + perception verbs + the same 4 operators). Three dispersed kinds,
8 items/kind, **3 reps (seeds 1–3), n=72/arm**, DeepSeek V4 Flash think-off. Data: `results-calc-scale-p2{,-rep2,-rep3}.jsonl`;
traces: `traces/results-calc-scale-p2*/`. This confirms/refutes the "second bottleneck = locating" hypothesis by
reading every one of the 37 failed trajectories.

## Headline

At 98% explored both ops arms tie at **~0.75**, and the tie is a **locating/enumeration wall, not an arithmetic one**.
Of **37 failures across both arms**, ~**59% are direct locating errors** (operators pointed at the wrong or an
incomplete set of tiles/cities), another ~**32% are convergence cap-outs** that are *caused by* the cost of locating
candidates through the query loop, and only ~**5% are residual computing** (off-by-one / move-rate / output mis-read).
Counting cap-outs as downstream of locating, **~92% of the ceiling is "finding the right thing," ~5% is "computing over it."**
The single region-count "miss" is a scoring artifact (the operator returned the right number; the harness extracted the
wrong token from the prompt) — region-count is genuinely at ceiling for both arms.

**Verdict: the corrected thesis is CONFIRMED.** The calculator killed the arithmetic bottleneck (small-board raw-ops
0.986, 0 wrong); at scale a *locating* bottleneck re-emerges and caps both ops arms. Richer **measurement/valuation**
operators — the maximal-calculator plan — will hit the **same** wall, because the failures are almost never arithmetic.
What would break it is **enumeration/index operators that return predicate-filtered positions**, not more scalars.

## The mechanism is baked into the two arms' substrates

| arm | perception substrate | tools | how it must LOCATE entities |
|---|---|---|---|
| **raw-ops** | full 148k-char board in system prompt | `distance`, `travel_turns`, `count_terrain`, `count_resource` (scalars only — **no operator returns positions**) | **eyeball** coordinates of its cities / resource tiles out of the giant board text, then feed them to operators |
| **interactive-ops** | none (empty board; must query) | above **+** `region_summary`, `scan`, `scan_grid`, `get_tile`, `list_cities`, `list_units` | **enumerate** via `list_cities`/`list_units`/`scan` — one round-trip per query |

Neither arm has an operator that answers "*where are the Wine tiles / my cities*" as data. raw-ops must transcribe them
by eye; interactive-ops must fetch them one round-trip at a time. That single gap produces the entire per-kind split.

## Per-arm × per-kind (pooled 3 reps, n=24/cell) — confirms the clue

| arm | kind | correct | wrong | invalid | acc |
|---|---|---|---|---|---|
| **raw-ops** | nearest-owned | **12** | 12 | 0 | 0.500 |
| raw-ops | reachable-nearest | **18** | 5 | 1 | 0.750 |
| raw-ops | region-count | 23 | 1\* | 0 | 0.958 |
| raw-ops | **TOTAL** | 53 | 18 | 1 | **0.736** |
| **interactive-ops** | nearest-owned | **18** | 4 | 2 | 0.750 |
| interactive-ops | reachable-nearest | **12** | 9 | 3 | 0.500 |
| interactive-ops | region-count | 24 | 0 | 0 | 1.000 |
| interactive-ops | **TOTAL** | 54 | 13 | 5 | **0.750** |

\* the one raw-ops region-count "wrong" is a harness output-extraction artifact (below), not a reasoning error.

The clue reproduces exactly: **nearest-owned** interactive-ops 18 > raw-ops 12; **reachable-nearest** raw-ops 18 >
interactive-ops 12; **region-count** tied at ceiling. They cancel to a tie. Avg tool-calls show where the cost goes:
region-count is a **1–3 call** trivial `count` (both at ceiling); the two dispersed kinds burn **17–21 tool-calls / 5–7
turns** on locating loops.

## Failure-mechanism breakdown (all 37 failed trajectories, hand-classified)

| mechanism | raw-ops | interactive-ops | total | share |
|---|---|---|---|---|
| **LOCATING** — wrong candidate tile/city, incomplete enumeration, or missed ownership/reachability filter | 16 | 6 | **22** | 59% |
| **RAN-OUT / bail** — capped or premature give-up (non-answer / garbage), driven by locating round-trip cost | 1 | 11 | **12** | 32% |
| **COMPUTING** — residual: off-by-one, move-rate, output mis-read | 1 | 1 | **2** | 5% |
| **output-extraction artifact** (region-count) | 1 | 0 | **1** | 3% |
| **total** | 19 | 18 | **37** | |

- **raw-ops fails by LOCATING** (16/19 ≈ 84%): it eyeballs the wrong tiles.
- **interactive-ops fails by RUNNING OUT** (11/18 ≈ 61%): it over-fetches to locate and caps out.
- **Computing is ~nil** (2/37): the calculator did its job; almost nothing here is arithmetic.
- Folding cap-outs into locating (they are the *cost* of locating through a query loop): **34/37 ≈ 92% locating, ~5% computing.**

## Why raw-ops LOSES nearest-owned (the locating tax on a huge board)

nearest-owned asks: of the resource tiles **not within 2 of one of YOUR cities**, the fewest travel-turns from your
**nearest** city, capped at 6. That is three locating steps — (a) which cities are mine, (b) which resource tiles exist,
(c) the "within 2" exclusion — before any arithmetic. raw-ops must pull all of it out of the 148k board by eye, and it
can't. Representative (`raw-ops__nearest-owned_Unassigned2_Wine_6.json`, expected 3, **got 29**):

```
[think] Unassigned2's cities:
   - Chambéry at (13,3)
   - Esch-Uelzecht at (18,3) - wait, that's Charles le Téméraire   <- can't keep its own cities straight
   ...
   >> count_resource(whole board, "Wine")  =>  28     <- returns a COUNT, no positions
   >> distance((13,3)->(41,10)) => 28  ... distance to a hand-picked FAR Wine tile
FINAL: [hand-transcribes 20+ Wine tiles by eye] ... Answer 29
```

The operators returned correct numbers; they were **pointed at the wrong tile** (a far Wine at (41,10) instead of the
true-nearest eligible one that yields 3). All 12 raw-ops nearest-owned failures are this shape: wild answers
(got 29, 30, 18, 23, 32, 74) or a **number when the answer is "none"** (failed the reachability filter — picked a tile
that is >6 turns or on another landmass). Note the cost: raw-ops nearest-owned burns **14.4k generation tokens/item**
just narrating and re-transcribing city/tile lists.

**Why interactive-ops WINS it:** `list_cities` returns **owner-tagged exact coordinates** in one call, so the hardest
locating step (ownership) is solved outright (`interactive-ops__nearest-owned_Unassigned2_Wine_6.json`, correct):

```
   >> list_cities()  =>  "Chambéry" owner Unassigned2 at (13,3) ... "Esch-Uelzecht" owner Charles le Téméraire ...
   >> travel_turns((29,11)->(28,14)) => 3   <- correct nearest eligible Wine, correct answer 3
```

Its residual failures are 3 cap-outs and 3 selection slips — half of raw-ops's 12.

## Why interactive-ops LOSES reachable-nearest (the over-fetch tax)

reachable-nearest gives the **source in the prompt** ("You control the Riflemen at (44,29)") — no ownership/locating of
the source — and asks the min travel-turns to the nearest resource tile, capped at 6. Only candidate **enumeration**
remains. raw-ops, seeing the whole board at once, just **brute-forces** `travel_turns` over ~all candidate tiles and
takes the min (`raw-ops__reachable-nearest_u2032_Wheat_6.json`, correct):

```
   >> count_resource(board,"Wheat") => 27
   >> travel_turns to ~20 Wheat tiles: 40, 22, 37, 20, unreachable, 28, 12, ...
FINAL: min = 12 > 6  =>  Answer: none   (correct)
```

interactive-ops must **scan to surface those same tiles**, paying a round-trip per query, and it over-fetches — probing
tile-by-tile with `get_tile` until the query budget is gone (`interactive-ops__reachable-nearest_u2317_Oil_6.json`,
expected 5, **got 75**, 55 tool-calls / 12 turns, capped):

```
   >> get_tile(59,1)=Ocean  >> get_tile(60,1)=Ocean  >> get_tile(61,1)=Ocean   <- 47 single-tile probes
   >> get_tile(72,0)=Glacier [Oil] ...
[budget exhausted] final answer 75   <- an x-coordinate, not a turn count: garbage on cap-out
```

8 of 12 interactive reachable-nearest failures are this cap-out/bail signature (got `none`, blank, or a coordinate
emitted as garbage). This is the exact over-fetch → non-convergence signature from `findings-fog.md`, now reappearing
**with the calculator attached**: the operators can't help a run that never reaches them.

## The asymmetry, stated plainly

- **nearest-owned rewards ENUMERATION verbs** (ownership must be located) → `list_cities` gives interactive-ops the win.
- **reachable-nearest rewards SEEING ALL CANDIDATES AT ONCE** (source is given, only enumeration remains) → the full
  board gives raw-ops the win; the query loop over-fetches and caps out.
- **region-count needs neither** (a single `count` op with a bounding box) → both arms at ceiling.

Same underlying deficit — locating entities in a large board — expressed through whichever substrate each arm lacks. The
opposite per-kind preferences are two faces of one bottleneck, which is exactly why they cancel to a tie.

## The region-count "miss" is a scoring artifact

`raw-ops__region-count_13,39:5:Grassland` (expected 19, got 5): the model called `count_terrain` over the correct box,
the operator returned **19**, and the final text says "there are **19** Grassland tiles" — but the harness extracted "5"
(the radius from the prompt "within **5** tiles"). The reasoning was correct; the answer-parser grabbed the wrong number.
Region-count is truly at ceiling for both arms and contributes nothing to the ceiling story.

## What this implies for the maximal-calculator direction

The maximal-calculator plan adds richer **measurement/valuation** operators (combat, dominance, threat, retreat — all
scalars over positions you already supply). This analysis says that plan **will hit the same ~0.75 wall on large boards**,
because:

- **~92% of the ceiling is locating**, not computing. Free measurement already solved computing (2/37 residual). Adding
  *more* measurement operators adds nothing to the 34/37 of failures that are about finding the right positions to measure.
- **raw-maxops (full board + more scalar operators) is predicted to fail like raw-ops** — it still has to eyeball the
  entities out of a huge board.
- **The lever that would move it is an ENUMERATION/INDEX operator that returns predicate-filtered *positions*** — e.g.
  `list_tiles(resource=Wine, exclude_within=2_of_my_cities)`, `nearest_owned_city(tile)`, `list_resource_positions(kind)`.
  That pushes the locating step into the tool, exactly where the deficit lives. In effect the winning combination at scale
  is **cheap complete perception AND cheap enumeration**, not more arithmetic.

**Bottom line:** on small boards the calculator removed the arithmetic bottleneck; on a large board a **locating**
bottleneck re-emerges and pins both ops arms at ~0.75. raw-ops loses the ownership-heavy kind (can't eyeball its own
cities) and wins the source-given kind (brute-forces the whole board); interactive-ops does the reverse (enumerates
ownership cheaply, over-fetches on candidate scans). Only ~5% of failures are computing. Richer measurement operators
cannot break this ceiling — richer **enumeration** operators might.

## Caveats
- Single model (DeepSeek V4 Flash, think-off), n=24/kind pooled over 3 **different-seed** reps (broader item sample, not
  repeated-item majority vote). Mechanism classification is a hand-read of all 37 failed traces; a handful of near-miss
  cases (e.g. exp 3 / got 4) could be read as selection *or* metric slips — they are counted as locating, which is the
  conservative reading for the headline (moving them to computing would still leave computing < 15%).
- Operators are oracle-exact by construction; "computing" residual here means the model composed correct measurements
  wrongly (off-by-one, move-rate), not that a measurement was wrong.
- Corpus-specific: three dispersed kinds on one large late-game known board. The locating-wall claim is about
  dispersed-integration at scale, the regime where the ceiling appeared.
