# CivSpatial — Continuity / Handoff, 2026-07-29

Where we left off. Read this first, then `RESUME.md` (evergreen state + commands), `DESIGN.md`,
`T2-T3-design.md` (the approved valuation design — now implemented), and `analysis/findings-*.md`
(newest results in **`findings-interactive.md`** Stage D). Supersedes `CONTINUITY-2026-07-28.md`.

> **Headline of this session: four things shipped in parallel** — a new **`nearest-owned`** bounded
> question kind, **territory full/partial** inclusion in the interactive summaries, the **larger-n +
> think-on** siting experiments (interactive still wins), and the whole **T3 valuation tier**
> (`settle-site` dominance). All committed, `just check` green.

## Repo state
- **Branch `master`.** Key commits this session:
  - `593fc15` — `nearest-owned` kind + `Answer::OptionalInt` (bounded empire-relative nearest-resource).
  - `9d462a7` — `describe.rs` full-vs-partial territory inclusion ("Entirely player X's territory").
  - `6c8c178` — merge of the two parallel agent branches.
  - `3b6fa7f` — **T3 settle-site dominance tier** (`Answer::ChoiceSet` + graded threat field).
- `just check` green (tests + clippy `-D warnings` on default **and** `--features remote`, + Oracle-100%
  at both difficulty tiers). Untracked: `T1g-T3-questions-draft.md` (still a draft), `results-*.jsonl`.

## What shipped (2026-07-29)

### 1. `nearest-owned` — bounded, empire-relative nearest-resource (task 1+2)
Replaces the pathology of the old global `nearest` (unbounded search → interactive blew up to 200k+
tokens). New kind: *"distance from player P's nearest city to the closest UNEXPLOITED {resource} tile
(outside the 2-tile working radius, within horizon 6), or 'none'."* Horizon **stated in the question**
so the search is bounded by construction; "none" is a real outcome (`Answer::OptionalInt(Option<i64>)`).
The **unexploited filter** (distance > 2 from own cities) is the domain-real "resources you don't yet
work." The old global `nearest` is **kept** as the deliberate interactive-loser boundary case. Category
string `nearest-owned`; T50/T677 both yield ~8 numeric / ~4 none per per-kind-12 draw.

### 2. Territory full/partial in interactive summaries (task 3)
`describe.rs::ownership_phrase` now emits **"Entirely player X's territory."** when a region is wholly
inside one player's territory, distinct from the partial "Mostly"/"Partly"/"Contested" cases. Propagates
to the interactive quadrant overview + `region_summary` tool automatically. Helps territorial-containment
questions.

### 3. Siting confirmation (n=16) + think-on pass (tasks 4+5) — `findings-interactive.md` Stage D
- **D1 (n=16, think-off):** interactive **48/48 (1.000)** vs raw 0.708, hierarchical 0.729, at ~4.4×
  fewer tokens. The interactive win **holds decisively at larger n**. Walk-back: raw is **0.75 on
  best-site**, NOT the 4/8 "at chance" from the Stage-C small sample — honest claim is "materially worse,"
  not "chance." Cost fat-tail persists but bounded (max 61k, all best-site, all correct).
- **D2 (think-on):** all three converge to ~perfect (raw 23/24, hier + interactive 24/24) — **reasoning
  lifts the static encoders to interactive's accuracy** — BUT interactive keeps a big **cost + latency**
  win (10k tok / 30 s vs raw 66k / **120 s**). Sharpened thesis: **board access matters most when
  reasoning is off/cheap; with reasoning on it drives COST, not accuracy.**

### 4. T3 valuation tier — `settle-site` dominance (task 6) — `3b6fa7f`
The user-approved `T2-T3-design.md` §4, implemented in full (T2 was already built). Pieces:
- **`Answer::ChoiceSet { acceptable, options }`** — set-valued dominance scoring: correct = any
  non-dominated pick, **wrong = a dominated blunder**, invalid = off-option. `acceptable` precomputed at
  generation → the scorer stays board-free. **Negative Oracle test** added (a dominated pick must score
  *wrong*).
- **`rules.rs` threat field** — `ThreatField`: per-enemy **bounded terrain-aware BFS**, threat =
  `att_eff / reach_turns` (ceil, min 1 turn), faded to 0 past horizon **H=6**. Air/sea excluded. Plus the
  four **settle axes** (food / production / resources / safety) over the Chebyshev-2 working radius, and
  `SiteAxes` Pareto dominance (`dominates` / `dominates_terrain`).
- **`SettleSiteChoice` (T3)** question + 4-axis dominance solver; settle-axis prose appended to
  `rules_block` (flows to T3 items via the existing tier seam — no runner change).
- **Generator** = targeted trap construction: a threatened trap + an **identical-terrain safe twin**
  (dominates only via safety → *judgment-loaded*, not terrain-counting) + non-terrain-dominating
  distractors. Admissible only if the solved set has a dominated trap AND ≥2 sound options.
- **Feasible on both boards:** 12 decisive instances each on T50/T677 (drops ~50/48 — the targeted
  construction's expected yield); Oracle-100%.

## ⏸️ WHERE WE LEFT OFF — START HERE NEXT
Everything below is committed and green. Nothing is running. `.env` holds the OpenRouter key.

### DONE this session (unattended batch, 2026-07-29) — see `analysis/findings-realplay-corpus.md`
Built the **reachability-bounded real-play corpus** (commit `2e04eac`): `reachable-nearest` (scout — nearest
resource a UNIT can reach over land within 6 turns at its move rate) + refit `nearest-owned` to **travel-cost**
(terrain-aware land reach from cities, not raw Chebyshev), both via a shared `rules::ReachField`. Then ran the
**first live T3 + fair-corpus battery** across raw / hier / interactive on **both boards** (T677 + T50), 6
kinds (settle-site, best-site, region-count, terrain, nearest-owned, reachable-nearest), think-off, per-kind 12.

**THE HONEST RESULT — "interactive beats the pants off everything" was a curated-slate artifact.** On the fair
corpus interactive's edge is modest and kind-dependent: T677 interactive **0.736** / raw 0.625 / hier 0.569;
**T50 0.736 / 0.708 / 0.681 (within CIs — a coin-flip on the sparse board).** Robust cross-board shape:
- **Interactive WINS local aggregation** (terrain, best-site, region-count): 11–12/12 both boards.
- **Interactive LOSES dispersed-integration + travel** (nearest-owned, reachable-nearest): 5/12 both boards,
  raw ahead. **The bounded reframe did NOT rescue it** — bounding the search space doesn't help when the task
  needs integrating scattered cities/units + resources + travel cost (the fetch-loop is bad at this think-off,
  + 7 force-answer invalids on T677).
- **T3 settle-site is board-dependent** — interactive wins dense T677 (9/12), loses sparse T50 (7/12; hier 10).
- Interactive's edge **scales with board size** (static encoders are already cheap on small T50).
- **First live T3 data:** the blunder-rate metric separates encoders; the tier behaves.

### DONE — fog of war (`--fog`), and it REVERSES the interactive story early-game (`analysis/findings-fog.md`)
The save stores each player's explored map (`map_t`) separately from the global truth we parse; real
players know only **3%** (T50) to **7–98%** (T677). Built `--fog <player-id>` masking (commit `9ba5623`):
`Board::mask_to_known` → unexplored tiles become `Unknown`, occupants hidden, own cities kept; `is_land`
excludes Unknown; `rand_tile` skips it (determinism preserved when no fog); Oracle-100%-on-fogged invariant
+ civ-core fog tests; `just check` green. Fuller masked runs (fair corpus, both boards):
- **T50 → player1 (8.5% explored, early-game): interactive is STRICTLY DOMINATED — 0.700 vs raw 0.833, hier
  0.850, at ~10× tokens and ~12× latency.** On a ~320-tile known map raw's whole board block is 5k tokens,
  so interactive's "don't dump the board" pitch buys nothing and its round-trips make it the worst option.
- **T677 → player0 (52% explored, mid-game): interactive still leads 0.792 but hierarchical catches up to
  0.736** (omniscient was 0.569) — the edge compresses as the realistic board shrinks.
- **Gradient = game phase:** interactive's rank goes last (early) → 1st-narrowly (mid) → 1st (omniscient).
  Its advantage scales with known-map size and is *counterproductive early* — i.e. for most of a real game.
Session spend **~$2.67** of the $10 budget.

### Start here next
1. ✅ **DONE — known-fraction sweep (fixed T677, players at 6.8 / 52 / 98% explored)** — the gradient HOLDS on
   one board: interactive rank last (6.8%) → 1st (52%) → 1st (98%). **Sharper mechanism:** interactive's
   accuracy is flat (~0.82); the STATIC encoders collapse as the map fills in (raw 0.931 → 0.667 → 0.597) —
   query-loops are board-size-invariant, static dumps anti-scale, and they cross. Interactive's dispersed-
   integration weakness persists at every level. `results-fog-t677-p4/p0/p2.jsonl`, `findings-fog.md`.
2. **Cross-model on the FOGGED corpus** — the biggest remaining validity threat (all DeepSeek). Does the
   early-game interactive collapse replicate on Gemini / a frontier anchor, and does the crossover *threshold*
   (% explored where interactive stops losing) shift by model?
3. **Think-on on the fogged corpus — now the PRIMARY hypothesis for the dispersed-integration deficit.**
   `list_cities`/`list_units` directory verbs were built + tested (commit `1ec8a9f`) and **REFUTED the
   "missing directory" hypothesis**: on T677/p4 (6.8%) interactive went 0.819 → 0.792 (flat/noise), still far
   below raw 0.931, though tool calls fell ~30% (50.8 → 36.1). So the deficit is a **reasoning/integration
   limit, not an access limit** — the model can now *find* its scattered assets cheaply but still can't
   *compute* travel-cost/nearest over them think-off. **CONFIRMED by the think-on test:** re-running the
   T677/p4 dispersed kinds with `--think on` lifted interactive **0.583 → 0.778** (reachable-nearest 6→10,
   settle-site 9→10, nearest-owned 6→8), collapsing the raw-vs-interactive gap from 0.31 → 0.055 — while raw
   stayed flat (0.889→0.833, already sees everything). So the deficit was reasoning-bound: verbs (access) did
   nothing, think-on (compute) fixed it. Caveat: interactive+think-on is ~137k tok/q vs raw's ~5k on this
   small board (~25×), so it's still not cost-competitive early. `results-fog-t677-p4-thinkon.jsonl`,
   `findings-fog.md`. Verbs kept (cost win). **Both think-off and think-on results are written up in
   `findings-fog.md`.**
4. **Occupant-visibility layer** — v1 fog over-informs (a known tile reveals live occupants; realistically
   only currently-lit tiles do). Optional: a 10-point sweep (all T677 players) for a publishable curve.
5. **⭐ OPEN IDEA (user, for tomorrow) — make interactive WIN on SMALL maps.** Intuition: interactive *should*
   be able to beat raw even on small/early boards, yet today it wastes 36–56 round-trips flailing when the
   whole known world is only ~5k tokens — the query-loop tax buys nothing when there's nothing to save. Angles:
   - **Adaptive / hybrid surface (most promising):** when the known (reachable) map is below a size threshold,
     fold the *entire* known board into the `overview` so interactive **degrades to raw on small maps**, and
     only switch to coarse-summary-plus-query when the board is large. Makes interactive ≥ raw on small maps
     by construction while keeping its large-map edge — directly kills the cost inversion.
   - **Scaling overview** / fold `list_cities`+`list_units` into the overview so it never spends turns just
     discovering its own assets.
   - **Batch fetch** (one call returns a large region) to cut round-trips.
   Note: the dispersed-kind *reasoning* limit is orthogonal (it hits raw too; think-on fixes both), so the
   achievable small-map win here is mostly removing the round-trip overhead, not the compute.

The fair-corpus command for reference (rebuild remote first — it bites every time):
```sh
cargo build -p civ-cli --features remote
set -a; source .env; set +a
K=settle-site,best-site,region-count,terrain,nearest-owned,reachable-nearest
./target/debug/civ.exe run --board data/saves/testcontroller_T677.sav --seed 1 --per-kind 12 \
  --difficulty hard --think off --cache on --concurrency 4 --kinds "$K" \
  --encoding raw --encoding hierarchical --encoding interactive \
  --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1 \
  --api-key "$OPENROUTER_API_KEY" --out results-t3-final-t677.jsonl
```

**Corpus direction — root questions in real play, reachability-bounded (agreed 2026-07-29).** New
governing principle now in **`DESIGN.md` §6** ("Question corpus principle") and fleshed out in
`T1g-T3-questions-draft.md` §4: define the corpus by *what a player actually does*, and bound spatial
scope by the player's **reachable space** (terrain-aware travel cost from a controlled entity), not the
whole map. This externalizes corpus fairness (defeats the "you tested what interactive wins" critique) and,
done honestly, forces the interactive-hostile tasks back in *as bounded versions*. Concrete build items:
- ✅ **DONE `reachable-nearest`** (scout task): nearest {resource} reachable within N turns *from a unit*
  (terrain-aware `ReachField`) — the real-play bounded form of global `nearest`. Built + run (`2e04eac`).
- ✅ **DONE — refit `nearest-owned`** to travel-cost (terrain-aware land reach) instead of raw Chebyshev.
- **Keep unbounded `nearest` as a DIAGNOSTIC** (failure-mode visibility), not a headline activity.
- **Test an egocentric + interactive encoder** (origin-relative overview + fetch outward) — but *measure*,
  don't assume: a fast unit's reachable set can be a large fraction of the map, so bounded ≠ auto-win.

**Other open threads (lower priority):**
- **`nearest-owned` live run** — does the bounded reframe let interactive handle it (vs the old global
  `nearest` blow-up)? Add `nearest-owned` to a `--kinds` mix and check interactive's turn/token cost.
- **T1g global-aggregation tier** (`T1g-T3-questions-draft.md`) — still a draft; the real test of "does a
  hierarchy that summarizes X beat raw on questions about X."
- **T3 follow-ons** (`T2-T3-design.md` §4.3): `t3-retreat`, `t3-attack-target`, `t3-defend-city`,
  `t3-expand-region` — all reuse `ChoiceSet`; build order T3a(done) → t3-retreat next.
- **Balanced count sweep**; **think-on `nearest-owned`**; the **cost fat-tail** on best-site (bounded at
  ~61k now, watch as n grows).

## Ops notes
- **Rebuild remote before every cloud run** (`cargo build -p civ-cli --features remote`) — `just check` /
  any plain `cargo build -p civ-cli` strips the remote feature and the next cloud run dies at dispatch.
- **`.env`** holds `OPENROUTER_API_KEY`; source it (`set -a; source .env; set +a`), never put the key on
  the command line.
- Parallel-build hygiene: to build offline while a cloud run holds `target/debug/civ.exe`, use
  `CARGO_TARGET_DIR=target-exp` (git-ignored).
