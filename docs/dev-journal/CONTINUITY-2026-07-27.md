# CivSpatial — Continuity / Handoff, 2026-07-27

Where we left off after a long, productive session. Read this first, then `RESUME.md` (evergreen
state + commands), `DESIGN.md` (design), `RELATED-WORK.md` (prior art + positioning), and the
`analysis/findings-*.md` results.

## Repo state
- **Branch `master`, clean, all green** (`just check`: tests + clippy `-D warnings` + Oracle-100%).
  Latest commit ~`e99ca41`.
- **Boards:** `data/saves/myagent_T50.sav` (78×52, the fixture) and now **`testcontroller_T677.sav`**
  (84×56, 10 players, 118 cities, 682 units — the dense headline board; parser handled it unchanged).
- **5 encoders:** raw, ascii, adjacency, egocentric, hierarchical (last two opt-in via `--encoding`).
  Every static encoder is compile-time-forced to implement `reconstruct` (round-trip completeness gate).
- **Live path:** cloud (https via ureq+rustls) and local (http). Flags: `--cache`, `--provider`,
  `--concurrency N` (cloud, ~5× faster), `--think on|off` (now portable across providers),
  `--reasoning-effort low|medium|high`, `--difficulty easy|hard`.

## What this session did (the arc)
1. **Built the cloud path** (ureq+rustls TLS), difficulty/cache flags, and a cache-aware **concurrent
   runner**. First honest full-board cloud runs.
2. **Findings** (`analysis/findings-deepseek-fullboard.md`, `findings-tier-sweep.md`):
   - Full board **broke the small-board ceiling**; encodings separate.
   - **`raw` strictly dominates `adjacency`** (≥ accuracy at ~7× fewer tokens).
   - **Reasoning collapses the encoding gap** — but *model-conditionally* (clean on DeepSeek; Gemini
     keeps its gap; and the reasoning toggle turned out non-portable → we fixed it).
   - **Best encoding is model-dependent** (cloud→raw, local Qwen→ascii). Replicated across DeepSeek,
     Gemini 2.5 Flash, Sonnet 4.5 with Wilson CIs.
3. **region-count is the wall.** Found+fixed a default-terrain confound (`generators.rs` excludes the
   board's most-common terrain). Later, `analysis/findings-count-curve.md`: the letter-counting
   **multiplicity cliff reproduces at the board level, and the encoding shifts where it starts** —
   `raw` holds ~2 counts longer than `ascii` (not a model confound) → mechanistic account of raw>ascii.
4. **T2 valuation** implemented (combat `rules` module, T2a unit-strength, T2b city-defense, City-Walls
   `improvements` decode). Fixed a scorer whitespace bug (coord spacing) that was false-flagging T2.
5. **Encoders + gate:** egocentric, hierarchical, and the **round-trip reconstruction gate** (which
   immediately caught a latent `ascii` territory-owner completeness gap).
6. **Literature scan → `RELATED-WORK.md`** (top 5 papers read in full). Sobering + useful: much of our
   thesis is *prior art* (cite as foundation). Closest twin = **Talk like a Graph** (freeze object,
   vary encoding, score vs computed truth; no universal winner). Novelty narrowed to the **intersection**:
   real game board + encoding breadth + model-dependence + cost frontier + perception isolate. SAGA
   (CivRealm) validates "information architecture is the bottleneck" through game score.
7. **Hierarchical encoder came up NULL at T50** (`findings-hierarchical.md`): dominated (tied accuracy,
   +27% tokens). Scale is the untested variable → **fetched T677** to test it.

## THE IMMEDIATE NEXT ACTION — the T677 scale-crossover (PAUSED on two decisions)
The pivotal experiment: on the dense T677 board, **does `raw`'s advantage over `adjacency` invert, and
does `hierarchical` finally pay off?** (SAGA/NLGraph/Lost-in-Aggregation all predict relational/chunked
wins at scale.) Proposed run: 4 encodings, seed 1, per-kind 12, hard, `--cache on --concurrency 8`.
T677 model-token board sizes: ascii ~59k · raw ~64k · hierarchical ~89k · **adjacency ~255k**. Est.
**~$1.2/run**, ~20–40 min (adjacency is the cost + latency driver). Cumulative would be ~$2 of the $5.

**Two open decisions the user flagged (2026-07-27) — resolve before running:**
1. **Adjacency cost is scary.** ~$0.62 of a run's ~$1.2 is adjacency alone (255k tok/call). Options:
   run it anyway (it's the crux of the crossover hypothesis), or drop it and test raw/ascii/hierarchical
   only (cheaper, but then we can't test the raw-vs-adjacency inversion), or run adjacency at reduced
   per-kind. **Leaning:** keep adjacency for the crossover specifically (it's the hypothesis), but stop
   including it in routine runs elsewhere.
2. **think-off vs think-on is a real methodological question, not just cost.** The count-curve showed
   reasoning *masks* encoding differences, so think-off gives the cleanest encoding-separation signal —
   BUT (user's point) **reasoning is arguably integral to how LLMs are actually used**, so a think-off
   result may not reflect real deployment. Framing to adopt: think-off measures *how much a good encoding
   can help a non-reasoning model* (an upper bound on the encoding effect); think-on measures *what
   survives when the model can also reason* (the deployment-relevant number). **Best answer is probably
   to report BOTH** (or use `--reasoning-effort low` as a middle), and be explicit which question each
   answers — not to pick one and call it *the* result. Decide the primary metric before the crossover.

## Backlog — other stuff we wanted to work on
- **Balanced count sweep** — turn the count-curve into a real measurement (multiple boards, balanced true
  counts 0–20+, fixed model, raw-vs-ascii head-to-head). Cheap, high-value, de-noises the cliff.
- **Scene-graph encoder** — SAGA-style entities+relations, but **info-complete (surface+backing)** and a
  **bucketed-vs-exact distance two-arm** test (prior art predicts bucketing helps relations, hurts counts).
- **Landmark-graph** (multi-anchor egocentric done right); then the **interactive/queryable tool surface**
  (needs a tool-loop runner + a `get_tile`-parity completeness gate — the big build; where "query in
  chunks" token savings actually live).
- **Factored labeling axis** (integer coords vs named tile IDs — Talk-like-a-Graph); **tokenization/
  delimiter ASCII ablation** (likely explains local-Qwen→ascii); **ground-truth-representation control**
  (Text2Space: hand a clean layout, measure the lift — proves failures are read, not reasoning);
  **Fine/Meso/Macro probe decomposition** for region-count (Lost-in-Aggregation).
- **T3 valuation** (the `ChoiceSet` dominance-scoring tier) — design approved in `T2-T3-design.md`, NOT
  yet built (T2 is done). Needs the `Answer::ChoiceSet` variant + a negative Oracle test.
- **Harder T1 tier** (distance/nearest/reachability still saturate at 12/12). Statistical rigor / more
  boards. Possible writeup positioned as *"Talk-like-a-Graph for real 2D spatial game state, with a
  perception isolate and a cost frontier."*

## Cost/ops notes
- Cache-read on DeepSeek bills ~**$0.028/M** (not the 0.1× I first assumed); a full-board run ~$0.5–0.65.
  Formats-testing budget is **$5**; ~$0.85 spent so far. Reasoning ~6.4× slower wall-clock, cheap in $.
- Reasoning toggle is provider-specific — our `--think` now emits OpenRouter's unified `reasoning` field
  for cloud, `reasoning_effort` for local. DeepSeek toggles cleanly; Gemini/Sonnet were inert before the fix.
- OpenRouter key lives in the user's terminal history (workspace-limited; user unconcerned).
