# Findings — fog of war: the realistic view inverts interactive's advantage in the early game

**Date:** 2026-07-29 · `--fog` masking (per-player known map). DeepSeek V4 Flash, think-off, fair
corpus (6 kinds), per-kind 12, raw / hierarchical / interactive. Results: `results-fog-t677-p0.jsonl`
(T677 masked to **player0, ~52% explored**, mid-game), `results-fog-t50-p1.jsonl` (T50 masked to
**player1, ~8.5% explored**, early-game). Baselines: `findings-realplay-corpus.md` (omniscient).

**Why:** every prior run fed the *omniscient* map (100% of tiles) when a real FreeCiv player has
explored 3–98% (`findings` + the fog prototype). This is the realistic view — mask to a player's
`map_t` known tiles; unexplored → `Unknown`.

> **Methodological note:** masking changes which tiles are askable, so masked and omniscient runs are
> *different question corpora*, not the same questions re-scored. The valid comparison is the **encoder
> ranking and gap within each condition**, not masked-vs-omniscient accuracy deltas tile-for-tile.

## Headline: on a realistic EARLY-game view, interactive is *dominated*

T50 masked to player1 (8.5% explored — a real early-game player; 5 kinds, settle-site yielded none as
the player sees too few enemy units):

| encoder | accuracy | mean tokens | latency |
|---|---|---|---|
| **hierarchical** | **0.850** | 7,575 | 9.0 s |
| **raw** | **0.833** | 5,205 | 7.1 s |
| interactive | **0.700 (LAST)** | 53,813 | 87.8 s |

Interactive is **strictly dominated — worse on every axis**: lower accuracy *and* ~10× the tokens
*and* ~12× the latency. On a tiny known map (~320 land tiles) raw's whole board block is **5k tokens**,
so interactive's entire pitch — *don't pay to dump the board* — buys nothing, and its round-trip
overhead makes it the worst option. Per-kind, interactive still wins the pure-aggregation kinds
(region-count 12/12, terrain 12/12) but is crushed on nearest-owned (3/12 vs raw 10) and
reachable-nearest (5/12 vs raw 12).

## Mid-game (52% explored): the edge compresses but survives

T677 masked to player0:

| encoder | masked (52%) | omniscient |
|---|---|---|
| **interactive** | **0.792** | 0.736 |
| hierarchical | 0.736 | 0.569 |
| raw | 0.667 | 0.625 |

Interactive still leads, but **hierarchical nearly catches it** (0.736). Same per-kind shape as the
omniscient fair corpus — interactive wins local aggregation (terrain 12/12, region-count 12/12,
best-site 11/12, settle-site 11/12), loses dispersed-integration + travel (nearest-owned 7/12,
reachable-nearest 4/12). (raw's terrain dipped to 3/12 — likely small-n noise plus the `Unknown`
default-terrain interaction; flagged, not over-read.)

## The gradient IS game phase

Interactive's **rank** as a function of how much the player has explored:

| known-map | interactive rank | margin |
|---|---|---|
| 8.5% (early) | **last** — dominated | −0.13 vs best |
| 52% (mid) | 1st | +0.06, static encoders closing |
| 100% (omniscient, unreal) | 1st | clearer on dense boards |

**Interactive's advantage scales with known-map size, and known-map size *is* game phase.** The
realistic (fogged) benchmark shows the value proposition is even narrower than the omniscient fair
corpus implied: querying only pays once a player has explored a large fraction of a dense map, and it
is actively *counterproductive* early — which is most of the game until mid-game. The token economics
literally invert on small known maps.

## Confirmed on a FIXED board — the crossover is the static encoders collapsing

The 2-board gradient above confounds board with phase. Re-run on **one board (T677)**, masking to three
players at different exploration levels — same map, only the known fraction varies (`results-fog-t677-p4/p0/p2.jsonl`):

| explored | interactive | raw | hier | interactive rank | int − best-static |
|---|---|---|---|---|---|
| 6.8% (early) | 0.819 | **0.931** | 0.875 | **#3 (last)** | −0.111 |
| 52% (mid) | 0.792 | 0.667 | 0.736 | **#1** | +0.056 |
| 98% (late) | 0.819 | 0.597 | 0.750 | **#1** | +0.069 |

The gradient **holds with the board fixed** — interactive is dominated at 6.8% explored (matching the T50/p1
early result, so it is *not* a small-board artifact), and leads from mid-game on. The board-vs-phase confound
is removed.

**The mechanism is sharper than "interactive scales with board size."** Interactive's accuracy is roughly
**flat (~0.82) across exploration** — it fetches the focused slice it needs regardless of how big the known
map is. It is the **static encoders that collapse as the map fills in**: raw falls **0.931 → 0.667 → 0.597**
as there is ever more to cram into one block and eyeball. So the crossover is not interactive *improving*;
it is the static encoders **anti-scaling** with known-map size and crossing interactive's flat line. That is
the real economic story: query-loops are *board-size-invariant*, static dumps *degrade with board size*.

Two riders: (a) **consistency check** — the 98% masked point ≈ the omniscient T677 result (same ranking,
interactive ahead), so masking behaves sanely at the high-exploration end. (b) Interactive's
**dispersed-integration weakness persists at every exploration level** (reachable-nearest 4/12, nearest-owned
8/12 even at 98%), i.e. it is structural, not a board-size effect.

## The complete arc (three lenses on the same question)

1. **Curated local-aggregation slate** → "interactive beats the pants off everything" (an artifact).
2. **Fair real-play corpus, omniscient** → a modest, kind-dependent edge; loses dispersed-integration.
3. **Fair corpus + realistic fog of war** → interactive *loses* the early game and its mid-game edge
   compresses as the static encoders shrink with the board.

Each step removed a way the benchmark was flattering interactive; the honest, realistic result is that
interactive is a *situational* strategy (mid/late-game, dense, local-aggregation questions), not a
general win.

## Testing the "missing directory" hypothesis: list_cities / list_units — REFUTED

The 6.8% diagnosis (all 12 dispersed-kind failures capped out at 16 turns / ~56 tool calls hunting for
scattered cities/resources) pointed at a missing **entity directory**. So we added observation-only
`list_cities` / `list_units` verbs (commit `1ec8a9f`) and re-ran the *identical* T677/p4 (6.8%) corpus,
interactive only (same items):

| kind | interactive (no verbs) | interactive (+ directory verbs) | raw |
|---|---|---|---|
| nearest-owned | 7/12 | 6/12 | 11/12 |
| reachable-nearest | 8/12 | 6/12 | 10/12 |
| settle-site | 9/12 | 9/12 | 11/12 |
| **overall** | **0.819** | **0.792** | 0.931 |

**The verbs did not close the deficit** — accuracy is flat/slightly down (within noise), still far below raw.
What they *did* change: **tool calls per question fell ~30% (mean 50.8 → 36.1)** — the model does find its
assets faster. But **turn-cap-outs barely moved (38 → 37 of 72)**, and the item-level result is **churn** —
5 previously-wrong items fixed, 8 previously-right broken — the signature of a trajectory shift dominated by
noise, not a real gain.

**Conclusion: the binding constraint is INTEGRATION / COMPUTATION, not asset-discovery.** Even handed a cheap
directory, a think-off model still can't reliably compute travel-cost / nearest over the (now easily-found)
scattered entities, so it keeps capping out. The deficit is a **reasoning limit, not an access limit** — which
predicts **think-on** (not a better verb) is what should close it (the logical next test). The verbs are
**kept** regardless: ~30% less flailing at no accuracy cost, and they are info-complete (a cheaper view of
what `scan` already exposes).

## Confirmed: think-on closes the deficit — it was a REASONING limit, not access

Re-ran the same T677/p4 (6.8%) dispersed kinds with `--think on` (raw + interactive, directory verbs on);
identical items, so directly comparable to the think-off rows above:

| kind | raw off | raw ON | interactive off | interactive ON |
|---|---|---|---|---|
| nearest-owned | 11/12 | 12/12 | 6/12 | 8/12 |
| reachable-nearest | 10/12 | 9/12 | 6/12 | **10/12** |
| settle-site | 11/12 | 9/12 | 9/12 | **10/12** |
| **dispersed total** | **0.889** | 0.833 | **0.583** | **0.778** |

**Reasoning closes it.** Interactive's dispersed accuracy jumps **0.583 → 0.778** with think-on (the directory
verbs alone did *nothing*), while raw stays flat (0.889 → 0.833, noise — it already sees everything, so there
is nothing extra to reason over). The interactive-vs-raw gap on these kinds collapses from **0.31 (think-off)
to 0.055 (think-on)**, and interactive actually *ties or beats* raw on reachable-nearest (10 vs 9) and
settle-site (10 vs 9); only nearest-owned still favors raw (12 vs 8).

This settles the mechanism: the dispersed-integration deficit was a **reasoning / integration limit**. Given
the scattered assets (found via `scan` or the directory verbs), a *reasoning* model can do the travel-cost /
nearest computation over them; a think-off model cannot. **Access (verbs) was the wrong lever; compute
(think-on) was the right one** — the two experiments together isolate the cause cleanly.

**But reasoning doesn't make interactive cost-competitive here.** Interactive+think-on averaged **~137k prompt
tokens/question** (reasoning × many tool-call round-trips) vs raw+think-on's **~5k** on this small early board
— ~25×. So on early/sparse maps raw+think-on is *both* slightly more accurate *and* vastly cheaper;
interactive's reasoning-rescued accuracy still can't overcome its structural cost disadvantage when the board
is already small. (Consistent with the whole fog story: interactive only earns its keep on large known maps.)

## Caveats
- Masked vs omniscient are different corpora (ranking-within-condition is the valid read).
- Different player per board — known fraction (8.5% / 52%) is the phase variable, but also confounds
  board (T50 vs T677). A clean **known-fraction sweep on one board** is the follow-up.
- T50/p1 masked yielded no settle-site (too few known enemy units) → 5 kinds there.
- Single model, n=12/cell (wide CIs); v1 fog over-informs on fogged-but-explored tiles (occupants).

## Next
- **DONE — known-fraction sweep** on fixed T677 (6.8 / 52 / 98%): gradient holds; the crossover is the
  static encoders collapsing while interactive stays flat (fixed-board section above).
- **Cross-model** — ✅ **early-game point DONE on Gemini 2.5 Flash** (see the cross-model section below): the
  early-game collapse REPLICATES, but the think-off comparison is confounded by an **agentic-effort asymmetry**
  (DeepSeek works the loop ~13× harder than Gemini at the same `[nothink]`). **New priority: a think-ON
  cross-model pass** to equalize in-loop effort; then finish Gemini p0/p2 for the crossover threshold.
- A **10-point curve** (all T677 players, 6.8→97.9%) plotting the int−best-static gap vs known-fraction, if a
  publishable figure is wanted.
- **Occupant-visibility** layer (a known tile currently reveals live occupants; realistically only
  currently-lit tiles do).

**Cost:** fog runs $0.91; **session total ~$2.67**.

---

## Cross-model (Gemini 2.5 Flash) — early-game collapse replicates, and a critical think-off behavioral asymmetry
**Date:** 2026-07-30. Ran the **early-game** fogged point only (T677 `--fog 4`, 6.8% explored) on
`google/gemini-2.5-flash [nothink]`, fair 6-kind corpus, per-kind 12 (n=72/encoder), to test whether the
DeepSeek-only fog story is model-specific. Stopped after p4 for budget (p4 cost ~$2.16; session ~$4.83/$10);
p0/p2 not yet run. `results-fog-gemini-t677-p4.jsonl`.

### 1. The early-game interactive collapse is NOT DeepSeek-specific
Interactive ranks **last at 6.8% explored on Gemini too**, and the deficit is *deeper*:

| encoder | DeepSeek p4 | Gemini p4 |
|---|---|---|
| best static | raw **0.931** | hier **0.889** |
| 2nd static | hier 0.875 | raw 0.806 |
| **interactive** | **0.819** | **0.694** |
| interactive − best-static | −0.111 | **−0.195** |

The per-kind failure cluster replicates (Gemini interactive: reachable-nearest 4/12, nearest-owned 7/12 — the
dispersed-integration/travel kinds), and interactive ties the field on local-aggregation (terrain 12/12).
One model nuance: the best *static* encoder flips (hierarchical wins early on Gemini; raw won on DeepSeek).
**Robust finding = the within-model RANKING (interactive last early-game), which now holds on two lineages.**

### 2. ⚠️ "Think-off" is NOT behaviorally equivalent across models — the cross-model magnitude is confounded by agentic EFFORT, not by the reasoning flag
Investigating *why* Gemini interactive (0.694) < DeepSeek interactive (0.819), the raw JSONL shows the two
models use the tool loop completely differently at the **same** `[nothink]` setting:

| interactive `[nothink]` | turns | tool-calls | gen-tok/turn | total gen-tok | acc |
|---|---|---|---|---|---|
| **DeepSeek** V4 Flash | 11.9 | 50.8 | 480 | 5697 | 0.819 |
| **Gemini** 2.5 Flash | 3.8 | 7.3 | 75 | 284 | 0.694 |

DeepSeek **works the loop** (~12 turns, ~51 fetches, planning between them); Gemini **bails** after ~4 turns /
~7 fetches. The gap in accuracy tracks the gap in effort.

**Confounds ruled out (verified from the JSONL, not the labels):**
- **Not a think-flag mismatch.** Every row is tagged `[nothink]` for both models.
- **DeepSeek is not secretly reasoning.** Its *single-shot* completions are tiny — raw **450**, hier **536**
  gen-tok — so reasoning really is off. Its large *interactive* token count (5697) is purely **many turns ×
  ~480 tok/turn of tool orchestration**, not hidden chain-of-thought. This cleanly kills the "DeepSeek
  accidentally think-on" hypothesis.
- **The mirror tell:** on *single-shot* questions Gemini is the *verbose* one (raw gen-tok **6746** vs
  DeepSeek's 450). Gemini clearly *can* generate a lot — it just **disengages in multi-turn tool use**,
  front-loading effort into one-shot answers instead.

**Implication for how we report cross-model results.** The interactive mode's payoff is contingent on the
model actually exploring the loop; a model that under-engages think-off gets less from it. So:
- The **honest cross-model claim is the within-model ranking** (interactive last early-game, replicated).
- **"DeepSeek interactive > Gemini interactive" is confounded by in-loop effort**, not intrinsic query skill,
  and the Wilson CIs overlap (Gemini int [0.580, 0.789] vs DeepSeek int ≈[0.71, 0.89]) — treat as suggestive.
- This is *consistent with* the established **reasoning/effort-bound** deficit: Gemini think-off simply
  allocates far less effort to the loop. **A think-ON cross-model pass would equalize in-loop effort and is
  the clean way to compare** — now the priority cross-model follow-up, above finishing Gemini p0/p2 think-off.

**Cost:** Gemini p4 ~$2.16 (verbose on static encoders at $2.50/M out); **session total ~$4.83/$10**.

---

## Replicated isolation sweep — the levers separated, and the early-game reversal confirmed
**Date:** 2026-07-30. After the small-map interactive improvements (scan_grid bulk fetch + fetch-whole
nudge; enriched region_summary with cities-by-owner / per-owner units / per-type resources; moving cache
breakpoint), a single-run A/B suggested the early-game deficit had closed — but that run was **noise-limited**
(a check found ~12–18% of item verdicts flip run-to-run purely from DeepSeek's temp-0 non-determinism, and its
"baseline" predated the directory verbs). So we ran a **3-arm × 3-rep isolation sweep** on the same T677/p4
(6.8% explored) corpus, DeepSeek [nothink], majority-voted per item, with paired McNemar between arms. Arms
pinned to commits: **Arm0** `f82bd5a` (baseline, has directory verbs), **Arm1** `1009cb3` (+scan_grid+nudge),
**Arm2** `0664dd7` (+enriched summary, full HEAD). Ran in an isolated git worktree. `results-sweep-*.jsonl`.

**Consensus accuracy (majority over 3 reps; n=216 pooled):**

| Arm | interactive | vs static |
|---|---|---|
| 0 baseline | **0.750** | behind (raw/hier 0.861) — reproduces the "dominated early-game" result |
| 1 +scan_grid+nudge | 0.806 | |
| 2 +enriched summary (HEAD) | **0.917** | **ahead** (raw 0.861, hier 0.861) |

**Lever isolation (paired McNemar, net items fixed on the same 72; b=A-right/B-wrong, c=A-wrong/B-right):**
- **scan_grid+nudge (Arm0→1): net +4** — but it's the **efficiency lever**: −40% tool-calls (36.2→21.8),
  turns 11.4→7.9. Rescued nearest-owned (+5) but **regressed settle-site (−3)**.
- **enriched summary (Arm1→2): net +8** — the **larger accuracy lever (~⅔ of the gain)**. Fixed
  reachable-nearest (+4), recovered the settle-site regression (+3), nearest-owned +1. Adds calls back
  (21.8→28.9) but stays below baseline.
- **combined (Arm0→2): net +12, b=0** — zero items regressed at consensus. Dispersed kinds carried it:
  nearest-owned 0.417→0.917, reachable-nearest 0.417→0.750.

**Why scan_grid+nudge HURT settle-site (mechanism, from the per-item turns/calls).** All 3 regressed
settle-site items used **fewer turns/calls under Arm1 than Arm0** (e.g. 17→12.7, 16.7→13.7, 11.3→8.7 turns) and
got them **wrong**. Settle-site = "best candidate tile for the most {terrain} in the work radius" — a meticulous
per-candidate count. The nudge ("grab the whole region in one grid") pushed the model to a **single big-grid
grab + early termination** instead of careful iterative per-candidate scanning, and it **miscounts** off the
dense grid. The enriched summary recovered these by making the per-region aggregates **exact** (so a big grab no
longer requires manual re-counting) — Arm2's calls on those items climbed back up (34–46). Lesson: a
bulk-fetch + "fetch it all at once" nudge trades careful aggregation for coverage, which backfires on
fine-counting comparison tasks; exact summaries are the fix, not more raw grid.

**The early-game reversal — real, but ACCURACY-only.** The old "interactive is strictly dominated at 6.8%
explored" holds **only for the baseline** (Arm0 0.750 < static 0.861). **At HEAD interactive is ahead on
accuracy** (consensus 0.917 vs 0.861). Honest bounds: **pooled** CIs overlap (0.875 vs 0.856/0.843) → tied-to-
ahead; the clear lead is at **consensus** over 3 reps. **Cost is unchanged** — interactive still ~76k prompt
tok/q and ~8.9 turns / 28.9 calls vs raw's ~5.4k / 1 turn (~13–14× tokens, ~9× round-trips). So the claim is
**"no longer accuracy-dominated early-game," NOT "cheaper."**

**Corrects the single-run A/B:** true baseline is **0.750** (not the lucky-high 0.819 we'd quoted), and the
gain is **larger** than the noisy run showed (+0.167 consensus; that run's single HEAD rep at 0.875 was itself
low vs the 0.917 consensus). The tool-call "−35%" credited earlier to scan_grid was mostly the *directory verbs*
(a prior change); scan_grid's own marginal call-saving is Arm0→1's −40%.

**Why interactive burns ~13× the tokens (decomposition — and it DOES implicate over-fetching).** Interactive
prompt tokens = Σ over turns of (system + question + accumulated transcript) ≈ **76k**, vs raw's 5.4k sent
**once**. **Correcting an earlier mis-statement:** the interactive **overview is small — ~1,050 real tokens,
only ~0.15× the raw board block** (measured on T677/p4: overview 2,003 chars vs raw 13,402), NOT "raw-sized."
The per-turn bulk is the **accumulating tool-result transcript**. Decomposing the 76k: the model fetches an
estimated **~13–15k tokens of board content cumulatively over ~29 calls — roughly 2.5–3× the entire known board
(5.4k in raw)**, i.e. substantial **redundant/overlapping fetching** (the over-fetch hunch, *supported*); the
**re-send protocol then re-bills that growing transcript across ~9 turns (~4–4.5× amplification)**. So
≈ (2.5–3× over-fetch) × (~4.5× re-send) ≈ the observed ~13×. **74% of the tokens are cached** (moving cache
breakpoint) → ~7–8× in **dollars**; the unclosed cost is **latency** (9 sequential round-trips), which caching
doesn't touch. Cutting it needs **less fetching AND fewer round-trips** (batch fetch / program-once DSL), not
just re-send caching. **Caveat: the ~13–15k cumulative-fetch figure is an *estimate* — the runner logs only
summed tokens, not per-turn/per-call sizes; pinning the over-fetch precisely needs trajectory logging (now being
added).**

**Cost:** sweep ~$2.03 (DeepSeek). **Bottom line:** the enriched summary is the accuracy lever (esp. on
dispersed-integration kinds), scan_grid+nudge is the efficiency lever (and a fine-counting hazard alone), and
interactive is no longer accuracy-dominated early-game — but its cost/latency disadvantage is intact.

---

## Over-fetch, CONFIRMED on real trajectories (the tracer's first payoff)
**Date:** 2026-07-30. With the trajectory tracer live, ran 32 traced interactive questions (T677/p4, 6.8%
explored, 319 known tiles; DeepSeek [nothink]) and read the actual tool-call sequences — the estimate from
token-math is now confirmed with data. `analysis/trace_overfetch.py`/`trace_seq.py`.
- **Content over-fetch ≈ 1.8× overall, 2.4× on nearest-owned** (the worst dispersed kind) — the earlier
  ~2.5–3× estimate brackets reality; dispersed kinds sit at the low end. By **tile count it's ~6× (12.6× on
  nearest-owned)**; pure box-overlap redundancy is ~1.7–2.2×. The overall content multiple is a touch *below*
  the char-based estimate because scan_grid's glyph grid makes a fetched tile cheaper than raw's per-tile cost.
- **~86% of the unique tiles interactive scans are Unknown fog** — it sweeps oversized boxes over a
  mostly-unexplored board, netting ~half the known tiles while touching ~1,155 tiles.
- **Re-send dominates:** total prompt ≈ **103k tok/question = 19.7× the raw board** here (harder-iterating than
  the ~13× run) = **~1.8× content over-fetch × ~11× transcript re-send**; 78% cached → ~4.3× in $.
- **Mechanism (no fetch-memory), three patterns:** (A) **identical re-scans** — settle-site re-issues the same
  boxes ~11× across turns; (B) **get_tile-spam of already-scanned tiles**; (C) **whole-board fog sweeps** —
  nearest-owned makes ~75 `region_summary` calls re-summarizing seen sectors; the worst scanned the *entire*
  board *twice*, then force-answered blank. Root cause: the model keeps no memory of what it fetched and no
  index of where entities are.
- **Over-fetch predicts NON-answers, not wrong answers:** invalid (force-answer cap-outs) 47.2 calls / 15.0
  turns / 80% hit the turn cap; correct 33.5 / 11.1 / 24%; **wrong** is the opposite mode — *early bail* (2
  cases, ~6 turns, confidently wrong). So over-fetch → failure-to-converge, not incorrectness.
- **Fix (on-pattern):** a server-side **dedup / "already fetched X" guard** (collapse identical/subsumed scans,
  remember get_tiles) kills Patterns A/B cheaply; a **coverage hint** (tiles-still-Unknown + entity locations)
  kills Pattern C. A scanned-tile *budget* only as a safety net (budget pressure hurt settle-site fine-counting,
  above). The 19.7× re-send itself only yields to a **program-once/DSL** structural change. **Priority caveat:**
  whether to build the dedup guard is coupled to the calculator experiment — if the dispersed deficit is
  *arithmetic* (calculator fixes it), the project pivots to `raw-ops`/decision-corpus (which barely fetches) and
  interactive over-fetch becomes a side issue. **Cost:** ~$0.28.
