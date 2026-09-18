# Findings — eval-validity bug sweep (2026-08-01)

Read-only sweep of `civ-core/src`, `civ-eval/src`, `civ-cli/src`, `analysis/summarize.py`, and the unmerged
`maxops-enum` operator code, plus a trace sub-agent that read 587 recorded LLM traces. Focus: bugs that
silently **corrupt scored eval results**, not style/robustness. Severity: **[SCORE]** flips a scored item ·
**[REPRO]** breaks determinism/fairness · **[DATA]** poisons a run's numbers · **[TRACE]** side-channel only.

Companion: `findings-fog-bug-rootcause.md` (the stacking root-cause, agent A). Impact claims below are
**verified against the data** where noted (the parent reconciled the two agents and corrected two overstatements).

## Verified corrections to the raw agent output
- **C1 impact was overstated.** The 113 empty completions the trace agent saw live in the *excluded* frontier
  **trace directories** (smoke/retry/partial runs), not the canonical scored file. Verified: the canonical
  `results-frontier-t677-p4.jsonl` = n=48, 37 correct (**0.771**), **0** empty completions → the frontier
  anchor finding STANDS. A repo-wide audit of every scored `results-*.jsonl` found only **4 isolated
  zero-token rows** (1 each in `results-fog-gemini-t677-p4`, `results-fog-t677-p4-thinkon`,
  `results-gemini25-on`, `results-sweep-full-rep3`) — immaterial. C1 is a real *latent* bug, near-zero actual
  footprint.
- **The u3085 "different board states" hypothesis (agent B, C4-pt3) is superseded.** Agent A reproduced it as
  the *stacking* bug: at (65,8) the operator's first-match resolved to a move-5 AEGIS Cruiser (→3 turns) while
  the subject is a move-3 Mech. Inf. (→5 turns) — same board, different unit. Both agents independently confirm
  the merged pipeline hands ONE masked board to generation + render + operator (agent B lists "fog consistency
  in the merged path" as correct-by-design), so no board-state mismatch is needed. It's stacking, not a board
  mismatch.

## CONFIRMED bugs

### B1 — Unit stacking: coordinate→unit resolved 3 different ways [SCORE, high on dense boards]
The headline bug (full root-cause in `findings-fog-bug-rootcause.md`). On dense/late boards (T677: up to **37
units on one tile**), the same `(x,y)` resolves differently for the three consumers:
- **solver / scored answer** → by unit **id** (correct)
- **static render** (`encoders.rs:341-346`, `Occupants` HashMap) → **last-wins**, and drops the other 36 units
- **operators** `reach_turns`/`att_eff`/`def_eff`/`garrison_defense` (`unit_at`, `encoders.rs:~1729`) → **first-wins** `.find()`

So the model is asked about unit A, shown unit B, and the operator computes on unit C. **Ground truth is NOT
corrupted** (solvers resolve by id; field/area solvers iterate the whole `units` vec). But: every unit-by-coord
operator is wrong on stacked tiles; the render is lossy for all arms (a **perception confound on dense boards**);
`reachable-nearest` on stacked tiles is ill-posed for the model. The reconstruction gate also diverges — its
`RecoveredTile::expected` uses first-match while the renderer keeps last (`encoders.rs:171-188` vs 341-346).
**Why every gate missed it:** the operator-parity test runs on a one-unit-per-tile synthetic strip
(`combat_strip()`), where first-wins/last-wins/by-id can never differ. Blind spot = single-unit tiles vs. real
stacks. **This confounds the T677/dense results (scale ceiling, the enum ablation); sparse boards (6.8% explored)
have little stacking and are likely fine.**

### B2 — Answer extraction fabricates `got` when the model omits `Answer:` [SCORE, medium, broad]
`scoring.rs:45,55,64,90` — `first_int(&field).or_else(|| first_int(reply))`: takes the **first** integer in the
answer field, and on a missing `Answer:` line falls back to scanning the **whole reply**. Live confirmed
false-negative (`raw-ops__region-count_13_39_5_Grassland`): model wrote "there are **19 Grassland tiles**" (bold,
no `Answer:`) = correct 19; extractor grabbed **5** (the Chebyshev radius from the prompt) → scored **wrong**.
Also ~**28 of 94** cap-out `wrong` items carry a fabricated `got` that is a coordinate fragment from trailing
text (e.g. `got=29` = the subject's own x) and should be `invalid`/no-answer. **Corrupts both accuracy and the
wrong-vs-invalid split** — which several narratives lean on (the calculator "0 wrong answers" claim, the
over-fetch "non-answers not wrong answers" claim). 438 well-formed `Answer:` items extract flawlessly. Fix: take
the last integer on the answer line (or require the field to be ~just the number) and drop the whole-reply
numeric fallback.

### B3 — `most_common_terrain` tie-break is non-deterministic [REPRO, medium]
`encoders.rs:385-395` — `counts.into_iter().max_by_key(|(_,n)| *n)` over a std `HashMap`, with no name tiebreak
(every other histogram in the repo adds `.then_with(|| a.0.cmp(b.0))`). HashMap order is per-map random, so on a
terrain-count tie the omitted "default" terrain is non-deterministic across runs *and* across calls within a run.
Effects: (a) the default-terrain exclusion in Count/BestSite/SettleSite/ConstraintSite flips → the **same
(board,seed) can generate a different question set** (breaks the determinism guarantee); (b) generator and encoder
call it independently, so a `region-count` about terrain T can be rendered by an encoder that treats T as its
default → the exact count-by-exclusion unfairness the generator tries to prevent. Not directly answer-flipping
(solve counts exact equality; header re-states the default) but breaks reproducibility + cross-encoding fairness,
and it bites the tie-prone `--crop`/`--fog` boards used by the scale/ablation runs. Fix: deterministic name
tiebreak or `BTreeMap`.

### B4 — Empty/0-token completions scored as trials [DATA, latent — near-zero actual impact]
Provider-returned-nothing completions (0 prompt+completion tokens) are scored `invalid` and folded into the
denominator with no retry and no "failed-call vs answered-wrong" distinction (`runner.rs` score path;
`remote.rs` not hardened for empty bodies). **Verified footprint: 4 isolated rows across all scored files**
(above) — immaterial to every finding, incl. the frontier anchor. Still worth a cheap guard (reject/retry
0-token completions; don't score them) so a future run can't be silently deflated.

## SUSPECTED / minor
- **B5 [SCORE, low-med]** — turn-cap cap-outs aren't a distinct status; they land in `wrong` (via B2) or
  `invalid`, contaminating both buckets. Caps are tight (raw-ops/maxops = 5 turns) for
  nearest-owned/reachable-nearest. `runner.rs:434-449`. Overlaps B2.
- **B6 [TRACE, low]** — trace filenames map both `:` and `,` → `_` (`trace.rs:213-217`), so ids differing only
  in `:`/`,` placement overwrite each other (silent trace loss). Scored JSONL unaffected; only trace-analysis.
  Docstring `trace.rs:12-14` wrongly asserts non-collision.

## Checked and CORRECT-by-design (not bugs)
- The 4 **basic** operator↔solver parities (`op_distance=chebyshev`; `op_count_terrain`==region-count over the
  clipped square; `op_travel_turns` shares the `ReachField` BFS + `ceil(step/mv)`); box ops clip OOB while
  distance/travel require in-bounds (correct asymmetry). Tests `encoders.rs:1883-1966`.
- **Fog consistency in the merged path** — `mask_to_known`, `is_land`/`is_water` (Unknown=neither), `rand_tile`
  (skips Unknown, RNG-neutral); generation + render + operator share one masked board.
- **Scorer classification** — ChoiceSet acceptable/dominated/off-option, OptionalInt none-before-int, Compare3
  incomparable-first, `match_option` longest-first.
- **`summarize.py`** — plain group-by on `status=="correct"`, correct Wilson CI, no pivot bug → our aggregations
  are sound.
- **Determinism otherwise** — SplitMix64 pure, index-keyed substreams, candidate order frozen into items; B3 is
  the sole non-determinism.

## Priority
1. **B1 (stacking)** — confounds the whole dense-board (T677) line incl. the maxops ablation and the
   scale-ceiling interpretation. Biggest design fix (coordinate→unit contract; see root-cause doc).
2. **B2 (extraction)** — cheap offline fix; restores accuracy + the wrong/invalid split that narratives use.
3. **B3 (determinism)** — cheap offline fix; restores reproducibility/fairness on crop/fog boards.
4. **B4/B5/B6** — cheap hardening (reject 0-token, add a "capped" status, fix trace names).

Ground-truth integrity holds and aggregation is sound, so nothing is thrown out wholesale — but **dense-board
interpretations and any wrong-vs-invalid claim need re-derivation after B1+B2**, and affected T677 runs need a
clean rerun.
