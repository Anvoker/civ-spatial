# The 3-arm clean run — raw vs raw-maxops vs roster-maxops

**The headline experiment. Executed 2026-08-18, analyzed 2026-08-19/20. DeepSeek V4 Flash, think-off,
fogged. Pre-registered (frozen 2026-08-17): `analysis/run-preregistration-3arm-2026-08-17.md`.**

> **Post-run correction (2026-08-20):** re-scoring surfaced an oracle-validity bug on **reachability** (the
> ground truth let a land unit walk across ocean). Fixed (`question.rs`, commit `521c0a1`), the recorded
> answers were **re-scored** against the corrected oracle (no re-run — the fix touches only ground truth,
> not the model's answers). **All numbers below are post-correction.** The pre-correction reachability
> figures (raw 0.88 / maxops 0.50 / 0.62) are superseded — see the reachability section and the deviation
> log §12.
>
> **Post-run correction #2 (2026-08-24) — answer-extraction fixes (tool arms were UNDERSTATED).** Two
> extractor bugs mis-scored *correct* model answers as wrong/invalid: (1) on verbose two-entity "which-city"
> answers the scorer grabbed the wrong name — the fogged/loser entity, not the one the model declared "better
> defended"; (2) coordinate answers formatted with markdown (`**83, 39**`) or without parens (`83, 39`) failed
> to match the `(83, 39)` option and scored *invalid*. Fixed in `scoring.rs` (`match_option` picks the
> declared winner; `norm` strips markdown + canonicalizes coord formatting), and the recorded completions were
> **re-scored through the real fixed extractor** (`civ rescore`, no re-run). Faithful-row reproduction was
> validated first: with the *pre-fix* extractor the rescore reproduces every recorded status exactly (0 changes
> in 954 rows). The fix flips **4 DeepSeek rows, all `city-defense`, all wrong→correct, with ZERO regressions**
> (correct→wrong = 0) — it only ever recovered mis-scored corrects. Both hits are in the **tool arms** (raw
> untouched), so this **widens** the C1 tool gap, stacking on the reachability correction. The city-defense
> cells, the aggregates, and the C1 discussion below are updated; superseded values are struck or noted.

## TL;DR

Two load-bearing claims confirmed, and the correction made both *stronger*:

1. **Tools are a compute lever (C1).** A single-shot model reading the masked board (`raw`) sits at
   **0.71**; give it the maxops operator menu and it jumps to **0.95–0.96** (cf-vacate removed). The gap
   is **+24–25 pts** after the reachability + extractor corrections. On movement kinds it's crushing
   (0.59 → 0.94–0.97).
2. **Roster-maxops strictly dominates raw-maxops on cost (C2).** Equal accuracy, but roster spends
   **2.5× fewer tokens** overall and **3.7× fewer on the large board** — because front-loading the whole
   masked board re-sends it every turn, while roster fetches only what it needs. Front-loading is off the
   cost frontier.

Three honest refinements:
- **Perception under `raw` is weak, not decent (C3 not supported).** With reachability corrected, raw
  scores 0.56 (region-count) and 0.62 (reachability) — a *perception wall*, not just a counting wall.
  Tools rescue both (0.91–1.00).
- **The pre-registered "withheld-tool survivors" saturated** under maxops rather than staying hard —
  consistent with the ceiling theorem, weaker as an exhibit.
- **One genuine tool-trap remains: cf-vacate** (tools can hurt — resolved → `findings-cfvacate-tooltrap.md`).
  The *other* apparent inversion, reachability, turned out to be **our oracle's bug, not the model's** — the
  tool arms were right all along (see below).

---

## What ran

| | |
|---|---|
| **Arms (3)** | `raw` (full masked board, single-shot, no tools) · `raw-maxops` (full board + maxops tool loop) · `roster-maxops` (roster front-loaded + `scan_region` terrain fetch + maxops tool loop; code id `interactive-maxops`) |
| **Model** | `deepseek/deepseek-v4-flash`, think-off. **Gemini deferred on cost** (deviation `ca34bdd`) → C5 unrun; single-model result. |
| **Boards** | mid = `corpus_medium_a3_s1337` fog p1 · late = `corpus_large_a6_s42` fog p4 |
| **Kinds** | 11 (mid) / 9 (late; movement kinds excluded — rail trivializes them). |
| **Params** | `--per-kind 8 --difficulty hard --think off --cache on`, 2 seeds. **954 trials.** |
| **Harness** | Hardened (post-B1 stacking / post-C1 cache-deduct budget / S1 Answer-line / S2 value-beats-none / 6-key resume). Replaces all pre-B1/pre-C1 numbers. |

**Deviations affecting the numbers:** (1) cf-vacate was corpus-degenerate on both frozen boards (all
answers "no") → **excluded from the interpretable aggregate**, re-measured standalone
(`findings-cfvacate-tooltrap.md`). (2) reachability oracle corrected post-run (ocean walkability) → answers
**re-scored**, not re-run. Both logged in prereg §12.

---

## Headline (post-correction)

**Overall, all 11 kinds as-run** (n = 318/arm; 95% Wilson CI):

| arm | accuracy | tot_tok / ans | tot_tok / correct | turns | tool calls | latency |
|---|---|---|---|---|---|---|
| raw | 0.695 [0.642, 0.743] | 40k | 58k | 1.0 | 0 | 9.1 s |
| raw-maxops | 0.881 [0.840, 0.912] | **367k** | 421k | 7.7 | 27.9 | 67.9 s |
| roster-maxops | 0.884 [0.844, 0.914] | **146k** | 166k | 8.7 | 21.8 | 63.0 s |

*(raw-maxops 0.871→0.881 and roster-maxops 0.881→0.884 vs the reachability-only correction: +3 / +1
`city-defense` extraction recoveries; raw unchanged.)*

**Overall, cf-vacate removed** (n = 288/arm — the interpretable headline):

| arm | accuracy |
|---|---|
| raw | 0.712 [0.657, 0.761] |
| raw-maxops | 0.948 [0.916, 0.968] |
| roster-maxops | **0.958 [0.929, 0.976]** |

The tool gap is **+24–25 pts** (raw-maxops +0.236, roster +0.247 over raw) and roster sits numerically ahead
of raw-maxops (CIs overlap) — roster is *at least* as accurate and much cheaper. *(raw-maxops 0.938→0.948,
roster 0.955→0.958 after the `city-defense` extraction fix; raw unchanged, so the gap widened.)*

---

## C1 — tools are the compute lever ✅ (strengthened)

`raw` cannot reliably decide *or perceive* on a fogged board; the matching operators fix both. The gap is
large, consistent, and present in every task group:

| task group | raw | raw-maxops | roster-maxops |
|---|---|---|---|
| decision kinds (incl. cf-vacate) | 0.739 | 0.847 | 0.856 |
| perception kinds | **0.594** | **0.953** | **0.953** |
| movement kinds | **0.594** | **0.969** | **0.938** |

*(decision-kinds raw-maxops 0.833→0.847 and roster 0.851→0.856 after the `city-defense` extraction fix;
raw unchanged.)*

The two maxops arms are statistically indistinguishable from each other — as pre-registered, within-maxops
accuracy saturates (ceiling theorem); the signal is in the arm *contrast* and the cost axis, not the
maxops-vs-maxops accuracy.

## C2 — the cost frontier: roster ≫ full front-load ✅ (the money result)

Equal accuracy, very unequal cost. Per-board tokens-per-answer (cf-vacate removed):

| board | raw-maxops | roster-maxops | roster advantage |
|---|---|---|---|
| mid | 149k | 134k | 1.12× |
| late (large) | **576k** | 155k | **3.72×** |
| pooled | 367k | 146k | 2.51× (2.57× per correct) |

The mechanism is exactly the pre-registered prediction: `raw-maxops` re-sends the entire masked board on
every tool turn, so its cost **explodes with board size** (576k tok/ans on the large board), while
`roster-maxops` front-loads only a compact roster and fetches terrain on demand. Front-loading the whole
board buys nothing over fetching it, and costs 2.5–3.7×. **Roster-maxops strictly dominates raw-maxops on
the cost frontier**, most decisively at scale — the result that motivates preferring a query loop over a
front-loaded dump.

## C3 — perception under `raw`: NOT supported ⚠️

Predicted `raw` ≥ 0.8 on the perception controls. **After the reachability correction, this fails:**
- **region-count**: `raw` 0.56 — the counting wall.
- **reachability**: `raw` **0.62** (was 0.88 against the buggy oracle; the ocean-walking ground truth had
  flattered raw, which just eyeballs grid distance).

So `raw` is weak on *both* perception kinds — a **perception wall**, not merely a counting wall — and tools
rescue both (region-count 1.00, reachability 0.94). This is the opposite of the pre-registered read and
folds perception squarely into the C1 "tools are the compute lever" story.

## C4 — the "survivors" saturated ⚠️ (softer than pre-registered)

Predicted constraint-site and compare-two-attacks would stay *below* saturation (~0.75–0.9) under maxops,
as a "difficulty = the tool we didn't bundle" exhibit. Instead they saturated: **constraint-site
roster-maxops 32/32 (1.00)**, **compare-two-attacks 30/32 (0.94)** both arms. Once the menu is present these
kinds are solved. This is *consistent* with the ceiling theorem (any frozen-board function is
tool-computable) — but it makes the withheld-tool exhibit less dramatic than hoped. Report as ceiling-theorem
support, not as a standing frontier.

---

## The two apparent inversions — one real tool-trap, one oracle bug

Two kinds *initially* showed the tool-less arm beating both tool arms. They turned out to be opposite in
kind — and this is a useful methodological point: **an arm disagreeing with `raw` is a signal to check the
ground truth, not just the model.**

**cf-vacate — a genuine tool-trap (tools hurt).** Original: raw 0.53 > raw-maxops 0.23 > roster 0.17, an
artifact of an all-"no" set. Root cause: the current-state `city_fall_prob` tool misleads on the post-vacate
*counterfactual*, biasing the model toward "yes." Fixed sampler + standalone balanced rerun show the
aggregate inversion **dissolves** and the true effect is an **answer-value asymmetry** (tools help on YES
8/8, hurt on NO 3/8→1/8). This is the run's "tools can hurt" exhibit. Full writeup:
**`findings-cfvacate-tooltrap.md`**.

**reachability — an ORACLE bug (the tools were right).** Original: raw 0.88 > raw-maxops 0.50, roster 0.62.
Root cause was *not* the model: the reachability oracle's BFS forbade only Mountains and tactical ZOC — it
**never checked `is_land`, so its ground-truth path could walk across ocean.** On items whose short path
crosses a water tile, the oracle said "reachable" but a real land unit (and the `travel_turns`/`reach_turns`
tools) correctly cannot get there → the tool arms answered "no" and were scored wrong against a
physically-impossible truth. `raw` scored well only by matching the buggy oracle. **Fix:** an `is_land`
guard in `reachable()` (`question.rs`, commit `521c0a1`; regression test = a land unit can't cross a 1-tile
ocean channel). **Re-scoring the recorded answers** (no re-run): 14 of 32 items flipped `expected` yes→no
(all water-crossing), and the inversion **fully reverses** —

| reachability | before (buggy) | after (corrected) |
|---|---|---|
| raw | 0.875 | **0.625** |
| raw-maxops | 0.516 | **0.94** |
| roster-maxops | 0.625 | **0.94** |

So reachability is **not** a "tools hurt" case — it now *supports* C1 (tools help decisively), and the
episode is a small win for the harness: the tool arms' disagreement with `raw` flushed out a ground-truth
bug. *(raw-maxops reachability n = 31: one trial returned an empty answer.)*

---

## Per-kind table (pooled boards + seeds, post-correction)

| kind | raw | raw-maxops | roster-maxops |
|---|---|---|---|
| region-count | 18/32 = 0.56 | 32/32 = 1.00 | 31/32 = 0.97 |
| reachability | 20/32 = 0.62 | 29/31 = 0.94 ¹ | 30/32 = 0.94 |
| nearest-owned | 13/16 = 0.81 | 15/16 = 0.94 | 15/16 = 0.94 |
| reachable-nearest | 6/16 = 0.38 | 16/16 = 1.00 | 15/16 = 0.94 |
| city-defense | 23/32 = 0.72 | 30/32 = 0.94 ² | 32/32 = 1.00 ² |
| constraint-site | 27/32 = 0.84 | 29/32 = 0.91 | 32/32 = 1.00 |
| compare-two-attacks | 25/32 = 0.78 | 30/32 = 0.94 | 30/32 = 0.94 |
| t3-retreat | 25/32 = 0.78 | 32/32 = 1.00 | 32/32 = 1.00 |
| triage-reinforce | 24/32 = 0.75 | 32/32 = 1.00 | 31/32 = 0.97 |
| forward-posting | 24/32 = 0.75 | 28/32 = 0.88 | 28/32 = 0.88 |
| ~~cf-vacate~~ | ~~16/30 = 0.53~~ | ~~7/30 = 0.23~~ | ~~5/30 = 0.17~~ (excluded — degenerate) |

¹ one raw-maxops reachability trial returned an empty answer (excluded from its denominator).
² `city-defense` re-scored through the fixed answer-extractor (correction #2): raw-maxops 27/32→30/32,
roster 31/32→32/32 (+3 / +1 wrong→correct, 0 regressions). raw unchanged at 23/32. These were verbose
"City A … is better defended … versus City B" answers where the old scorer recorded the fogged loser.

## Threats to validity / caveats

- **Single model.** DeepSeek only; C5 (Gemini) unrun. cf-vacate's tool-trap especially may be model-specific
  (tool-engagement disposition, [[deepseek-gemini-tool-engagement]]) — a Gemini replication is the cleanest test.
- **n = 16/kind/arm** pooled → per-kind CIs are wide (~±0.15). Kind-level claims are directional; the
  aggregate and the two headline contrasts (C1, C2) are the powered results.
- **cf-vacate excluded** as corpus-degenerate (re-measured standalone). **reachability corrected + re-scored**
  post-run; its generator is now yes/no-skewed after removing ocean-crossing (6 yes / 26 no) — a *future*
  fresh run may want to rebalance the goal band, but this does not affect the validity of the re-score.
- Noise floor ~12–18%/item; any sub-0.05 accuracy claim needs replication (holds here — C1/C2 gaps are far
  above it).

## Takeaways for the blog

- **Headline #1 (C1):** tools are a *compute* lever — a fogged-board decision, *and even perception*, needs
  the operator, not a bigger prompt. Cleanly measured, tight CIs; strengthened after the reachability fix.
- **Headline #2 (C2):** *how* you deliver perception is a *cost* lever — a query loop (roster + fetch)
  strictly dominates front-loading the board, 2.5× overall and 3.7× at scale. The strongest single figure.
- **The sharp sidebar (cf-vacate):** tools can *hurt* — a plausibly-named current-state tool imposes an
  answer-value prior on a counterfactual question. The tool–task-fit thesis stated in the negative.
- **A methods aside (reachability):** the tool arms disagreeing with `raw` caught a bug in our own oracle —
  differential agreement across surfaces is a cheap ground-truth check.

**Related:** [[project-direction-maximal-calculator]] · [[fidelity-merge-bar]] · [[civspatial-publication-push]]

**Sources:** `results-3arm-2026-08-18-{mid,late}-{s1,s2}-deepseek.jsonl` (reachability re-scored under the
corrected oracle `521c0a1`; `city-defense` re-scored through the fixed answer-extractor via `civ rescore`
over the recorded `traces/`); spec `analysis/run-preregistration-3arm-2026-08-17.md` §12.
