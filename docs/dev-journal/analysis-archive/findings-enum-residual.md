# Findings — enumeration moved the residual OFF locating (into composition + tool-trust)

> **⚠️ REVISED 2026-08-01 — run on the BUGGY harness; the enumeration *accuracy* lift here is largely a render-bug
> artifact.** This doc analyzed `raw-maxops-enum` at 0.847 and credited enumeration with a +0.055 accuracy lift
> over its calculator control. On the FIXED harness (`findings-clean-ablation.md`) that lift collapses to +0.012
> (gone): once the render shows *all* stacked units (B1), the calculator arm can locate entities directly and
> enumeration's edge evaporates — the old lift was enumeration compensating for a render that showed only 1 of N
> units. **What SURVIVES and is confirmed clean:** (a) enumeration's ~2× efficiency win (fewer turns/tokens at
> equal accuracy); (b) the diagnosis that nearest-owned's residual is *not* perception but exclusion+argmin logic
> — and the clean run sharpens it: enumeration makes nearest-owned *worse* (over-engagement → ~178 hand `distance`
> calls → cap-out), which validates this doc's call for a positional-FILTER operator. Read the mechanism here;
> read `findings-clean-ablation.md` for the corrected accuracy accounting.

**Date:** 2026-08-01 · offline failure-mode analysis of the `raw-maxops-enum` arm. Board: T677 masked to
player2 (`#fog(p2)`, 98% explored). 3 dispersed kinds, 8/kind, 3 different-seed reps, n=72/arm, DeepSeek V4
Flash think-off. Companion to `findings-scale-ceiling.md` (the raw-ops 92%-locating baseline). Data in the
`maxops-enum` worktree: `results-ablation-enum-p2-seed{1,2,3}.jsonl`, `traces/results-ablation-enum-p2-seed*/`.

## Headline

`raw-maxops-enum` = **0.847**, the top arm (raw-ops 0.750, raw-maxops 0.792). The **11 residual failures**
(10 wrong + 1 invalid) were hand-read trace-by-trace. **Enumeration did what it was designed to do: it
demolished the locating wall.** Locating errors fell from **~84% (raw-ops) to ~9% (1/11)**. The enumerators
(`list_tiles`, `list_owned_cities`, `list_owned_units`) were **adopted on 11/11 failures and returned the
correct, complete positional set every time** (Gems→3 tiles, Wheat→27, Pheasant→34, Fish→64, Silk→25; cities
20/35/3). This is **not** an adoption problem and **not** a locating problem anymore. The residual is
**downstream of locating**: composing the answer from correct positions (the "within-2" exclusion done by
hand, move-rate conversion, wrong metric) plus a chunk of harness/oracle artifact in the `reach_turns` operator.

**The bottleneck MOVED.** raw-ops was ~92% locating / ~5% computing. `raw-maxops-enum` is ~**9% locating /
~73% computing-and-composition / ~18% harness-artifact**.

## Per-kind (raw-maxops-enum, pooled 3 seeds, n=24/kind)

| kind | correct | wrong | invalid | acc |
|---|---|---|---|---|
| nearest-owned | 19 | 4 | 1 | 0.792 |
| reachable-nearest | 18 | 6 | 0 | 0.750 |
| region-count | 24 | 0 | 0 | 1.000 |
| **TOTAL** | 61 | 10 | 1 | **0.847** |

region-count stays at ceiling. All 11 misses are the two dispersed kinds. (The 1 invalid is nearest-owned
Pheasant, seed1.)

## Adoption: 11/11 called the enumerators, and they worked

Every failing trajectory called `list_tiles` (all 11) plus `list_owned_cities`/`list_owned_units` where the
kind needs ownership. **In no failure did the model fall back to eyeballing entities out of the board** — the
raw-ops signature. Enumerator output was correct and complete in every case (verified against the solver's
tile/city sets). The model located the right things, then mishandled them — the clean inverse of raw-ops,
which eyeballed the *wrong* things.

## Failure-mechanism breakdown (all 11 failed trajectories)

| mechanism | count | items |
|---|---|---|
| **cap-out / bail** (non-answer or coordinate-garbage; driven by hand-doing the exclusion or move-rate dither, NOT locating) | **5** | nearest: Gems-U5, Pheasant-U5(invalid), Fish; reachable: Wine, Pheasant-u1131* |
| **COMPUTING** (right positions, wrong composition: move-rate ×3, wrong-metric ×1) | **4** | reachable: Gems-u3094, Gold, Oasis*; nearest: Silk |
| **LOCATING** (incomplete candidate search despite enumeration) | **1** | nearest: Wheat |
| **oracle / operator-parity artifact** | **1** | reachable: Game-u3085 |
| **enumeration-misuse** (wrong box/predicate to list_tiles) | **0** | — |
| **output-extraction artifact** | **0** | — |
| **total** | **11** | |

\* Pheasant-u1131 and Oasis are *also* hit by the `reach_turns` "no modeled unit" bug — part of their cause is
harness artifact, not model reasoning.

### Side-by-side with the raw-ops baseline

| mechanism (% of that arm's failures) | raw-ops (n=19) | **raw-maxops-enum (n=11)** |
|---|---|---|
| LOCATING | **~84%** (16/19) | **~9%** (1/11) |
| cap-out / bail | ~5% | 45% (5/11) |
| COMPUTING | ~5% | **36%** (4/11) |
| oracle / harness artifact | ~5% | ~9% (1/11) |

**Crucial re-fold.** The prior doc folded cap-outs *into locating* (they were the cost of locating through a
query loop). **That folding does not apply here.** Enumeration solved locating in 1–2 calls; the enum cap-outs
come from the model doing the *arithmetic/exclusion* by brute force (60–68 manual `distance()` calls to test
"within 2 of a city", or repeated move-rate dithering), not from finding entities. Folding cap-outs into their
true driver:

| folded bucket | raw-ops | **raw-maxops-enum** |
|---|---|---|
| LOCATING (incl. its cap-out cost) | **~92%** | **~9%** (1/11) |
| COMPUTING + composition (incl. cap-outs from hand-arithmetic/move-rate) | ~5% | **~73%** (8/11) |
| harness / oracle artifact | ~3% | **~18%** (2/11) |

The residual has **inverted**. Enumeration hit exactly the bottleneck it targeted.

## The `reach_turns` operator bug (2/11, distorts more)

`reach_turns(ux,uy,tx,ty)` is the correct oracle for reachable-nearest (uses the unit's own move rate via
`unit_at(board,ux,uy)` over `board.units`). On the p2-masked board it fails for some of the very units the
question names:

- **Oasis-u3956** (unit at (48,17); board text shows `{unit Alpine Troops owner Unassigned4}`): all 9
  `reach_turns` calls returned `error: no modeled unit at (48, 17)`. The unit is *rendered in the board text*
  but *absent from `board.units`* on this masked board → operator unavailable. Model fell back to `travel_turns`
  (move-rate-1)=3 and answered **3**; truth is **1**.
- **Pheasant-u1131** (Engineers at (29,11)): same — 4× `error: no modeled unit`, then dithered and capped
  (got=29, a coordinate).
- **Game-u3085** (Mech.Inf. at (65,8)): `reach_turns` *worked* but returned min **3** — model answered 3 —
  while ground truth is **5**. **All three arms miss this item**, and both tool arms emit exactly the operator's
  3. This is an operator-vs-solver **parity gap** (the masked run board differs from the generation board the
  expected answer was solved on), analogous to the region-count extraction artifact in the prior doc. Not a
  model reasoning failure.

So ~2–3 of the 11 "failures" are harness issues; the *true* model residual is closer to 8 items.

Note a striking pattern from the cross-arm table: enum uniquely fails several items the **simpler arms got
right** — Gems-u3094 (raw-ops & raw-maxops both `none`✓, enum 3✗), Gold (both 2✓, enum 1✗), Silk (both 3✓,
enum 5✗), Fish (both `none`✓, enum capped✗). The enumeration arm sometimes does *worse* by over-engaging tools
(move-rate fabrication, capping on tool loops) on items the eyeball arms nailed.

## Three annotated excerpts

**(1) COMPUTING — model distrusts a correct tool and fabricates move-rate math.** `reachable-nearest:u3094:Gems:6`,
expected **none**, got **3**. raw-ops and raw-maxops both correctly answered `none`; only enum failed.
```
list_tiles(resource=Gems) -> (75,22),(75,25),(28,26)          [enumeration: correct]
reach_turns(55,18->75,22)=>8  reach_turns(->75,25)=>8  reach_turns(->28,26)=>9
   [oracle: all > 6 turns => none reachable within horizon => "none"]
MODEL: "travel_turns for a 1-tile/turn unit are given; we need ceil(turns/move_rate).
        (75,22): travel_turns = 8, ceil(8/3) = 3 ... All three are 3 turns. Answer: 3"
```
The model *invented* `travel_turns=8` (it never called travel_turns), divided by an assumed move rate, and
overrode the correct `reach_turns=8` it already held. Gold-u3593 is identical (reach_turns min=2 = truth;
model applied `ceil(2/3)=1`).

**(2) cap-out — enumeration nailed locating; model burned the budget on the exclusion by hand.**
`nearest-owned:Unassigned5:Gems:6`, expected **6**, got **2** (68 tool calls, truncated).
```
list_tiles(resource=Gems)->3 tiles  list_owned_cities(Unassigned5)->20 cities  [both correct]
  then: distance(gems_tile -> city) x ~60, re-issued across turns 1-3 to test "within 2"
  ... never reaches the final travel_turns min; output truncated => got "2" (stray token)
```
Locating was instant and correct. The model computed the Chebyshev-2 exclusion **one `distance()` per
(tile,city) pair**, re-did it three times, and ran out.

**(3) COMPUTING — wrong metric substituted for the tool.** `nearest-owned:Unassigned4:Silk:6`, expected **3**,
got **5**. raw-ops and raw-maxops both correct.
```
list_tiles(resource=Silk)->25 tiles  list_owned_cities->35 cities  [correct]
  MODEL: "Silk at (70,5) ... my nearest city Balkh (75,3) distance 5 ..."   [distance = Chebyshev]
  30x distance() calls, ZERO travel_turns calls => reports a Chebyshev distance as the turn count
```
The model reported **Chebyshev `distance`** as if it were land-travel turns, never converting via
`travel_turns`. Right positions, wrong operator for the metric.

## Verdict

The ~15% gap-to-ceiling is **not adoption** (11/11 used the enumerators) and **not still-locating** (locating
fell from ~84% to ~9%; enumeration returned complete correct positions every time). It decomposes as:

- **~40% genuine COMPUTING floor** — move-rate conversion and metric-selection errors, several of them the
  model *distrusting a correct `reach_turns`/`travel_turns` result and re-deriving it wrong*. A DeepSeek
  disposition (recompute over trust-the-tool; matches the `deepseek-gemini-tool-engagement` finding), not a
  tooling gap. More enumeration or measurement cannot fix it; only a decision-level operator returning the
  final answer would, which trivializes the task.
- **~35% cap-out from hand-doing the "within-2" exclusion** — the one place a **second enumeration operator
  genuinely helps**: a *positional-filter predicate* on `list_tiles` (`exclude_within=K_of_player_cities`, or a
  `nearest_owned_city(tile)` index) that `list_tiles` cannot currently express. This is the "positional filter"
  the prior doc anticipated.
- **~18% harness/oracle artifact** — the `reach_turns` "no modeled unit" and parity bugs. **Fix this first**;
  cheap, removes 2 items outright, and eliminates the move-rate fallback trap behind several computing errors.
- **~9% residual LOCATING** — one incomplete-search near-miss (Wheat: had all 27 tiles from `list_tiles` but
  measured only a hand-picked subset and missed the 3-turn tile).

**Recommendation.** (a) Fix the `reach_turns` operator/board-parity bug before drawing any floor conclusion.
(b) Add **one** more enumeration op — a *positional-exclusion predicate* on `list_tiles` — targeting the
nearest-owned cap-outs; do **not** add more entity lists or more scalar measurement (locating is solved,
arithmetic is solved). (c) Accept that the move-rate / tool-distrust computing errors are a real model floor
for this model — they are what actually caps `raw-maxops-enum`, and no perception/enumeration operator
addresses them. Worth a cheap think-on or "trust-the-tool: `reach_turns` already accounts for move rate; do
not divide" preamble A/B before committing new operators.

## Caveats

- **n=11 failures, single model**, 3 *different-seed* reps (broad item sample, not repeated-item majority
  vote). Percentages are directional.
- **2–3 of the 11 are harness artifacts** (`reach_turns` bug, Game parity) — on a base of 11 this materially
  inflates the computing/artifact share; the true *model* residual is ~8 items, dominated by move-rate
  composition.
- The "distrust the tool and re-derive move rate" pathology is model-specific.
- The Wheat near-miss (got 4 / exp 3) is read as LOCATING (incomplete search) but could be a travel
  composition slip — a single item either way.

## Pointers
Results `maxops-enum` worktree `results-ablation-enum-p2-seed{1,2,3}.jsonl`; traces
`traces/results-ablation-enum-p2-seed*/`; operator semantics `civ-eval/src/encoders.rs` (`op_reach_turns`
~L1788, `list_tiles`/`list_owned_*` ~L2034–2186, `raw-maxops-enum` preamble ~L2516); solvers
`civ-eval/src/question.rs` (nearest-owned ~L621, reachable-nearest ~L742). The `reach_turns` "no modeled unit"
bug is at `unit_at` (encoders.rs ~L1729) reading `board.units`, which is empty for units that render in the
masked board text but aren't in the operator's unit vector.
