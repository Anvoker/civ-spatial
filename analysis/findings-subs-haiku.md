# Second-model arm — Haiku 4.5 on the Max subscription (exploratory, majority-voted)

**Purpose:** a second-model replication of the 3-arm result (`raw` vs `raw-maxops` vs
`roster-maxops`) to test whether C1 (tools = compute lever) and C2 (roster dominates on cost)
generalize past DeepSeek, or are a single-model quirk. Model: **Claude Haiku 4.5** (`claude-haiku-4-5`,
think-off), run through the **Claude Max subscription** via CLIProxyAPI. Created 2026-09-15.

> **Cite with disclosure.** This is an **exploratory** arm, not a clean citable number, for two
> reasons baked into the subscription path: (1) it runs under a **hidden Claude Code system prompt**
> we don't control, and (2) it is **non-deterministic at temp 0** (~64–73% answer reproducibility).
> The clean, deterministic, citable arms remain on OpenRouter (DeepSeek; Gemini if run). Haiku's
> value here is *directional*: does the same effect appear on a different model family? It does.

---

## Parity with the DeepSeek run — confirmed (same experiment, not a thinner one)

Diffed against `results-3arm-2026-08-18-*.jsonl`:

| axis | DeepSeek run | Haiku run |
|---|---|---|
| arms | raw · raw-maxops · roster (interactive-maxops) | **same 3** |
| boards | mid `corpus_medium_a3_s1337` fog p1 · late `corpus_large_a6_s42` fog p4 | **same 2** |
| kinds | 11 | **same 11** |
| params | `--per-kind 8 --difficulty hard --think off`, 2 seeds | same |
| **distinct items** | 906 arm-items | **909 arm-items — matches cell-for-cell** (city-defense 32/32, constraint-site 32/32, compare-two-attacks 31/31, region-count 32/32, movement 14/15, cf-vacate 18–19) |

The Haiku run asked the **same questions on the same boards**. The one difference is **duplication,
not coverage**: the subscription path is non-deterministic and had overlapping invocations, so the
**tool arms were re-attempted several times per item** (1186 raw rows → 909 distinct; DeepSeek was
near-clean at 954 → 906). Attempts-per-item: 644×1, 255×2, 8×3, 2×4. This is a scoring nuance
(dedup), not a thinness problem.

---

## Provenance — the scored data is post-fix (verified, not assumed)

The run happened in two slices; only the second is the real run:

- **08-24 (11 of 13 trace dirs, 07:31–08:58)** — the run. This post-dates **every** harness fix,
  including the last trajectory-shaping one, `c0b8cd6` "message-level cache_control for maxops on the
  native-Anthropic path" (08-20 18:49) — the fix that made the tool arms work on the subscription at
  all (they 400'd before it). Earlier harness fixes (stacking, cache-deduct budget, resume-key,
  prompt text) landed 08-12; the reachability-oracle and cf-vacate-sampler fixes landed 08-20
  afternoon.
- **08-20 morning (2 dirs, 11:59–12:01)** — preflight smokes only: 6 traces each, an **old kind set**
  (`terrain`, `nearest` — not in the roster), raw + raw-maxops only, and 2 maxops `error` rows (the
  pre-`c0b8cd6` 400s). Their **only** overlap with the analysis roster is 4 `region-count` rows.

**Excluding the two preflight dirs changes no number** (verified: overall and every per-kind cell
identical to the last decimal) — the same `region-count` items were re-run on 08-24, and the
majority-vote pools them by item_id. Independently, all 1198 rescored rows are **`faithful`**
(regenerated canonical answer == recorded `expected`), which also confirms the reachability items were
generated under the **fixed** oracle (a pre-fix item would surface as non-faithful; none did). So both
the trajectories (08-24 hardened harness) and the scores (re-scored through `4df3033`) are post-fix.

## Method

1. **Re-scored through the fixed extractor.** Every trace's final completion was re-scored offline via
   `civ rescore --traces <dir>` over all 13 Haiku trace directories (the shipping Rust scorer +
   regenerated acceptable-sets; no model calls). All 1198 rows are **faithful** (regenerated canonical
   answer == recorded `expected`, so ground truth didn't drift). The fix recovered **41 rows
   (24 wrong→correct + 17 invalid→correct), zero regressions** — Haiku writes markdown/bare coords
   (`**83, 39**`, `88,27`) and verbose two-city answers that the pre-fix extractor mis-scored. This is
   the same class of correction applied to the published DeepSeek numbers, so both use the same scorer.
2. **Majority vote over attempts.** For each (arm, board, item), attempts with a non-empty completion
   were majority-voted on correctness (`error`/empty attempts excluded from the denominator).
3. **Ties.** With mostly 1–2 attempts, **60 of 305 items are exact 50/50 ties** (a single re-attempt
   disagreed), concentrated in the tool arms (which got the duplicate re-runs). Ties have no defined
   majority at n=2, so per-arm accuracy is reported as a **band**: `[ties→wrong, ties→correct]`. The
   per-kind table below uses ties→correct ("correct in ≥ half of attempts"); the overall gives both.
4. **cf-vacate excluded** from the aggregate (corpus-degenerate on both frozen boards, same as
   DeepSeek), reported separately.

---

## Per-kind accuracy (Haiku majority-vote, ties→correct) vs DeepSeek (published post-correction)

| kind | Haiku raw | Haiku rmax | Haiku roster | DS raw | DS rmax | DS roster |
|---|---|---|---|---|---|---|
| region-count | 0.56 (18/32) | 1.00 (32/32) | 0.91 (29/32) | 0.56 | 1.00 | 0.97 |
| reachability | 0.59 (19/32) | 0.88 (28/32) | 0.91 (29/32) | 0.62 | 0.94 | 0.94 |
| nearest-owned | 0.71 (10/14) | 0.57 (8/14) | 0.93 (13/14) | 0.81 | 0.94 | 0.94 |
| reachable-nearest | 0.47 (7/15) | 1.00 (15/15) | 0.93 (14/15) | 0.38 | 1.00 | 0.94 |
| city-defense | 0.94 (30/32) | 0.97 (31/32) | 1.00 (32/32) | 0.72 | 0.94 | 1.00 |
| constraint-site | 0.84 (27/32) | 0.69 (22/32) | 0.69 (22/32) | 0.84 | 0.91 | 1.00 |
| compare-two-attacks | 0.68 (21/31) | 0.81 (25/31) | 0.90 (28/31) | 0.78 | 0.94 | 0.94 |
| t3-retreat | 0.78 (25/32) | 0.81 (26/32) | 0.84 (27/32) | 0.78 | 1.00 | 1.00 |
| triage-reinforce | 0.88 (28/32) | 1.00 (32/32) | 1.00 (32/32) | 0.75 | 1.00 | 0.97 |
| forward-posting | 0.78 (25/32) | 0.94 (29/31) | 0.91 (29/32) | 0.75 | 0.88 | 0.88 |
| *cf-vacate (excluded)* | *0.79 (15/19)* | *0.74 (14/19)* | *0.32 (6/19)* | *0.53* | *0.23* | *0.17* |

**Overall, excl cf-vacate:**

| arm | Haiku (band) | DeepSeek |
|---|---|---|
| raw | 0.71 – 0.74 | 0.712 |
| raw-maxops | 0.84 – 0.88 | 0.948 |
| roster-maxops | 0.82 – 0.90 | 0.958 |

---

## What replicates

- **C1 — tools are a compute lever ✅ (directionally).** Haiku raw ≈ 0.72 lifts to ≈ 0.84–0.90 with
  the operator menu — a **+12 to +16 pt** tool gap (band from the tie rule). Same sign and same broad
  magnitude class as DeepSeek's +24, on a different model family. Raw's *perception* weakness
  replicates too (region-count 0.56, reachability 0.59 → tools rescue both).
- **C2 — roster dominates on cost ✅ (cleanly).** Tokens/answer: roster **88k** vs raw-maxops **193k**
  (2.2× overall). By board: mid 0.94× (roster ~even), **late 3.15×** (92k vs 290k) — the identical
  "raw-maxops re-sends the whole board every turn, so cost explodes with board size" mechanism
  DeepSeek showed (3.72× at scale). This is the strongest cross-model result: the cost frontier is not
  model-specific.
- **The cf-vacate tool-trap ✅.** Tools *hurt* on cf-vacate for Haiku too (roster 0.32 < raw 0.79) —
  cross-model evidence that the current-state `city_fall_prob` tool imposes a wrong prior on the
  post-vacate counterfactual. Not a DeepSeek artifact.

## Where Haiku differs — honest wrinkles

- **Smaller, noisier lift.** +12–16 pts vs DeepSeek's +24. Two causes: Haiku is a smaller model, and
  the subscription non-determinism gives the tie-band (60/305 items).
- **Under-exploration on search-heavy kinds** — the lazier agentic disposition
  ([[deepseek-gemini-tool-engagement]]) shows up as *accuracy*, not just cost. On **constraint-site**
  the tool arms *drop* to 0.69 (DeepSeek saturates 0.91–1.00) and on **nearest-owned** raw-maxops
  (0.57) is *below* raw (0.71) — Haiku makes ~15 tool calls where DeepSeek makes ~100, so it quits the
  region search early. The tools are capable; Haiku just doesn't drive them hard enough.
- **Haiku eyeballs city-defense better** (raw 0.94 vs DeepSeek 0.72) — a small model-specific perception
  quirk, not load-bearing.

## How to cite in the blog

> *"We replicated the headline on a second model, Claude Haiku 4.5, run through a Claude Max
> subscription. Because that path is non-deterministic and runs under a system prompt we don't control,
> we treat it as exploratory rather than a clean number — we re-ran items, re-scored every completion
> through the same fixed extractor, and majority-voted, reporting a band where 50/50 re-attempts make
> the vote ambiguous. The two load-bearing results survive the model change: tools lift accuracy
> (+12–16 pts here, +24 on DeepSeek) and the roster query-loop dominates full front-loading on cost
> (2.2× overall, 3.15× on the large board). The one place they diverge is instructive — Haiku
> under-drives the tools on the search-heavy kinds, so its tool-lift is smaller: the ceiling is set by
> how hard a model works the operators, not by whether the operators exist."*

**Sources:** `results-subs-arm-{mid,late}-{s1,s2}-claude_haiku_4_5_20251001.jsonl` +
`traces/results-subs-arm-*/` (13 dirs), re-scored via `civ rescore`. Compare:
`analysis/findings-3arm-clean-run.md` (the DeepSeek headline). Related: [[deepseek-gemini-tool-engagement]]
· [[civspatial-publication-push]].

---

## Re-run (temperature check) — 3 fresh passes, 2026-09-16

The first Haiku run had uneven attempts per item, so its majority vote had a wide tie-band (60/305
items at 50/50). To fix that and measure the subscription path's non-determinism directly, the full
matrix was re-run as **3 fully-independent passes** (`results-subs-arm-rerun-p1/p2/p3-*`), cf-vacate
excluded, everything else identical (2 boards, 2 seeds, per-kind 8, all 3 arms, think-off). Every item
gets exactly 3 fresh attempts → a clean majority vote with no ties. (The subscription's own session
limit interrupted the run twice; errored rows were stripped and re-run on reset until all 12 files were
0-error. 36 trace dirs re-scored through `civ rescore`; stale error-traces rescore to `error` and are
excluded.)

**Overall (10 kinds, 3-pass majority):**

| arm | re-run (3-pass) | Haiku #1 | DeepSeek |
|---|---|---|---|
| raw | **0.715** (203/284) | 0.739 | 0.712 |
| raw-maxops | **0.880** (250/284) | 0.876 | 0.948 |
| roster-maxops | **0.912** (259/284) | 0.898 | 0.958 |

The tool gap firms up at **+16.5 pts (raw-maxops) / +19.7 pts (roster)** — slightly larger than run #1's
band, and the roster/raw-maxops ordering (roster ahead) holds. C1 confirmed.

**Per-kind (3-pass majority, raw / rmax / roster):**

| kind | re-run | Haiku #1 | DeepSeek |
|---|---|---|---|
| region-count | 0.56 / 1.00 / 0.94 | 0.56 / 1.00 / 0.91 | 0.56 / 1.00 / 0.97 |
| reachability | 0.59 / 0.84 / 0.94 | 0.59 / 0.88 / 0.91 | 0.62 / 0.94 / 0.94 |
| nearest-owned | 0.71 / 0.64 / 0.93 | 0.71 / 0.57 / 0.93 | 0.81 / 0.94 / 0.94 |
| reachable-nearest | 0.47 / 1.00 / 0.87 | 0.47 / 1.00 / 0.93 | 0.38 / 1.00 / 0.94 |
| city-defense | 0.94 / 1.00 / 0.97 | 0.94 / 0.97 / 1.00 | 0.72 / 0.94 / 1.00 |
| constraint-site | **0.62 / 0.50 / 0.75** | 0.84 / 0.69 / 0.69 | 0.84 / 0.91 / 1.00 |
| compare-two-attacks | 0.71 / 0.94 / 0.94 | 0.68 / 0.81 / 0.90 | 0.78 / 0.94 / 0.94 |
| t3-retreat | 0.78 / 0.91 / 0.88 | 0.78 / 0.81 / 0.84 | 0.78 / 1.00 / 1.00 |
| triage-reinforce | 0.84 / 1.00 / 1.00 | 0.88 / 1.00 / 1.00 | 0.75 / 1.00 / 0.97 |
| forward-posting | 0.78 / 0.91 / 0.91 | 0.78 / 0.94 / 0.91 | 0.75 / 0.88 / 0.88 |

**The temperature result (the point of the re-run).** Aggregate accuracy is *stable* across passes, but
individual items are decided differently run-to-run:

| arm | per-pass accuracy (p1/p2/p3) | 3-way agreement (all 3 passes agree per item) |
|---|---|---|
| raw | 0.676 / 0.690 / 0.718 | **0.694** |
| raw-maxops | 0.859 / 0.870 / 0.870 | **0.768** |
| roster-maxops | 0.894 / 0.859 / 0.884 | **0.796** |

So the **aggregate wobbles only ~1–4 pts** between identical passes, but **~20–31% of individual items
flip correctness** across the three runs (raw is the most stochastic; the tool arms are steadier but
still ~20–23% flip). The non-determinism is real at the item level and averages out at the aggregate —
which is exactly why the headline rests on the aggregate contrast and a majority vote, never a single
per-item score.

**Cost frontier (C2) reproduces:** tokens/answer raw 40k · raw-maxops 161k · roster 78k → **roster
advantage 2.08× pooled, 3.24× on the late board** (1.06× mid). Same shape as run #1 (2.2× / 3.15×) and
DeepSeek (2.5× / 3.72×).

**What moved >0.10 from run #1** — almost all in **constraint-site** (raw 0.84→0.62, rmax 0.69→0.50;
roster 0.69→0.75) and compare-two-attacks rmax (0.81→0.94). constraint-site is the search-heavy kind
where Haiku under-explores; on n=32 with a stochastic model its per-kind cell swings ~0.2 between runs.
This is the honest caveat of the whole arm: **trust the aggregate and the variance metric, not any
single per-kind Haiku cell.** Both load-bearing claims (C1 tool lift, C2 cost frontier) reproduce; the
divergence from DeepSeek (weaker tool arms, driven by under-exploration on search kinds) reproduces too.

**Re-run sources:** `results-subs-arm-rerun-p{1,2,3}-{mid,late}-{s1,s2}-*.jsonl` (12 files, 0-error) +
`traces/results-subs-arm-rerun-p*/` (36 dirs), re-scored via `civ rescore`.
