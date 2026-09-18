# CivSpatial — Kind Roster (pre-registration draft)

*Draft 2026-08-07, reconciled with the holistic analysis in `kind-roster-analysis.md`.
The **frozen list of question kinds** for the fogged multi-board run, their per-kind
sample sizes, and which claim each serves. Pre-registration discipline: this roster is
fixed **before** any LLM sees a board, so kinds cannot be cherry-picked post-hoc (the
hindsight-scoring failure the reasoning-frontier design guards against). Counts are
proposals for owner review. This covers the KIND ROSTER only; the full run **matrix**
(encodings × models × think × reps) is the follow-on doc.*

> ## ⚠ AMENDMENT 2026-08-13 — the three reasoning-frontier hidden-info kinds are AUDITED OUT (validity defect: information-bound / luck)
>
> Pre-registration discipline cuts both ways: a kind may be **removed** after the fact only
> if the removal reason is a stated validity defect discovered *independently of any score*, and
> the removal is logged rather than silently applied. This is that log. The three
> **T3-frontier hidden-information kinds** — `hidden-force` (P3), `fogged-assault` (P7f),
> `surprise-strike` (P1) — are removed from the LLM run and reclassified as **information-bound
> (luck), not reasoning-bound.** They remain in code as an illustrative *ceiling-theorem exhibit*.
>
> **Why (the diagnostic, not the scores).** A plateau under maximal tooling has two causes and only
> one is a real frontier: **reasoning-bound** (answer IS a function of the observable masked board,
> but needs reasoning no single tool bundles → *a smarter model climbs*) vs **information-bound**
> (answer depends on facts NOT in the observable inputs → *no reasoning helps; it is environmental
> entropy*). Diagnostic: *would 2× intelligence cross the plateau?* Auditing the three kinds against
> it — **3 for 3 luck**:
> - **surprise-strike (P1)** — scored on the *actual* hidden striker's placement (`hidden_strike_threat`
>   read `unmasked`). *Information-bound as originally built.* Redesigned to a deducible masked-board
>   worst-case (`surprise_exposure`, merged `a0a4fd3`) — which then **SATURATES 8/8 both surfaces**, i.e.
>   it becomes a tool-solvable positive control, not a frontier kind.
> - **fogged-assault (P7f)** — `worst_case_garrison` scanned the *unmasked* board. Fixed to solve on the
>   masked board (observed-repertoire, merged `d904de7`); the `no-visible-defender → yes` branch then
>   makes ground truth *strategically naive*, and the model scores **below the majority-class baseline**
>   (a risk-posture mismatch, not a reasoning failure).
> - **hidden-force (P3)** — **AUDITED = luck** (`analysis/hidden-force-audit.md`, committed perturbation
>   tests): `rules::hidden_force_in_region` sums `att_eff` over `unmasked.units` on fogged tiles; a
>   committed test flips the answer `(9,0)→(6,0)` on a **byte-identical masked board**. Demote, don't
>   redesign — a deducible redesign would just be another saturating computation (per surprise-strike).
>
> **The deeper reason (ceiling theorem).** Any deterministic function of a fully-observable frozen board
> is tool-computable by construction, so "reasoning-bound difficulty on a frozen board" is only ever *the
> tool we withhold*. Genuine tool-resistance needs **luck** (rejected), **intractability**, or **time
> horizon** — no free lunch. Full argument: `analysis/reasoning-frontier-vs-luck.md`.
>
> **Consequence for the run.** The big fog-frontier matrix is **CANCELLED**. Re-scoped to **3 arms:
> `raw` vs `raw-maxops` vs `roster-maxops`** on the decision kinds + ≥1 perception kind (raw≪maxops =
> the accuracy/compute ablation; raw-maxops vs roster-maxops = the cost frontier). The removal is a
> **credibility signal** ("we audited our own kinds and found three of them luck"), not a hole — under
> path (a) "everything saturates under maxops" is the *expected* shadow of the ceiling theorem, and the
> signal lives in arm-spread + cost + won't-self-select + the withheld-tool survivors (constraint-site,
> compare-two-attacks). **In-run kinds are now 16** (the 19 below minus the fog trio).
>
> The `In run?` cells for the three kinds below are left at their pre-amendment values with a **✂ AUDITED
> OUT** marker so the original frozen record stays visible.

Corpus: the 15-board fogged corpus in `data/corpus/selection-optimized.txt` (3 early / 3
mid / 9 late, 15 distinct games, 5/5/5 sizes). "Support" = # of those 15 boards that yield
the kind under fog (`data/corpus/yield-fullpool.json`). The run generates up to
`--per-kind K` decisive items per (board, kind), so **effective N ≈ K × support**. Given
the ~12–18% per-item noise floor, pool ≥3 reps for any sub-0.05 accuracy claim.

## The frozen roster (proposed `K = 8`)

| Kind | Tier | Support /15 | K | eff. N | In run? | Serves | Note |
|---|---|---|---|---|---|---|---|
| terrain | T0 | 15 | — | — | **OUT** | — | Saturated readout; Oracle floor-check only. |
| adjacency | T0 | 15 | — | — | **OUT** | — | Saturated. |
| direction | T0 | 15 | — | — | **OUT** | — | Saturated. |
| distance | T1 | 15 | — | — | **OUT** | — | Saturated Chebyshev readout (agent: could keep as a cheap floor — low priority). |
| nearest | T1 | 15 | — | — | **OUT** ▲ | Encoding demo | ▲ Demoted: collapses under calc, breaks interactive, least game-real; signal covered by the two turn-costed variants. **Retain only in the interactive arm** as the unbounded-search failure demonstrator. |
| **region-count** | T1 | 15 | 8 | 120 | **IN** | Encoding | The counting wall — strongest encoding discriminator (interactive/hierarchical showcase). |
| **reachability** | T1 | 15 | 8 | 120 | **IN** | Encoding | 8-dir flood; cheap distinct spatial skill. |
| **best-site** | T1 | 15 | 8 | 120 | **IN** | Encoding + decision-lite | Local aggregation; interactive beat front-loaded here. |
| **nearest-owned** | T1-rp | 6† | 8 | 48 | **IN** ⚠ | Encoding + tool-fit | Multi-source parity-gap win (0.667→1.000). † early/mid boards only (E). |
| **reachable-nearest** | T1-rp | 6† | 8 | 48 | **IN** ⚠ | Encoding + tool-fit | Single-unit Dijkstra reach. † early/mid only (E) — ~41% of late items are 0–1 turns (free rail). |
| **unit-strength** | T2 | 12 | 8 | 96 | **IN** | Tool-fit | Stated combat model floor. |
| **city-defense** | T2 | 14 | 8 | 112 | **IN** | Tool-fit | Best-defender + walls. |
| **constraint-site** | T3 | 15 | 8 | 120 | **IN** ★ | Tool-fit | ★ **Survivor under full tooling (0.79)** — feature it. |
| **compare-two-attacks** | T3 | 12 | 8 | 96 | **IN** ★ | Tool-fit | ★ **Survivor under full tooling (0.88)** — feature it. |
| **settle-site** | T3 | 10 | 8 | 80 | **IN** ° | Tool-fit | ° Saturates to 1.00 in maxops; carries the *arm* spread. Thinnest support. |
| **adv-assault-target** | T3 | 15 | 8 | 120 | **IN** ° | Tool-fit | ° Saturator; arm-spread evidence. |
| **cf-vacate** | T3 | 12 | 8 | 96 | **IN** ° | Tool-fit | ° Saturator (0.96). |
| **triage-reinforce** | T3 | 12 | 8 | 96 | **IN** ° | Tool-fit | ° Saturator (0.92). |
| **t3-retreat** | T3 | 12 | 8 | 96 | **IN** | Tool-fit + frontier(b) | 3-axis Pareto; safe-twin family. |
| **t3-threat** | T3 | 12 | 4 | 48 | **IN** ▽ | Tool-fit | ▽ Weakest discriminator (single-axis, tool-trivial); keep as defense-POV mirror, **sample light (K=4)**. |
| **forward-posting** | T3-frontier | 12 | 8 | 96 | **IN** | Reasoning-frontier | P4 champion (engine b). |
| **hidden-force** | T3-frontier | 11 | 8 | 88 | ~~IN ✦~~ **✂ AUDITED OUT** | ~~Reasoning-frontier~~ luck | ✦ P3 (engine a, hidden-info): which fogged area hides the massed force. **AUDITED = information-bound (luck), 2026-08-13** — see amendment + `hidden-force-audit.md`. |
| **fogged-assault** | T3-frontier | 12 | 8 | 96 | ~~IN ✦~~ **✂ AUDITED OUT** | ~~Reasoning-frontier~~ luck | ✦ P7f (engine a): take a fog-garrisoned city. **Reclassified information-bound (luck), 2026-08-13** — masked-board fix made ground truth strategically naive; see amendment. |
| **surprise-strike** | T3-frontier | 3 | 12 | 36 | ~~IN ✦⚠~~ **✂ AUDITED OUT** | ~~Reasoning-frontier~~ luck | ✦ P1 (engine a): most-exposed city vs an unseen striker. **Reclassified 2026-08-13**: deducible redesign SATURATES 8/8 → tool-solvable positive control, not a frontier kind; see amendment. |

**In-run: originally 19 kinds** (16 + the frontier hidden-info trio P7f/P1/P3); **now 16 after the
2026-08-13 amendment** removed the fog trio as information-bound (luck). OUT: the 5 saturated/demoted
readouts (terrain, adjacency, direction, distance, nearest) — retained in code as the Oracle
floor-check; `nearest` also kept as the interactive-arm demonstrator — **plus the 3 audited-out fog
kinds** (hidden-force, fogged-assault, surprise-strike), retained in code as the ceiling-theorem exhibit.

Legend: ★ discrimination survivor under full tooling (highest value) · ° saturates within
the maxops arm but IS the cross-arm tool-fit evidence · ▽ low discrimination, keep-light ·
✦ built reasoning-frontier hidden-info kind (engine a, scored on the unmasked board) ·
⚠ corpus-interaction flag · ▲ demoted this revision.

## Kinds → claims

- **Encoding thesis** (raw/ascii/hierarchical/interactive/maxops): region-count, reachability,
  best-site + the two movement kinds; `nearest` demo in the interactive arm only.
- **Tool–task-fit (claims #1/#2)**: the 2 T2 + 8 T3 decision kinds. Feature the **survivors**
  (constraint-site, compare-two-attacks); the saturators carry the no-ops→maxops arm spread.
- **Headline (claim #3)** raw-maxops-enum vs interactive-maxops-enum: decision + dispersed kinds
  on both surfaces.
- **Clean raw vs interactive (claim #4)**: perception + decision, post-fix rerun.
- **Reasoning-frontier chapter**: `forward-posting` now — **one kind deep** (see BUILD).

## BUILD — recommended before/with this run (from the holistic analysis)

1. **Global / whole-board aggregation kind** *(encoding thesis; the agent's #1)*. "Which quadrant
   holds the most {terrain/cities}", "which player controls the most tiles", "how many {X} on the
   whole board". **Zero current coverage**, and RESUME predicts this is exactly where hierarchical/
   interactive separate from flat `raw` — the claim-#3 headline currently has **no kind to test its
   own prediction**. Cheapest high-leverage build: question-design + a trivial deterministic solver,
   **no fog pipeline, no corpus change**. Strongly recommended before the run.
2. **Frontier hidden-info pair/trio: P1 + P3 + P7f** *(frontier thesis)*. The engine-(a)
   hidden-information payoff — "the thing nothing in the field has" — has **no built kind**. The
   fogged corpus precondition is now met; the only lift is the one-time **unmasked-board pipeline**
   (solve truth on the true board, render masked, + a leak-invariant test), paid once and amortized.
   P7f is cleanest (static garrison, no speed-leak, no history). Needed for a frontier chapter deeper
   than one kind — decide vs running P4-only now.

**Later / conditional:** N1 (blocked on a turn-history corpus regen), P5 (generation probe first —
drop if it collapses into settle-site's safety axis), economic-value-beyond-size + multi-turn
planning (decode/history-blocked). P2/P6 stay dropped.

## Corpus-interaction flags (resolve before freezing)

1. **Movement kinds vs a late-heavy corpus.** `reachable-nearest`/`nearest-owned` go trivial on
   railed late boards; 9/15 are late. Restrict their sampling to the 3 early + 3 mid boards (or add a
   min-turns decisive filter). Do NOT sample uniformly.
2. **settle-site thinnest (10/15).** If it must carry weight, raise its K rather than adding boards.
3. **Within-maxops light sampling.** The saturators (°: adv-assault-target, settle-site, cf-vacate,
   triage-reinforce) add little *within* the maxops arm — sample them lightly there and spend the
   budget on the survivors (★) + frontier kinds. (Full sampling in the no-ops/spatial-ops arms, where
   they supply the spread.)

## Decisions (resolved 2026-08-07)

- **A.** ✅ **Approved (tentative)** — 16-in / 5-out; `distance` also dropped. `nearest` retained
  in code as the interactive-arm search-blowup demonstrator only.
- **B.** ✅ **Approved** — `K = 8` default; `t3-threat` `K = 4`; `settle-site` `K = 12`.
- **C.** ❌ **Declined** — no global/whole-board aggregation kind: a counter nails it (owner), which
  is exactly why the frontier line disqualifies aggregates. Consequence: the encoding headline rests
  on the local-aggregation kinds `region-count` + `best-site`; no whole-board kind.
- **D.** ✅ **DONE — P7f + P1 + P3 built and merged** on the unmasked-board seam (§v2.3, GenCtx).
  Measured fogged support on the 15-board corpus: **P3 hidden-force 11/15** (abundant, 12/board),
  **P7f fogged-assault 12/15** (broad, thin per board), **P1 surprise-strike 3/15** (narrowest — the
  design's predicted "widest-sweep" kind; bumped to K=12, still ~36 N). Round-trip green; synthetic
  tests in `civ-eval/tests/fog_hidden_force.rs`.
- **E.** ✅ **Movement kinds → early/mid boards only** (6 boards, N 48 each) — avoids rail-triviality
  on the 9 late boards.

**Status:** roster was **FROZEN at 19 kinds** (16 + P7f/P1/P3); **amended 2026-08-13 → 16 in-run kinds**
after the fog trio was audited out as information-bound (luck) — see the amendment banner at the top.
Remaining before the run: pre-register the re-scoped 3-arm matrix (`raw` vs `raw-maxops` vs
`roster-maxops` × the 16 kinds × boards × reps). Budget top-up gates the run.
