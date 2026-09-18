# CONTINUITY — 2026-08-13 EOD

Dated handoff snapshot. Durable state + command reference live in `RESUME.md`.

## Headline
A **conceptual-breakthrough day.** We audited the reasoning-frontier kinds by a sharp new diagnostic
and discovered that **all three fog / hidden-information kinds (surprise-strike, fogged-assault,
hidden-force) were measuring LUCK, not reasoning** — their apparent tool-resistance came from
*unmeasurability*, not from hard reasoning. Chasing a fix surfaced a deeper truth — the **ceiling
theorem**: any deterministic function of a fully-observable frozen board is tool-computable by
construction, so "reasoning-bound difficulty" on a frozen board is only ever *the tool menu we withhold*.
Genuine tool-resistance needs one of exactly three things — **luck** (rejected), **intractability**, or
**time-horizon** — and the latter two are expensive by nature. **Decision: path (a) — own the
microscope's ceiling** rather than chase an un-tool-solvable frozen kind. The day produced two merged
redesigns, one audit, one live test that proved the point, and a re-scoped experiment. **The thesis comes
out sharper, not weaker.** Blog is already underway; Saturday (2026-08-15) still in play.

## Git state
- **master = `d904de7`** (fast-forwarded today). Today's merged commits:
  - `a0a4fd3` redesign(surprise-strike): deducible worst-case exposure from the masked board
  - `d904de7` redesign(fogged-assault): observed-repertoire worst-case (masked-board solve)
  - Both green (`just check` = test + clippy -D + verify-oracle ×4). Observed-repertoire philosophy: the
    worst-case garrison/striker is bounded by the enemy repertoire the model can actually SEE, so the
    answer is a pure function of the masked board (no hidden-placement luck).
- **UNMERGED — hidden-force audit** (evidence, no production change): branch
  `worktree-agent-ace542d2a5e081e20`, commit **`d527fd1`** (atop `d904de7`). Contains
  `analysis/hidden-force-audit.md` + two perturbation tests in `mod hidden_force_audit`
  (`civ-eval/src/generators.rs`). Safe to leave; branch persists even if the worktree is pruned.
- **Worktrees still present** (prune tomorrow after the audit merge decision):
  `agent-a5a556cdd524b029d` (redesign/surprise-strike-deducible — already merged) and
  `agent-ace542d2a5e081e20` (the audit branch).
- **Uncommitted (unchanged from 2026-08-12, still parked for owner):** `viewer/src/sample-data.js` +
  `viewer/samples/latest/` + the root export JSONs.
- Test output (gitignored): `results-hiddeninfo-test.jsonl` + `traces/results-hiddeninfo-test-1786610872878/`.

## The big result — reasoning-bound vs information-bound ("the two plateaus")
A kind's accuracy can plateau under maximal tooling for **two fundamentally different reasons, and only
one is a real frontier:**
1. **Reasoning-bound (real frontier):** the answer IS fully determined by the observable (masked) board,
   but computing it needs reasoning no single tool bundles — adversarial lookahead, constraint
   composition, search, trade-off judgment. **A smarter model would climb.**
2. **Information-bound (luck / fake frontier):** the answer depends on facts NOT in the observable inputs
   (hidden placement, hidden tech). No reasoning helps; the plateau is environmental entropy. **A smarter
   model does NOT climb.**
- **Diagnostic:** *would 2× intelligence cross the plateau?* Yes → reasoning-bound. No → luck.

## The fog kinds — 3 for 3 LUCK (the hidden-information engine is retired as a frontier mechanism)
- **surprise-strike (P1)** — was keyed on the *actual* hidden striker's placement (`hidden_strike_threat`
  read `unmasked`). Redesigned to a deducible masked-board `ThreatField` worst-case (`surprise_exposure`).
  **MERGED (`a0a4fd3`).** Consequence, proven live: once deducible it **SATURATES (8/8 both surfaces)** →
  it's a *positive control now, not a frontier kind*.
- **fogged-assault (P7f)** — was worst-case-robust already, but `worst_case_garrison` scanned the
  *unmasked* board. Fixed to solve on the **masked** board → observed-repertoire. **MERGED (`d904de7`).**
  Consequence, proven live: the `no-visible-defender → undefended → yes` branch makes ground truth
  *strategically naive* (a lone Artillery "captures" a fogged city). The model prudently answers "no" and
  scores **below the majority-class baseline (raw 1/8, roster 3/8)** — a risk-posture mismatch, not a
  reasoning failure.
- **hidden-force (P3)** — **AUDITED = LUCK.** `rules::hidden_force_in_region` (rules.rs:1001) sums
  `att_eff` over `unmasked.units` on fogged tiles; the committed perturbation test flips the answer
  `(9,0)→(6,0)` with a byte-identical masked board. Recommendation: **demote** (don't redesign — a
  deducible redesign would just be another saturating computation, like surprise-strike). See
  `analysis/hidden-force-audit.md`.

## The ceiling theorem (why this kept happening)
**Any deterministic function of a fully-observable frozen board is tool-computable by construction.** So a
kind "resists tools" only because we *withhold* the bundling tool (write `find_constraint_site()` and
constraint-site saturates; write "compute the 2-ply-sound advance" and forward-posting saturates). The
"frontier" was never a property of the problem — it was the tool menu. Genuine tool-resistance therefore
requires **exactly one of three sources, no others:**
1. **Luck** — answer not a function of the observable state (hidden info / stochastic). *Rejected — it's
   not reasoning.*
2. **Intractability** — the ideal tool exists but can't run in feasible time (deep game trees, NP-hard
   optimization). Grading goes fuzzy (best-known vs optimal).
3. **Time horizon** — the "answer" is the emergent outcome of a long interaction you can't precompute
   without playing. This is where the latency/cost lives — the price of the only honest escape.
- **Corollary (the strong thesis):** reasoning-boundness is a property of task **structure**
  (adversarial / compositional / temporal), NOT of information-hiding. And **frozen-state benchmarks
  structurally cannot demonstrate a game-competence gap** (there's always an ideal tool) — which *explains*
  why benchmark scores and real play diverge. This is a *better* headline than "fog is hard."

## Decision — PATH (a): own the microscope's ceiling
Chosen over (b) buy-intractability (fuzzy grading) and (c) buy-time (rollouts — the real post-Saturday
frontier, deferred). **Do NOT chase an un-tool-solvable frozen kind — it's a theorem, not a design gap.**
Bank the real frozen-board findings + the ceiling meta-finding as the methodological spine:
tool–task-fit, the calculator lever, won't-self-select, **and** "difficulty on a frozen board = the tool
withheld" (the fog audit is the live case study).

## The 2-kind live test (proof of the above) — `results-hiddeninfo-test.jsonl`
DeepSeek, `corpus_medium_a6_s1337-T0221-Y01600-final.sav` fog 1, per-kind 8, surprise-strike +
fogged-assault × raw-maxops + roster-maxops (32 trials):
- surprise-strike **8/8 both surfaces** (saturated — deducible ⇒ trivial).
- fogged-assault raw **1/8**, roster **3/8**; ground truth was 7 yes / 1 no, model answered "no"
  categorically (risk-posture mismatch, below the "always-yes" baseline).
- **roster-maxops beat raw-maxops overall (11/16 vs 9/16) at ~2.6× fewer tokens** (228k vs 597k mean
  prompt) — the cost-frontier point, clean.

## Question-set disposition under path (a)
- **Floor-check (code only, never in the LLM run):** terrain, adjacency, direction, distance, nearest —
  unchanged.
- **Perception lever — keep:** region-count, reachability. **best-site DEMOTE** ("the best site" is
  underdetermined in real play — a validity problem the owner flagged).
- **Tool-fit decision workhorses (the core evidence now):** unit-strength, city-defense, constraint-site,
  compare-two-attacks, settle-site, adv-assault-target, cf-vacate, triage-reinforce, t3-retreat, t3-threat.
  These are all tool-solvable — under path (a) that's the *point*; they carry tool–task fit + calculator +
  won't-self-select via the ARM SPREAD (no-ops→spatial-ops→maxops) and the COST axis, not within-maxops
  accuracy. Movement kinds (nearest-owned, reachable-nearest) stay restricted early/mid.
- **Fog three → RETIRE from the run, keep as the ceiling-theorem EXHIBIT:** surprise-strike (keep the
  deducible redesign as a saturating-control illustration), hidden-force + fogged-assault demoted to
  illustrative. Their value is now the "we audited our own kinds and found them luck" credibility signal.
- **"Everything saturates under maxops" is EXPECTED** — it's the shadow of the ceiling theorem, not a
  hole. Signal lives in arm-spread, cost, won't-self-select, and the two withheld-tool survivors
  (constraint-site 0.79, compare-two-attacks 0.88 — reframed as the *live demo* that difficulty = the tool
  we didn't provide).

## The re-scoped experiment (frozen-board, path (a))
**3 arms: `raw` vs `raw-maxops` vs `roster-maxops`** — the big fog-frontier matrix is CANCELLED.
- **raw vs the maxops arms = the accuracy ABLATION** — the LLM can't reliably do decision kinds by
  eyeballing (~0.64) vs ~0.9+ tooled → tools are the *compute* lever. Say "cannot *reliably*," not
  "cannot at all" (0.64 > chance).
- **raw-maxops vs roster-maxops = the COST FRONTIER, not accuracy** — equal accuracy, front-loading the
  whole board is strictly dominated (roster-maxops ~2.6× cheaper). Frame on cost to sidestep the
  roster-convergence confound (below); the equivalence-on-accuracy claim needs enough n to bound the diff.
- **Include one perception kind (region-count/reachability)** — raw is fine there, so it shows raw isn't
  uniformly hopeless → tools = a *compute* lever, not a perception crutch (sharper claim).
- Run on the decision kinds + 1 perception kind, medium + large corpus boards, on the hardened harness —
  replaces the pre-B1/pre-C1 numbers that carry the "broken harness" asterisk. This is the "modest clean
  run" (the only thing that costs cloud time).

## Naming — `roster-maxops` (PRESENTATION LAYER ONLY)
"interactive-maxops" now overclaims — the query loop shrank to a single terrain fetch (`scan_region`) and
the occupant roster is front-loaded on both arms; the only real difference is **terrain front-loaded (raw)
vs fetched (roster)**. Decision: relabel to **`roster-maxops`** in the **blog / figures / viewer labels
only** — keep the code identifier `interactive-maxops` (historical `results-*.jsonl`, traces, viewer,
`summarize.py` all stay intact). Optional post-Saturday: a clean code rename with a back-compat alias.

## Prompt-size measurement (re: "trim the system prompt to relevant rules?")
Measured from the test traces (approx tokenizer; the board undercounts because grids tokenize dense):
- **Rules block = ~3.6–4k tokens, IDENTICAL on both surfaces** (13,097 chars exactly — the fixed all-kinds
  bundle; only ~1 of ~12 blocks is relevant per question).
- raw-maxops ~64.8k prompt tok/turn: rules ≈ **6%**, board ≈ ~90%.
- roster-maxops ~17k prompt tok/turn: rules ≈ **~24%** (the owner's instinct was right — for the *lean*
  surface).
- **BUT 98–99.9% cached** → trimming saves ~$0. It's a *context-cleanliness* question, not cost; the
  question already names its rule block, so the model isn't hunting. **Decision: HOLD trimming for
  post-Saturday** (churn + a per-kind prompt muddies cross-kind cost comparisons); note as a methods
  footnote ("rules are ~¼ of the lean surface but cached, so we left them").

## Ideas parked for post-Saturday (path (c) — "buy time", the only real frontier left)
- **Tempo / lookahead via bounded rollouts** — decide, simulate 2–3 *contested* turns, score the outcome.
  Escapes tool-solvability through adversarial branching, at the latency cost named. This is where the Vox
  Deorum "thin on timing, no lookahead" evidence has teeth.
- **forward-posting → 2-ply** (deeper adversarial) — lower-risk, but same axis; on a frozen board it's
  still tool-reducible (the ceiling theorem bites).
- **fogged-assault reframe** the owner sketched (NOT built): "given you must assume the era's best
  defender, which of these 4 hypothetical stacks have >50% capture odds?" — a subset/exact-match answer
  (~1/16 guess rate). Clean, but STATING the defender removes the fog → it becomes a combat-CALCULATION
  tool-fit kind (saturates), not a frontier kind; needs a new `Answer::Subset` type + an era-baseline
  (turn→best-defender small table, NOT a tech-availability model). Deferred.

## PENDING — tomorrow's tasks (all path (a), no new kinds)
1. **Merge the hidden-force audit artifacts** — branch `worktree-agent-ace542d2a5e081e20` / `d527fd1`
   (doc + 2 passing perturbation tests, no production change). Then prune both worktrees.
2. **Amend `analysis/kind-roster-preregistration.md`** — reclassify the fog three as *audited-out on a
   validity defect (information-bound / luck)*, with the reason. This is the credibility move: document the
   exclusion, don't silently drop (guards against the cherry-pick charge).
3. **Write `analysis/reasoning-frontier-vs-luck.md`** — the standalone methods doc (two-plateaus
   definition + smarter-model diagnostic + the 3/3 fog reclassification + the ceiling theorem). *Owner owns
   the final blog framing — write it as evidence, don't declare the thesis "locked."* (A first draft may be
   started today alongside this file.)
4. **Small production edit** — mark hidden-force/fogged-assault illustrative and drop the fog kinds from
   the default run set (`DEFAULT`/run kind list); keep in code.
5. **The 3-arm clean run** (raw vs raw-maxops vs roster-maxops on decision kinds + 1 perception kind) —
   the only cloud-cost item; produces the publishable post-fix numbers.
6. Decide whether to commit the viewer sample bundle (still uncommitted).
7. **Blog** (already underway, `C:\Projects\anlog-blog`) — fold in the two-pole squeeze + the ceiling
   theorem as the methods spine.

**Health:** `just check` green on `d904de7`. `cargo build -p civ-cli --features remote` before any cloud
run; key in `.env` (`set -a; source .env; set +a`). Do NOT `cargo fmt --all` (rustfmt drift). Fog runs use
`--fog <player-id>`; corpus boards + per-kind fog yields in `data/corpus/yield-selected.json` (note:
surprise-strike/hidden-force yields there are PRE-redesign and stale).
