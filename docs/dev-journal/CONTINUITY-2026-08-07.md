# CivSpatial — Continuity / Handoff, 2026-08-07 (end of day)

Read this first, then `RESUME.md` (evergreen state + commands). Supersedes `CONTINUITY-2026-08-05.md`.

> **Headline — the fogged experiment is now REAL. Three branches were integrated; the fogged 15-board corpus
> was selected by a yield-driven optimizer; the kind roster was pre-registered (19 kinds); the reasoning-frontier
> HIDDEN-INFORMATION family (P7f + P1 + P3) was BUILT on a new unmasked-board seam; and the FIRST live DeepSeek
> smoke ran for $0.62 — showing the frontier kinds resist maximal tooling and interactive ≥ raw.**
> Everything below is MERGED to master unless marked. `just check` + verify-oracle green throughout.

## Arc of the day
1. **Integrated the three pending branches** (from 2026-08-05) in order, each green: `feat-fog-perspective-coherence`
   → `feat-p4-forward-posting` (threaded `perspective` into forward-posting during the merge) → `corpus-selfplay-tooling`.
   Pruned all three worktrees; deleted the merged branches. Preserved the 216 self-play saves into the main repo.
2. **Fog perspective + yield-driven corpus selection.** Built `scripts/corpus-yield.py` (picks a fog perspective per
   board = **largest sustained civ**; measures per-kind yield under fog) + `scripts/select-corpus.py` (greedy
   coverage optimizer). KEY FINDING: frontier-kind support is board-**idiosyncratic**, not band-driven — hand
   band-picking left settle-site 4/15, forward-posting 5/15, but those kinds are supported on 60 / 74 of 216 boards
   (missed, not rare). The optimizer lifted every discriminating kind to **10–15/15** while holding 15 distinct games
   + a balanced 5/5/5 size mix. Committed corpus: **`data/corpus/selection-optimized.txt`** (the pre-registered 15).
3. **Kind roster pre-registration** (`analysis/kind-roster-preregistration.md`) + a holistic kind analysis by an agent
   (`analysis/kind-roster-analysis.md`). Owner decisions: 16-in / 5-out (cut terrain/adjacency/direction/distance/
   nearest); K=8 default, t3-threat K=4, settle-site K=12; **no global-aggregation kind** (a counter nails it —
   owner); movement kinds early/mid only; **build P1+P3+P7f**. Roster now **FROZEN at 19 kinds**.
4. **Built the hidden-force family** (see next section) — the day's main engineering.
5. **First live smoke** on DeepSeek — $0.62, results below (`analysis/findings-smoke-nonsat.md`).

## The reasoning-frontier HIDDEN-INFORMATION family — BUILT + MERGED
The pipeline (§v2.3) so a question's answer can depend on fog-masked occupants the model cannot see. Two green
commits + a roster-freeze commit:
- **`6eb2b38` — unmasked-board seam (`GenCtx`) + P7f.** New `GenCtx { board (masked/what the model sees), unmasked
  (true board, Some only when fogged), rng, n, perspective }` threaded to all 21 kinds (existing kinds unchanged).
  CLI `load_board_pair` keeps the pre-mask board as ground truth. The Oracle is a prompt→stored-answer map, so an
  answer solved on the unmasked board + keyed by the masked prompt round-trips with **no oracle change**.
  **P7f (fogged-garrison assault, `Answer::Bool`)** — can my stack take a fog-garrisoned city? `rules::assault_capture_prob`
  (sequential storm, fallen-defender-exposes-the-next) on the UNMASKED garrison, decisive band across attacker
  orderings.
- **`c571c1c` — P1 + P3.** Both solved in the generator from BOTH boards (their answer needs unmasked attackers +
  masked fog). **P1 (surprise-strike, `Answer::Choice`)** — most-exposed of my cities vs an unseen striker; the
  candidate set carries a **provably-safe decoy** (no fogged tile in range → luck-free floor); exposure and the
  safe-check share one radius so soundness is by construction. **P3 (hidden-force, `Answer::Choice` over region
  centers)** — which fogged area hides the most massed enemy `att_eff`; provably-empty decoy region.
- **`161a578` — roster frozen at 19.** Measured fogged support on the 15-board corpus: **hidden-force 11/15**
  (abundant), **fogged-assault 12/15** (broad, thin per board), **surprise-strike 3/15** (narrowest — the design's
  predicted widest-sweep kind; bumped to K=12).
- **Two honest mid-build course-corrections** (both caught before commit): P7f's original "adjacent attacker" premise
  was self-contradictory (adjacency reveals the garrison under our vision model → yields 0) — switched to a stack
  *poised* at Chebyshev 2..=3 from a genuinely-fogged city, strike granted (magic placement, like
  compare-two-attacks/triage). P1's exposure and safe-radius were inconsistent (P1 never fired) — unified them.
- Tests: `civ-eval/tests/fog_hidden_force.rs` — synthetic-board tests that positively exercise all three in CI
  (hand-crafted fog; no dependence on the gitignored corpus). Leak invariant asserted.

## First live smoke — `analysis/findings-smoke-nonsat.md`
1 board (`medium_a6_s1337` fog 1), the **7 non-saturated kinds**, DeepSeek think-off, raw-maxops + interactive-maxops,
per-kind 8, cache on. **Cost $0.62** (88% cache-hit). Accuracy: **interactive 0.889 vs raw-maxops 0.774**.
- **Finding 1:** the frontier kinds are genuinely **non-saturated under maximal tooling** — forward-posting ~0.40 and
  fogged-assault 0.43–0.71 on BOTH surfaces; region-count/constraint-site/compare-two-attacks at ceiling. The set
  cleanly separates tool-resistant from tool-solved.
- **Finding 2:** interactive-maxops ≥ raw-maxops on **both accuracy AND cost** (claim-#3 axis) — but **n=7–8/kind,
  one board/seed → inside the noise floor. A direction to confirm, not a result.**
- Data: `results-smoke-nonsat.jsonl` (gitignored) + traces `traces/results-smoke-nonsat-*` (one JSON/question).

## ⚠️ START HERE TOMORROW (2026-08-08)
1. **Analyze today's smoke results in the REPLAY VIEWER** (`viewer/`) — walk the frontier-kind misses
   (forward-posting especially) question-by-question. The run traced full LLM I/O to
   `traces/results-smoke-nonsat-*`; export/point the viewer at that trace set (see `viewer/README.md`).
2. **Start an agent to analyze failure modes in PARALLEL** while you review — feed it `results-smoke-nonsat.jsonl` +
   the `traces/results-smoke-nonsat-*` JSONs, ask it to classify each frontier-kind miss: model reasoning error vs.
   a litigable decisive-band instance (a curation bug) vs. an extraction/`invalid` artifact. This decides whether the
   ~0.40 forward-posting score is a real model limitation or a generator issue.
3. Then: **replicate** the smoke across ≥3 boards × ≥3 seeds (~$2–4) to lift Findings 1–2 above the noise floor with
   Wilson CIs; and **pre-register the full run matrix** (encodings × models × think × reps). Budget top-up (USER
   action) still gates the full paid run.

## State / health
master green (`just check` = tests + clippy -D warnings + verify-oracle on T50/T677 × fogged/unfogged, all 4 PASS).
`.env` has the OpenRouter key (verified live today via `ping-model`). **Remember: `cargo build -p civ-cli --features
remote` before any cloud `run`** (plain `just check`/build strips the remote feature). The 216 self-play saves live in
`data/corpus/saves/` (gitignored); the pre-registered corpus is `data/corpus/selection-optimized.txt`.

## The BLOG PLAN — status nudged today
Claim #3 (raw-maxops-enum vs interactive-maxops-enum, HEADLINE) got its first data point today at the **non-enum**
level (interactive ≥ raw at full tooling); the frontier kinds are a NEW complementary chapter ("what reasoning
survives even good tools") that today's smoke gave first evidence for. Enumeration remains **efficiency-only**
(`findings-clean-ablation.md`) — add `-enum` surfaces for the cost-frontier story, not accuracy.
