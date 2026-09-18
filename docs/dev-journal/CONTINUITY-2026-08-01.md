# CivSpatial — Continuity / Handoff, 2026-08-01 (end of day)

Read this first, then `RESUME.md` (evergreen state + commands), `analysis/findings-clean-ablation.md` (today's
headline result), `analysis/findings-bug-sweep.md` + `analysis/findings-fog-bug-rootcause.md` (the bugs we found),
and the two REVISED docs (`findings-scale-ceiling.md`, `findings-enum-residual.md`). Supersedes
`CONTINUITY-2026-07-31.md`.

> **Headline — we paused the experiment to audit the harness, and it paid off.** A bug sweep found the eval had
> **two classes of result-corrupting bug**, both now fixed, and the clean rerun **overturned the enumeration
> story**: the calculator (arithmetic) is the accuracy lever; **enumeration adds NO accuracy, only ~2× efficiency**
> — its prior "accuracy lift" was a broken board render in disguise.

## The arc of the day
1. Resumed; ran the enumeration ablation (`raw-maxops-enum` 0.847 > controls) — looked like enumeration broke the
   scale "locating ceiling."
2. Failure analysis of the residual (`findings-enum-residual.md`) turned up a `reach_turns` "no modeled unit" bug.
3. **Paused the follow-up pooling run** (near-zero spent) rather than measure on a suspect harness.
4. Root-caused it (`findings-fog-bug-rootcause.md`) — **NOT fog; unit STACKING** — and ran a full **eval-validity
   bug sweep** (`findings-bug-sweep.md`, with a trace-reading sub-agent over 587 traces).
5. Fixed everything (harness batch → master; B1 stacking contract → maxops line), assembled a clean experiment
   branch, built dead-op-free "trimmed" arms, and **re-ran clean** (`findings-clean-ablation.md`).

## Bugs found + fixed
- **B1 — unit stacking (the big one).** T677 (turn-677) has up to **37 units on one tile**. The same `(x,y)`
  resolved 3 ways: solver by **id** (correct), render **last-wins** (dropped 36 of 37 — a *perception* bug),
  operators **first-wins** `.find()` (wrong unit). Fixed on `maxops-enum` (commit `9368764`): render lists **all**
  units with ids; operators resolve unit subjects **by id** (`unit_by_id`); reconstruction gate agrees; latent
  sea-unit `reach_turns` bug fixed; stacked-tile test added. Ground truth was never corrupted (solvers use id).
- **B2/B3/B4/B6 — harness batch** (merged to master, `7888ab0`): B2 answer extraction no longer fabricates `got`
  from prompt/trailing text (Int/OptionalInt/Coord read the `Answer:` line only → unparseable = `invalid`, not a
  stray integer); B3 deterministic `most_common_terrain` tiebreak; B4 empty/0-token completions → distinct `error`
  status excluded from the denominator (retry once first); B6 injective trace filenames.
- **Verified NOT bugs / near-zero impact:** the 4 basic operator↔solver parities, fog consistency in the merged
  path, scorer classification, `summarize.py` aggregation; the "empty completions" bug (B4) had only 4 isolated
  rows in real data and did NOT corrupt the frontier finding (canonical file clean, 0.771 stands).

## Today's result (clean harness) — `findings-clean-ablation.md`
T677/p2 (98% explored), 3 dispersed kinds, DeepSeek think-off, 3 seeds, n=72/arm, trimmed arms:

| arm | acc | gen_tok | latency |
|---|---|---|---|
| raw (no-ops) | 0.542 | 741 | 7.2 s |
| raw-calc | 0.833 | 6,209 | 52.7 s |
| raw-calc-enum | 0.845 | 2,617 | 24.1 s |

- **Calculator/arithmetic is the accuracy lever** (no-ops→calc = +0.29). `reach_turns` drives reachable-nearest
  (14→22/24); count/list solves region-count (11→23/24).
- **Enumeration adds NO accuracy** (calc→enum = +0.012, noise) — it's a **pure ~2× efficiency lever** (24 s vs 53 s,
  2.6k vs 6.2k gen-tok at equal accuracy).
- **The prior enumeration accuracy lift (+0.055) was a render-bug artifact** — enumeration was compensating for a
  render that showed 1 of N stacked units; fix the render, the edge vanishes.
- **The old ~0.75 "locating ceiling" lifted to ~0.83–0.85** with the fixes — a real chunk of it was harness error.
- **nearest-owned is the residual hard kind, and enumeration makes it WORSE** (12/23, below no-ops 14/24). The
  "within-2-of-my-cities" exclusion tempts the model to brute-force the pairwise check — a capped trace showed
  **~178 hand `distance` calls** → over the turn budget → force-answered empty → `invalid`. Failure mode shifts
  from "wrong" (no-ops, one-shot guess) to "never finishes" (enum). More tools → more brute-forcing → more cap-outs.

## Repo state — EVERYTHING IS NOW ON MASTER (updated 2026-08-01 late, after the EOD merges)
- **master**: harness batch (B2/B3/B4/B6) + constraint-site + the **full maxops line** + the **positional-filter**,
  all merged. Key commits: **`b2b9f08`** (merge maxops-enum → master: maximal calculator, enumeration group, **B1
  stacking fix**, trimmed `raw-calc`/`raw-calc-enum` arms, constraint-site), **`4f5a1b3`** (merge positional-filter:
  `list_tiles` + `exclude_owner`/`exclude_radius`), plus `36c6ef9` (session docs). `just check` + `verify-oracle`
  green on both boards × both tiers + fogged; **98 civ-eval lib tests pass**. The B1 render fix is now on master, so
  **all** encoders (raw/ascii/hierarchical/…) render stacked units correctly — the perception bug is gone repo-wide.
- **No unmerged feature branches remain** — `maxops-enum` and the positional-filter branch are both merged. Old
  agent worktrees under `.claude/worktrees/` can be pruned (`git worktree prune` after removing the dirs).
- The `maxops-enum` merge decision (previously the "top open decision") is **RESOLVED** — merged.

## Pending / next steps (prioritized)
1. ✅ **DONE — merged the maxops line to master** (`b2b9f08`); the B1 render fix is now on master (top open decision
   from earlier today, resolved).
2. ✅ **DONE (built + merged) — the positional-FILTER operator** for nearest-owned (`4f5a1b3`): `list_tiles` gained
   optional `exclude_owner` + `exclude_radius` (Chebyshev-2 around all own cities, parity-tested vs the
   `NearestOwnedResource` solver — set-equal incl. stacked/fogged cases). All `*-enum` preambles steer the model to
   use it. **⏳ NOT yet confirmed live — THIS IS THE IMMEDIATE NEXT STEP:** a small nearest-owned re-run
   (`raw-calc-enum` on T677/p2, 3 seeds) to verify the ~8 cap-out `invalid`s collapse and nearest-owned accuracy
   recovers (before = 12/23; expect a jump). ~$0.5, needs a go. Ideally paired: run with the filter available and
   compare to the banked `results-clean-p2-seed{1,2,3}.jsonl` nearest-owned numbers (which had the filter absent).
   Command form is in "Ops notes" below — add `--kinds nearest-owned`.
3. **The SURVIVOR / decision-kind experiment (the actual maximal-calculator point) — still UNRUN, biggest open
   research thread.** cf-vacate, adv-assault-target, triage-reinforce, compare-two-attacks, settle-site,
   constraint-site under `raw-maxops` on the CLEAN harness. These are the kinds where the combat/valuation ops
   actually fire, so B1 (operators read stacked tiles by id) matters here. Question: which survive free measurement
   (stay discriminating) vs collapse? Run on both a small (6.8%) and the dense (98%) board.
4. **Firm the calc-vs-enum accuracy claim** if desired — the +0.012 is noise; honest claim is "no accuracy
   difference, ~2× efficiency." A repeated-item majority vote (same seed) would tighten it; the efficiency win is
   already method-independent. Probably not worth the spend.
5. **Cross-model / think-on** on the clean harness (lower priority; the DeepSeek think-off story is the spine).
6. **Housekeeping:** repo-wide `cargo fmt` drift exists (installed rustfmt disagrees with committed style across
   many untouched files); `just check` excludes `fmt` by design. A one-time `cargo fmt` normalization commit would
   silence it — do it in isolation, not bundled with feature work. Also `git worktree prune` the merged worktrees.

## Ops notes
- Session cloud spend today ≈ **$4** (old ablation ~$2 + clean run ~$1.5–2; the pooling run was cancelled at ~$0).
  Budget was topped to $15; plenty left.
- Rebuild remote before cloud runs (`cargo build -p civ-cli --features remote`). Source `.env`, never key on CLI.
- **Trimmed arms** live on `maxops-enum`: `raw-calc` (5 ops), `raw-calc-enum` (6 ops). Clean-run command form:
  `run --board data/saves/testcontroller_T677.sav --fog 2 --seed N --per-kind 8 --difficulty hard --think off
  --cache on --concurrency 4 --kinds nearest-owned,reachable-nearest,region-count --encoding raw --encoding
  raw-calc --encoding raw-calc-enum --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1
  --api-key "$OPENROUTER_API_KEY" --out results-clean-p2-seedN.jsonl`.
- The B1 render-all-units change enlarged the T677 board block → clean runs are ~3–7× slower per seed than the old
  ablation. Expected cost of complete perception.
- Noise floor ~12–18%/item → pool/vote ≥3 reps for sub-0.05 claims.
