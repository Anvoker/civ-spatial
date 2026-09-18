# constraint-site redesign — design/ideation

Status: DESIGN ONLY. No code changed, no eval run. Grounded in the current
generator/solver/scorer/encoder as of this branch.

## 1. The failure mode (one line + evidence)

`constraint-site` degenerates into **call-and-report**: the model calls one
predicate-lookup per handed candidate, ANDs the booleans, and answers — no
perception, no geometry, no candidate-to-candidate comparison.

Trace evidence (last smoke run): **8/8 trajectories were pure call+report**;
7/8 ran the full 3×4 = 12-call brute grid over the three conjuncts × four
handed candidates. The M2 `site_check` merge makes it *worse*: post-merge a
candidate is settled by exactly one call, so the whole item is ~4 calls total
(one `site_check` per candidate) with the "hard" step — the AND — pre-bundled
into the tool's `ok=` field. The kind now tests "can the model call a tool in a
loop," not spatial reasoning.

### Why the current construction can't escape this

Everything is delivered as a **closed 4-item shortlist**, and every discriminating
predicate the answer depends on is available as a **per-tile tool lookup**:

- Generator (`generators.rs:1901-2069`): `CONSTRAINT_K = 4`. Per player it
  partitions land tiles into `all3` (meets all three predicates) and `near`
  (meets exactly two), then builds either a SOME-case (1 `all3` survivor + 3
  `near` distractors) or a NONE-case (4 `near`). Because every distractor is a
  *near-miss that fails exactly one predicate*, the model never needs geometry to
  rule it out — one `site_check` returns the failing conjunct directly.
- Solver (`question.rs:1374-1410`): `acceptable` = candidates passing
  `rules::constraint_site_ok`, else `["none"]`; `options` = the 4 candidate
  coords + `"none"`.
- Scorer (`scoring.rs:103-111`, `ChoiceSet`): answer in `options`∧`acceptable` →
  correct; in `options` but not `acceptable` → wrong; not in `options` → invalid.
- Tool (`encoders.rs:2271-2281, 2509-2529`): `site_check(x,y,player)` returns
  `defensible / within_water / not_enemy / ok`, i.e. **the entire scored
  conjunction for one tile in one call.**

So the shortcut is optimal *by construction*: the search space is 4, and the
answer is a function the tool computes for you. Any redesign has to break at
least one of those two facts — **the space must be bigger than a shortlist, or
the answer must depend on something no single tool call returns** (ideally both).

### Ground truth we must preserve (the Oracle-100% invariant)

Every conjunct is a pure board function — `is_defensible` (`rules.rs:612`),
`is_within_water_radius` (Chebyshev-2 open water, `rules.rs:618`),
`not_enemy_territory` (`rules.rs:626`), composed by `constraint_site_ok`
(`rules.rs:638`). An offline Oracle enumerates tiles and computes these directly,
so it solves any of the variants below at 100% and `verify-oracle` stays green,
**provided each variant's answer set is a deterministic function of the board.**
All three variants keep that.

## 2. Reusable machinery (prefer these — don't invent)

- **`Answer::ChoiceSet { acceptable, options }`** already supports a *large closed
  option set with a many-membered acceptable set plus a "none" escape*. A region
  search is just a ChoiceSet whose `options` enumerate every candidate tile in the
  region instead of four handed tiles. No new Answer variant, no scorer change.
- **`BestSiteKind`** (`generators.rs:631-699`) is the template for region-scaled
  siting: it draws candidates over a radius derived from `Difficulty::count_radius_*`
  and evaluates each candidate's neighborhood. Its radius/window scaffolding and
  the `collect` / `item` / `log_drops` harness transfer directly.
- **`geometry::chebyshev` and `geometry::tiles_within`** (`civ-core/src/geometry.rs:101,132`)
  give the distance/coverage primitives a non-bundled spacing constraint needs.
- The generator already **classifies every land tile by predicate count** — the
  `all3` / `near` partition — which is exactly the precompute a region variant
  needs to name the acceptable set and guarantee decisiveness.

## 3. Variant designs

### Variant B — Non-bundled 4th constraint (spacing/adjacency). *Cheapest.*

**Idea (lever 3).** Add one hard requirement that `site_check` does **not** and
should not bundle, so a single merged call can no longer settle a candidate. The
natural choice is a **geometry** constraint the model must compute from the board:

- *Min-spacing:* the site must be **≥ D tiles (Chebyshev) from every existing
  city** (a real Freeciv min-city-distance rule), or
- *Empire adjacency:* the site must be **within R tiles of one of `player`'s own
  cities** (a forward-but-connected fort).

Either is a pure function of city positions (`geometry::chebyshev` over
`board.cities`), so it's Oracle-trivial and deterministic.

**Given vs must-find.** Same 4-candidate (or over-provisioned 8-12, lever 2)
shortlist. The model is *given* the tiles; it must *find*, for each, the nearest
relevant city and the distance — the tool won't.

**Phrasing.** Extend CONSTRAINT-SITE RULES (`rules.rs:1904`) with the 4th bullet,
e.g. "SPACED: at least D tiles (Chebyshev) from every existing city." Question
text (`question.rs:711`) gains "…defensible AND within 2 of water AND not in
enemy territory AND at least D tiles from any city."

**Ground truth / scoring.** Unchanged shape. Solver ANDs a 4th predicate into
`constraint_site_ok` (or a spacing-aware wrapper): `acceptable` = candidates
passing all four, else `["none"]`; `options` = candidates + `"none"`.
ChoiceSet scorer unchanged. Generator's tile partition becomes met∈{4} = survivor,
met==3 = near-miss (fails exactly one, so still one-predicate-decisive but the
failing predicate may be the *un-tooled* spacing one).

**Why call+report loses.** `site_check` returns `ok=yes` for a tile that is
actually **disqualified by spacing**. A model that trusts `ok` is now
*systematically wrong* on exactly the tiles where spacing bites — it must read
city coordinates and compute Chebyshev distance itself. The tool no longer
computes the answer; it computes 3/4 of it, and the missing quarter is geometry.
This is the same "tool tells you less than the truth" seam the fog kinds exploit.

**Cost.** ~1 predicate + 1 prompt bullet + generator partition tweak + solver
AND. **Lowest of the three.** Reuses ChoiceSet, harness, tool set as-is.

**Caveat / must-validate-live.** A sufficiently careful model will just do the
distance arithmetic and recover 100%; that's fine (it's now *doing* geometry).
The open risk is the reverse — that models *keep* trusting `ok` and the kind
becomes too hard/low-scoring for weak models. Only a live run shows where the
band lands. Also decide whether `site_check` should even keep its `ok` field, or
drop `ok` so no tool reports a spurious verdict (recommended: drop `ok`, keep the
three separable conjuncts).

### Variant A — Region search, no shortlist. *Best at killing the shortlist shape.*

**Idea (levers 1 + 4).** Stop handing candidates. Give a **rectangular region**
and ask the model to **name any satisfying tile inside it, or prove none exists.**

**Given vs must-find.** Given: a region (bounding box), reusing `BestSiteKind`'s
`count_radius`-scaled window sizing so difficulty is tunable. Must find: a tile
in the region meeting the conjunction — or certify none by covering the region.

**Phrasing.** "…which tile *in the region from (x0,y0) to (x1,y1)* is a valid
fortified site (defensible AND within 2 of water AND not enemy territory), or
`none` if the region contains no such tile?"

**Ground truth / scoring — reuses ChoiceSet directly.**
`options` = **every land tile in the region** (as `(x, y)` strings) + `"none"`;
`acceptable` = the region's satisfying tiles, or `["none"]` if empty. The scorer
is unchanged: any satisfying in-region tile → correct; an in-region tile that
fails → wrong; a coord outside the region (or water) → invalid; `"none"` → correct
iff the region is truly empty. Fully deterministic, Oracle enumerates the box.
Generator ensures decisiveness the same way it does today (require at least one
failing tile in the box; alternate some/none by choosing boxes with/without a
survivor using the existing `all3`/`near` classification).

**Why call+report loses.**
- To answer **"none"** the model must establish that *no* tile in the box
  satisfies — a **coverage proof over a space**, not four failed lookups. It can't
  enumerate "the four" because there are none; it must perceive the region.
- To answer **a tile** it must *locate* one among many, i.e. read terrain and
  water geometry off the board rather than being told which four to test.
- Under the **static / no-tool encodings there is no `site_check` at all**, so
  the answer is pure perception+geometry by definition.

**Residual shortcut + mitigation (flag).** Under *interactive* encodings a model
could still brute-force `site_check` over every tile in the box. A region of,
say, 5×5=25 land tiles makes that 25 calls — expensive but not impossible.
Region size only *raises the price* of the shortcut; it doesn't kill it. Two
mitigations: (a) size the region so exhaustive probing blows a reasonable call
budget, and/or (b) **combine with Variant B** (a non-bundled constraint) so even
exhaustive `site_check` probing returns an answer that is *wrong on the un-tooled
constraint*. Whether models actually stop brute-forcing at a given region size is
**not inferable — needs a fresh live run** to confirm the traces change shape.

**Cost.** Medium. New `Question` variant carrying a region (or reuse a
center+radius like `BestSiteChoice`), encoder rendering of the box, generator
rewrite to pick boxes and enumerate options. No Answer-enum or scorer change.

### Variant C — Region search + non-bundled constraint (A ∘ B). *Strongest, dearest.*

**Idea.** Variant A's region (kills the shortlist / forces coverage) with
Variant B's 4th geometry constraint (kills exhaustive `site_check` probing). The
model must search a space *and* the per-tile tool verdict is deliberately
incomplete, so neither "test the four" nor "probe them all" wins.

**Given/find, phrasing, scoring.** As Variant A, with the 4th predicate ANDed
into acceptability and added to the rules text. ChoiceSet, deterministic, Oracle
enumerates. Decisiveness: box must contain ≥1 failing tile; some/none controlled
by whether any box tile passes all four.

**Why call+report loses.** Both seams at once: no shortlist to iterate, and the
one tool that could shortcut a tile now under-reports the truth. A model must
perceive the region, compute water/terrain geometry, *and* compute city-distance
geometry. This is the only variant with no residual mechanical path.

**Cost.** Highest — Variant A's structural work plus Variant B's predicate. But B
is additive on top of A, so C is "A, then one more bullet," not a third design.

## 4. Recommendation (ranked by defeats-the-shortcut × implementation-cost)

| Rank | Variant | Anti-shortcut strength | Cost | Notes |
|------|---------|------------------------|------|-------|
| 1 | **B — non-bundled spacing constraint** | High for tool encodings (tool's `ok` becomes a *trap*, not the answer) | **Low** | Best value. Reuses ChoiceSet, harness, tool set. Recommend dropping `site_check.ok` alongside. |
| 2 | **A — region search** | High for the *shape* (no shortlist; forces coverage/perception) | Medium | Residual brute-probe risk under interactive encodings; needs sizing + live check. |
| 3 | **C — A + B** | Highest (no residual mechanical path) | High | Do this only if B alone under-moves the traces; it's A's cost plus B's bullet. |

**Recommended path:** ship **B first** (cheap, directly neutralizes the M2
`site_check` merge that caused the regression), read the traces, and if the item
still reads as "loop a tool" then **promote to C** by dropping the shortlist for a
region. Treat **A-without-B** as the weaker middle option — it fixes the *shape*
but leaves the interactive brute-probe escape open.

## 5. Open questions for the owner

1. **Which 4th constraint for B** — min-city-spacing (Freeciv-authentic, tests
   "distance from *every* city") or empire-adjacency (tests "distance to *your*
   city")? Spacing is the more natural hard rule and creates cleaner near-misses.
   Pick D/R relative to `WORK_RADIUS`/`count_radius`.
2. **Keep or drop `site_check.ok`?** Recommend dropping `ok` (keep the three
   separable conjuncts) so no tool ever emits a full verdict — consistent with
   the "tool reports less than the truth" design of the fog kinds. Confirm this
   doesn't break other consumers of `site_check`.
3. **Region encoding for A/C** — reuse `BestSiteChoice`'s center+radius window, or
   introduce an explicit `(x0,y0)-(x1,y1)` box? The former reuses more code and
   the `count_radius` difficulty knob.
4. **Region option-set size** — enumerate *land* tiles only (water can't be
   settled) as `options`; confirm ChoiceSet is comfortable with ~20-40 options
   (it is set-membership, so yes) and that generation-time decisiveness (≥1
   failing tile) is enforced.

## 6. Must-validate-live (do not infer)

- Whether **B** actually shifts traces away from "trust `ok`" toward computing
  distance — and whether that over-hardens the kind for weak models (score band).
- For **A/C**, the region size at which models **stop** exhaustively probing
  `site_check` and start perceiving — pure trace-shape question, only a fresh live
  run answers it.
- That the **no-tool/static** encodings of A/C still round-trip through
  `verify-oracle` at 100% once options enumerate a whole region.
