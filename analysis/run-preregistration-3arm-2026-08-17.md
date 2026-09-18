# Run pre-registration — the 3-arm clean run (frozen 2026-08-17)

**Status: FROZEN before any LLM sees these boards.** This document fixes the full run
specification — kinds, arms, boards, models, parameters, scoring, and the predicted results —
*ahead of execution*, so nothing can be cherry-picked post-hoc. Any change after this point goes in
the **Deviations log** at the bottom (dated), not silently into the spec.

Companion docs: `kind-roster-preregistration.md` (roster + the 2026-08-13 fog-audit amendment),
`reasoning-frontier-vs-luck.md` (the two-plateaus diagnostic + ceiling theorem — why this is path (a)),
`question-design-review.md` (per-kind rationale + real examples used to pick the 11).

Run date: **Monday 2026-08-18** (pre-flight `--per-kind 1` first). Harness: hardened
(post-B1 stacking / post-C1 cache-deduct budget / S1 Answer-line / S2 value-beats-none / 6-key
resume). This run **replaces** the pre-B1/pre-C1 numbers, which carry the broken-harness asterisk.

---

## 1. Design & claims

Frozen-board, fogged, **path (a)** ("own the microscope's ceiling"). Within-maxops saturation is
*expected* (ceiling theorem); signal lives in the arm contrasts, the cost axis, and the two
withheld-tool survivors — not in within-maxops accuracy.

| # | Contrast | Claim | Pre-registered prediction |
|---|---|---|---|
| C1 | **raw vs {raw-maxops, roster-maxops}** on decision kinds | Tools are the **compute** lever — an LLM can't *reliably* decide by eyeballing | raw ≈ 0.5–0.7 on decision kinds; maxops arms ≈ 0.85–1.0. Gap is large + consistent. |
| C2 | **raw-maxops vs roster-maxops** | **Cost frontier**: equal accuracy, front-loading the whole board is strictly dominated | Accuracy within noise (|Δ| < ~0.05); roster-maxops ≈ 2–3× fewer prompt tokens, largest on the **large** board. |
| C3 | **perception kinds (region-count, reachability) under raw** | Tools = *compute*, not a perception crutch — raw isn't uniformly hopeless | raw ≈ 0.8+ on the perception kinds (≫ its decision-kind accuracy). |
| C4 | **constraint-site, compare-two-attacks under maxops** | Withheld-tool exhibit — "difficulty on a frozen board = the tool we didn't bundle" | Both stay **below saturation** (~0.75–0.9), unlike the other decision kinds (~0.95+). |
| C5 | **DeepSeek vs Gemini** (exploratory) | Behavioral divergence is agentic disposition (tool-engagement), not just accuracy | No directional accuracy prediction; expect tool-call-count / turn-count differences. |

---

## 2. Kinds (11) — frozen roster

| Kind | Role / claim | Answer type | Mid board | Late board |
|---|---|---|:--:|:--:|
| region-count | perception control (C3) | number | ✓ | ✓ |
| reachability | perception control (C3) | yes/no | ✓ | ✓ |
| nearest-owned | spatial (movement) | number/"none" | ✓ | ✗ |
| reachable-nearest | spatial (movement) | number/"none" | ✓ | ✗ |
| city-defense | tool-fit T2 | which-city | ✓ | ✓ |
| **constraint-site** | ★ survivor (C4) | coords/"none" | ✓ | ✓ |
| **compare-two-attacks** | ★ survivor (C4) | coords/"incomparable" | ✓ | ✓ |
| t3-retreat | tool-fit decision | any sound coords | ✓ | ✓ |
| cf-vacate | tool-fit decision | yes/no | ✓ | ✓ |
| triage-reinforce | tool-fit decision | which-city | ✓ | ✓ |
| forward-posting | tool-fit (adversarial 1-ply) | any sound coords | ✓ | ✓ |

**Movement kinds (nearest-owned, reachable-nearest) run on the MID board only** (decision E —
rail + dense cities trivialize them on the late board). Late board = 9 kinds.

**Excluded, with reason (anti-cherry-pick log):**
- **Dropped from this run** (kept in code): settle-site (underdetermined "best site"; thinnest
  support), adv-assault-target (scalar argmax; capture-axis ⊂ t3-threat), unit-strength (near
  table-lookup, redundant with city-defense), best-site (narrow proxy, perception covered by
  region-count), t3-threat (single-axis special case of adv-assault-target). See
  `question-design-review.md`.
- **Audited out as information-bound / luck** (2026-08-13): hidden-force, fogged-assault,
  surprise-strike. See the `kind-roster-preregistration.md` amendment + `reasoning-frontier-vs-luck.md`.
- **`site_axes` retired from the maxops menu** — the one tool no in-run kind uses once
  settle-site/best-site are dropped (dispatch retained for viewer/parity; model can't call it).

---

## 3. Arms (3)

| Arm | Encoding (code id) | Description | Tools |
|---|---|---|---|
| raw | `raw` | Full masked board front-loaded, single-shot | none |
| raw-maxops | `raw-maxops` | Full board front-loaded + maxops menu (tool loop) | maxops (site_axes retired) |
| roster-maxops | `interactive-maxops` | Roster front-loaded, terrain fetched (`scan_region`) + maxops menu (tool loop) | maxops (site_axes retired) |

`roster-maxops` is the **presentation label**; the code id stays `interactive-maxops` (traces,
`summarize.py`, viewer unchanged).

---

## 4. Boards (2, from the pre-registered corpus)

| Slot | Save | Era / size | Fog perspective | Yield check |
|---|---|---|---|---|
| Mid | `corpus_medium_a3_s1337-T0120-Y00380-auto.sav` | mid / medium | `--fog 1` (Atawallpa) | all 11 kinds yield 8/8 |
| Late | `corpus_large_a6_s42-T0221-Y01600-final.sav` | late / large | `--fog 4` (François Mitterrand) | all 11 yield 8/8; movement kinds excluded by design |

*(Offline oracle pre-flight 2026-08-17 confirmed yields. `--fog 4` chosen on the late board because
`--fog 1` there yielded 0 combat-kind instances — a perspective with no reachable threats.)*

---

## 5. Models (2)

| Model | Slug | Role | Ping (2026-08-17, think off) |
|---|---|---|---|
| DeepSeek V4 Flash | `deepseek/deepseek-v4-flash` | workhorse | `pong` OK |
| Gemini 2.5 Flash | `google/gemini-2.5-flash` | cross-lineage contrast | `pong` OK; `reasoning_effort:"none"` accepted (not rejected) |

*Residual check for the pre-flight:* confirm Gemini's completion/reasoning-token counts stay
low think-off on real questions (a 1-token pong can't prove reasoning is fully suppressed).

**Deviation 2026-08-18 (cost): initial run is DeepSeek ONLY.** Gemini is ~75% of the spend
(15× output pricing × output-heavy tool loops) and C5 is exploratory, so Gemini is **deferred** to a
later run on this same frozen spec, from the tagged commit `run-3arm-frozen-2026-08-18`. The paired
comparison stays clean because generation is deterministic (same seed → byte-identical questions,
temperature 0) — the later Gemini pass sees the exact questions DeepSeek saw, provided the harness
commit is unchanged and the slug is provider-pinned. C1–C4 run in full now; C5 becomes a later add-on.

---

## 6. Fixed parameters

- **Think:** OFF (`reasoning_effort:"none"`) — matches all prior comparison data; cleanest compute ablation.
- **Difficulty:** hard · **Cache:** on · **per-kind K:** 8 · **Seeds:** 2 (`--seed 1`, `--seed 2`)
- **max-turns:** 16 (tool loops) · **budget:** cache-deduct + 2M backstop (turns govern) · **concurrency:** 4 (cloud)
- **Resume:** on (6-key: encoding, item_id, board, model, think, effort) — crash-safe incremental flush.

---

## 7. Scoring

- Type-directed `Scorer` → correct / wrong / invalid. All answer types require a literal `Answer:`
  line (S1); a value beats "none" where a value exists (S2).
- **Acceptable-set kinds** (constraint-site, t3-retreat, cf-vacate, forward-posting): `Answer::ChoiceSet`
  with the acceptable set precomputed at generation — any acceptable pick = correct, a
  decisively-dominated pick = wrong. compare-two-attacks: better target or "incomparable".
- **Decisiveness filter:** only decisive instances are generated (near-ties dropped at generation,
  logged per kind), so every scored item has an unambiguous ground truth.
- Oracle-100% invariant holds for all 11 kinds on both boards (`just check` green).

---

## 8. Trial accounting

- Mid board: 11 kinds × 8 × 2 seeds = **176 items**
- Late board: 9 kinds × 8 × 2 seeds = **144 items**
- Unique items: **320** · × 3 arms × 2 models = **1,920 trials** (initial DeepSeek-only run =
  **960 trials**; Gemini's 960 deferred — see the 2026-08-18 deviation in §5/§12)
- Effective N per (kind, arm, model): 16 (mid-only kinds) to 32 (both-board kinds), pooled across 2 seeds.
- **Noise floor ~12–18%/item** → treat any |Δaccuracy| < ~0.05 as within noise (2 seeds bound it, do not eliminate it).

---

## 9. Analysis plan (pre-committed)

- Report accuracy by (arm × kind), pooled over models+boards+seeds, and broken out per board and per model.
- Report **prompt tokens / trial** and **tokens-per-correct** by (arm × board) for C2 (cost frontier).
- Report tool-call count + turn count by (model × arm) for C5.
- Primary tables: C1 (raw vs maxops, decision kinds), C2 (raw-maxops vs roster-maxops cost),
  C3 (raw on perception), C4 (survivors under maxops). C5 is exploratory (no hypothesis test).
- Regenerate the viewer sample bundle from these results (item 6, deferred until now).

---

## 10. Cost / wall-clock estimate

Rough **$15–40** (DeepSeek cheap + cached; Gemini pricier on output), a few hours wall-clock with
concurrency. **Wall-clock is the constraint, not $.** Monday's `--per-kind 1` pre-flight calibrates
exact spend + per-arm wall-clock before the full launch.

---

## 11. Run commands (reference — the script wires these with `--resume`)

```sh
cargo build -p civ-cli --features remote        # RESUME gotcha: rebuild remote right before the run
set -a; source .env; set +a
BASE=https://openrouter.ai/api/v1
ENC="--encoding raw --encoding raw-maxops --encoding interactive-maxops"
MID="region-count,reachability,nearest-owned,reachable-nearest,city-defense,constraint-site,compare-two-attacks,t3-retreat,cf-vacate,triage-reinforce,forward-posting"
LATE="region-count,reachability,city-defense,constraint-site,compare-two-attacks,t3-retreat,cf-vacate,triage-reinforce,forward-posting"

# 8 invocations = {mid fog1, late fog4} × {seed 1,2} × {deepseek, gemini}; all append to one --out with --resume.
# (per-model, per-board, per-seed loop; --kinds = MID or LATE per board.)
```

---

## 12. Deviations log

- **2026-08-18** — Initial run is **DeepSeek only** (cost: Gemini ≈75% of spend, C5 exploratory).
  Gemini deferred to a later paired run on this frozen spec from tag `run-3arm-frozen-2026-08-18`;
  comparison stays clean via deterministic generation + temp 0 (see §5). C1–C4 unaffected; C5 deferred.

- **2026-08-18 — cf-vacate de-confounding validity fix (sampler balanced; oracle UNCHANGED).**
  The original cf-vacate item set was **all expected="no"** on both frozen boards/perspectives (mid
  `corpus_medium_a3_s1337-T0120` fog 1; late `corpus_large_a6_s42-T0221` fog 4), both seeds. This is a
  **validity confound, not a cherry-pick**: every threatened frontier city on those boards is garrisoned
  by a **single** defender, so pulling the best defender leaves the city UNDEFENDED → `city_fall_prob_
  undefended = 1.0` → decisive fall → "no". With a constant answer, accuracy cannot separate a *reasoned*
  "no" from a bare "no" prior, and it manufactured a spurious "tools hurt" inversion (raw 0.53 /
  raw-maxops 0.23 / interactive-maxops 0.17, all on an all-"no" set).
  **Fix (generators.rs `CfVacateKind::generate`):** the sampler was verdict-blind (uniform draw over
  garrisoned cities). It now **buckets every candidate city by its oracle verdict** and draws the most
  even yes/no mix the board+perspective supports (deterministic seeded Fisher–Yates within buckets;
  interleaved draw; a `[cf-vacate] balance:` availability line logged). The oracle
  (`question.rs::CanVacateCity`, 0.3/0.7 bands, presupposition of current security) and the
  `Answer::Bool` contract are **untouched** — only *which* cities are asked changed. Unit test
  `generators::cf_vacate_balance` proves the set is non-constant on a board that supports both verdicts.
  **This rerun supersedes the original cf-vacate rows.**
  **Both frozen boards cannot exercise the fix — corpus-content, not a code bug.** An exhaustive
  per-city oracle sweep of the whole corpus (216 saves × every living-civ fog perspective) shows the
  two **pinned (board, perspective) pairs have ZERO "yes"-capable cities** — multi-defender cities
  exist only in the safe rear (no live threat → correctly dropped). So resampling *within the frozen
  boards* stays all-"no". "Yes" cases are common elsewhere (99/216 saves, 314 balanced perspective-
  slots, 797 yes-items).
  **OWNER DECISION (2026-08-18): standalone auxiliary run.** cf-vacate is **EXCLUDED from the main
  frozen-board cross-kind table** (corpus-degenerate on both pinned boards, per the 216-save sweep) and
  is instead measured by a **separate targeted 3-arm experiment** on the best-balanced yes-capable view
  found by the sweep: **`corpus_medium_a9_s1337-T0200-Y01490-auto.sav` fog 4 (Little Crow)** —
  available yes=8 / no=8, the corpus's highest balanced yield; a *medium* board on seed *1337* (same
  size class + map-seed family as the frozen mid board `medium_a3_s1337`, differing only in agent count
  a9 vs a3). GATE 1 confirmed both seeds emit a clean **4 yes / 4 no** at `--per-kind 8 --difficulty
  hard`. The aux run mirrors the frozen 3-arm params (raw / raw-maxops / interactive-maxops; DeepSeek;
  think off; cache on) but its numbers are **NOT directly comparable to the other kinds' cells** — it
  is a *different board*, run as a mechanism experiment (does the current-state `city_fall_prob` tool
  help on genuine "yes" cases while hurting on "no"?), not a cross-kind comparison. Results:
  `results-cfvacate-aux-medium-a9-s1337-T0200-fog4-s{1,2}-deepseek.jsonl`.

- **2026-08-20 — reachability oracle treated OCEAN as walkable (validity fix; re-scored, no re-run).**
  The `reachability` oracle `question.rs::reachable()` (an 8-neighbor BFS) skipped a tile only if it
  was the `avoid` terrain (Mountains) or tactically ZOC/enemy-blocked — it **never checked `is_land`**,
  so its path could wade straight across water. **Every other movement construct in the codebase is
  land-only** (nearest-owned & reachable-nearest solvers, settle/retreat/forward-posting generators,
  and the `travel_turns`/`reach_turns` tools all restrict to `crate::question::is_land`); the
  reachability oracle was the lone outlier that forgot the ocean guard. **The oracle was wrong and the
  tools were right.** On items whose short path crosses water the oracle marked `expected="yes"`, but a
  real land unit (and the movement tools) cannot get there → the tools correctly returned "unreachable",
  the tool-armed models answered "no", and they were scored WRONG against a physically-impossible ground
  truth. Proven example: `reachability:48,46:52,46:6:Mountains:Atawallpa` (mid board) — the straight
  line crosses Ocean; `travel_turns` returns "unreachable", yet the buggy oracle said reachable.
  **Fix (`question.rs::reachable`):** add an `is_land` impassability guard so the walk can never ENTER a
  water tile (guarded unconditionally in the neighbor loop, alongside the existing `avoid` skip — water
  is impassable to a land unit whether or not ZOC applies), plus a goal guard (a non-land goal is
  unreachable, mirroring the existing "cannot end on a forbidden tile" early-return). The Mountains/
  `avoid` and tactical ZOC/enemy logic are untouched. Regression tests `reachable_cannot_cross_water_
  channel` and `reachable_land_detour_around_water_succeeds` added. `just check` green.
  **Re-scoring (NO re-run, NO paid calls):** the model never sees the oracle and the movement tools were
  already land-only, so every recorded model answer (`got`) is unchanged — only `expected`/`correct`
  move. The exact already-answered reachability items were re-evaluated under the fixed `reachable()` on
  the same masked boards (mid fog 1 / late fog 4) and re-scored with the harness's `got`-vs-yes/no rule
  (`error` rows stay out of the denominator). **The generator does NOT use the reachable verdict to
  select items** (`ReachableKind::generate` calls `reachable()` only to COMPUTE each answer, never to
  filter/balance), so the item_id set is identical to a regeneration and the re-score is valid.
  **14 of 32 unique items flipped `expected` yes→no (0 no→yes); all 14 are water-crossing** (11 land on a
  water goal, 3 reach a land goal for which no all-land path exists within budget). **Corrected per-arm
  reachability accuracy (pooled, old→new):** raw 0.875→**0.625** (drops — it had been matching the buggy
  oracle), raw-maxops 0.516→**0.935**, interactive-maxops 0.625→**0.938**. **The spurious "tools hurt"
  inversion fully reverses:** the two maxops arms now decisively BEAT raw instead of trailing it. Overall
  3-arm aggregate shift (all kinds, old→new): raw 0.720→0.695, raw-maxops 0.833→0.874, interactive-maxops
  0.852→0.883. **These corrected reachability numbers supersede the original reachability rows.** Note
  (generator, not a validity issue): removing ocean-crossing leaves the fixed corpus yes/no-skewed (6
  yes / 26 no across the 32 items) because the goal-band heuristic was tuned when ocean counted as
  reachable; a *future* fresh run may want to rebalance the band, but that does not affect the validity
  of re-scoring the already-answered items.
