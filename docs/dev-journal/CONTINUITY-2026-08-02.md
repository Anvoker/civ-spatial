# CivSpatial — Continuity / Handoff, 2026-08-02 (end of day)

Read this first, then `RESUME.md` (evergreen state + commands) and `analysis/findings-tool-parity.md` (today's
headline). Supersedes `CONTINUITY-2026-08-01.md`.

> **Headline — "more tools" is not the lever; TOOL–TASK FIT is.** Two linked results, both on the clean (post-B1)
> harness: (1) on decision kinds, generic spatial ops don't help (no-ops 0.64 ≈ spatial-ops 0.61 ≪ maxops 0.92) —
> only the *matching* combat/valuation ops win; (2) a tool surface must be parity-complete with the solver, or the
> model pays an unaffordable composition and shortcuts to a wrong heuristic. We proved (2) end-to-end: added the
> missing multi-source primitive and the residual hard kind (nearest-owned) jumped **0.667 → 1.000**.

## The arc of the day
1. **Confirmed the positional-filter** (nearest-owned, T677/p2, 3 seeds): enum cap-out `invalid`s collapsed 8 → 1,
   accuracy 0.522 → 0.667. The filter fix from 2026-08-01 works. (`results-nearowned-filter-seed{1,2,3}.jsonl`.)
2. **Ran the SURVIVOR experiment** (the big open thread): 6 decision kinds × raw/raw-calc/raw-maxops × 3 seeds,
   T677/p2, $2.50. Result = **tool–task fit** (see below). (`results-survivor-dense-seed{1,2,3}.jsonl`.)
3. **Failure-analysis agent** on the 7 wrong `raw-calc-enum` nearest-owned traces (verified: rebuilt the solver,
   reproduced all 7 expected, replayed every tool value vs BFS). Verdict: **all model errors, not a harness bug**;
   6/7 = incomplete argmin, root cause = a **tool-parity gap** (solver does multi-source BFS from all cities;
   tool only had point-to-point `travel_turns`).
4. **Design agent → draft → user-approved → implemented + confirmed** the fix: `travel_turns_from_owned`.
5. **Documented** both results in `analysis/findings-tool-parity.md`; **merged** the tool to master.

## Result A — tool–task fit (survivor experiment)
6 decision kinds, T677/p2 dense, think-off, n=24/kind/arm (n=144/arm):

| kind | raw (no-ops) | raw-calc (spatial ops) | raw-maxops (full calc) |
|---|---|---|---|
| adv-assault-target | 0.62 | 0.65 | **1.00** |
| settle-site | 0.58 | 0.62 | **1.00** |
| cf-vacate | 0.83 | 0.83 | **0.96** |
| triage-reinforce | 0.58 | 0.33 | **0.92** |
| compare-two-attacks | 0.58 | 0.62 | **0.88** |
| constraint-site | 0.62 | 0.58 | **0.79** |
| **arm total** | **0.64** | **0.61** | **0.92** |

- **NOT a monotonic ladder:** no-ops ≈ spatial-ops ≪ maxops. Generic spatial ops are the *wrong* tool for decisions
  (raw-calc *below* no-ops, 15 cap-out invalids; triage-reinforce collapses to 0.33). Only the matching
  combat/valuation ops (maxops) win (+0.28), and maxops is *cheaper per item* than raw-calc (stalls less).
- **Mirror of the dispersed-kind result** (`findings-clean-ablation.md`): same spatial ops, opposite outcome by
  question type. Tool–task fit, not tool count.
- **Survivor sub-finding:** `adv-assault-target`/`settle-site` saturate to 1.00 under maxops (collapse);
  `constraint-site` (0.79) and `compare-two-attacks` (0.88) stay discriminating — `constraint-site` is the hardest.

## Result B — the tool-parity gap, fixed (`travel_turns_from_owned`)
`nearest-owned` needs min land-turns from the *nearest own city*; the solver runs a multi-source BFS from ALL own
cities (`ReachField::from_origins`), but the tool surface only had point-to-point `travel_turns` → the model must
loop O(tiles × cities), can't afford it, guesses wrong. **Fix:** new enum op `travel_turns_from_owned(player,tx,ty)`
= min land-turns from the nearest own city, horizon baked in at 6 (`NEAREST_OWNED_HORIZON`, now `pub(crate)`), kept
SPLIT from the exclusion filter so the outer argmin stays under test. On `raw-calc-enum`/`raw-maxops-enum`/
`interactive-maxops-enum`.

**Confirmation** (nearest-owned, T677/p2, 3 seeds; enum arm has the tool, raw/raw-calc are controls):

| arm | before | after |
|---|---|---|
| raw (control) | 0.542 | 0.583 |
| raw-calc (control) | 0.522 | 0.667 |
| **raw-calc-enum (has tool)** | **0.667, 7 wrong** | **1.000 (23/23), 0 wrong** |

Residual hard kind **solved**, and cheaper/fewer turns. `results-nearowned-ttfo-seed{1,2,3}.jsonl`.

## Repo state
- **master** (`067be07`): `travel_turns_from_owned` merged (branch `travel-from-owned`, `--no-ff`). `just check` +
  `verify-oracle` green (T50 + T677 fogged), **100 civ-eval lib tests**, clippy `-D warnings`. New parity tests
  mirror the `list_tiles` exclusion gate. `findings-tool-parity.md` added.
- No unmerged feature branches. (`travel-from-owned` merged; can be deleted.)

## THE BLOG PLAN — the 4 claims, with status (recorded here + in RESUME.md)
The publishable spine. Each is one clean (post-B1) harness generation:
1. **maxops ≻ ops (tool–task fit)** — ✅ **DONE** (survivor experiment, Result A above; `findings-tool-parity.md`).
   Bonus: the parity-gap demonstration (Result B) is a strong companion mini-result.
2. **maxops-enum vs maxops** — ⏳ **UNRUN.** Add `raw-maxops-enum` as a 4th arm on the 6 decision kinds × 3 seeds
   (or standalone). Expectation from the trimmed analog (calc vs calc-enum): *no accuracy gain, ~2× efficiency* —
   frame dominance on the 2D cost frontier, not accuracy alone.
3. **raw-maxops-enum vs interactive-maxops-enum (the HEADLINE)** — ⏳ **UNRUN, ZERO DATA on either side.** Decision +
   dispersed kinds, ≥3 seeds, both boards. Delivers the headline (raw-maxops-enum > raw) AND the front-loaded-vs-
   query-loop comparison at full tooling. Biggest gap.
4. **clean raw vs interactive** — ⏳ **UNRUN.** raw · interactive (+ their -ops variants), dispersed kinds, both
   boards, post-B1. Replaces the SUSPECT pre-B1 numbers (raw-ops 0.986 small / 0.736 dense ties) — those were on
   the broken harness and must not be published as-is.

**Sequencing:** #2 folds into #1's sweep (one 4-arm run); #4 folds into #3 (one raw/interactive/maxops-enum sweep).
Whole-blog clean spend ≈ **$12–18** across both boards at current post-B1 dense costs → **top up budget before #3.**

## Ops notes
- Session cloud spend today ≈ **$3** (confirmation ~$0.5 + survivor $2.5 + ttfo re-run ~$0.55; two analysis/design
  agents were local). Budget still has room but #3 will need a top-up.
- Rebuild remote before cloud runs (`cargo build -p civ-cli --features remote`); source `.env`, never key on CLI.
- The B1 render-all-units change makes the T677/p2 board block ~42k tokens, re-read each tool-loop turn → tool-loop
  arms cost ~$0.011/item on the dense board (recomputed from fresh data, ~4× the old estimate).
- Noise floor ~12–18%/item → pool/vote ≥3 reps for sub-0.05 claims. Per-kind n=24 is ~3-item resolution (noisy);
  arm totals (n=144) are solid.
- Housekeeping still pending: one-time `cargo fmt` normalization commit (isolated); `git worktree prune`.
