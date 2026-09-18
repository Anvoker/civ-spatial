# CivSpatial — Continuity / Handoff, 2026-07-31 (end of day)

Read this first, then `RESUME.md` (evergreen state + commands), `analysis/findings-calc.md` (calculator +
the new **§Scale test**), `analysis/findings-crossmodel-thinkon.md`, `maximal-calculator-plan.md` (on the
unmerged maxops branch), and `findings-fog.md` (the arc this builds on). Supersedes the earlier same-day draft.

> **Headline — big day.** The basic calculator **confirmed the dispersed-integration deficit is ARITHMETIC on
> small boards** (`raw-ops` 0.986) — **but the scale test shows it does NOT hold on a large known map** (drops
> to 0.736 at 98% explored, ties `interactive-ops` 0.750). Shipped + **merged** the 4 decision-corpus kinds and
> the gen-questions overflow fix. **Built (NOT merged)** the maximal calculator. Finished **cloud validity**
> (think-ON cross-model + frontier anchor). **Langfuse is live** with traces loaded, plus an adapter timestamp fix.

## Findings (today)
1. **Calculator, small map** (`findings-calc.md`): DeepSeek think-off, T677/p4 (6.8% explored), 3 dispersed
   kinds, 3 reps, n=72/arm. `raw-ops` (full board + calculator) = **0.986**, above raw think-ON (0.875) and raw
   baseline (0.792), with **0 wrong answers** (raw baseline had 15). Pays off **only with full-board perception**
   — the same operators on the query loop did nothing (`interactive-ops` 0.806 ≈ `interactive` 0.819). The
   deficit was arithmetic, not spatial reasoning.
2. **Scale test — NEW, the key correction** (`findings-calc.md` §Scale test): re-ran the two ops arms on T677/p2
   (**98% explored, large dense known map**), 3 reps. **`raw-ops` drops 0.986 → 0.736; `interactive-ops` 0.806 →
   0.750 → they TIE.** The single-rep "inversion" (0.875 vs 0.708) was noise. Per-kind split: nearest-owned
   favors interactive-ops (18/24 vs 12/24), reachable-nearest favors raw-ops (18/24 vs 12/24), region-count tied.
   **Corrected thesis:** the calculator kills the *arithmetic* bottleneck on small boards, but at scale a *second*
   bottleneck (locating what to measure in a huge board) re-emerges and caps **both** ops arms at ~0.75.
3. **Cross-model think-ON + frontier** (`findings-crossmodel-thinkon.md`): think-ON did NOT equalize in-loop
   effort — Gemini still bails (3.4 turns/2.8 calls) vs DeepSeek (16.1/64.3, ~23×). The interactive gap is a
   durable **engagement asymmetry**, not a think-flag artifact; reasoning rescues interactive only where the model
   works the loop. Frontier anchor GPT-5.6-sol raw think-off = **0.771** — same 0.77–0.81 band as DeepSeek/Gemini
   → the dispersed deficit is **not** capability-tier. (Memory: `deepseek-gemini-tool-engagement` — blog candidate.)

## Shipped & MERGED to master today
- `f7a1372` + merge `5cb46ee` — 4 decision-corpus kinds (cf-vacate, adv-assault-target, triage-reinforce,
  compare-two-attacks) + `Answer::Compare3`; Oracle-100% both boards/tiers; tested green on master.
- `7735490` — docs: calculator + cross-model/frontier findings + this handoff.
- `d65c0b6` — Langfuse turnkey (up/down scripts, just recipes, README quickstart); `b3b1557` — persisted trace
  scripts + frontier-batch runner + run logs; `fb2ff41` — **adapter timeline fix** (traces land at import time,
  visible in the default view; `--base-time` for a fixed base).
- `b94d6e9` + merge `1330b49` — gen-questions subtract-overflow fix.
- (This commit — scale findings appended to `findings-calc.md` + this continuity + RESUME refresh.)

## Built but NOT merged — awaiting your decision
- **Maximal calculator**: branch `worktree-agent-ae7abce34c0372381` (commit `59da8bc`). Adds `raw-maxops` /
  `interactive-maxops` (full measurement operator set — att_eff/def_eff/city_defense/garrison_defense/threat/
  site_axes/reach_turns, each reusing the exact `rules.rs` fn), all *selection* operators withheld; plan doc
  `maximal-calculator-plan.md` with a per-kind survival table. `just check` green, `civ-core` untouched.
  **Caution from the scale finding:** `raw-maxops` (full board + more operators) likely hits the same ~0.75
  ceiling — worth a live test before betting on it as the primary arm.

## Langfuse (live)
- `bash observability/langfuse-up.sh` / `langfuse-down.sh` (add `--volumes` to wipe). UI: http://localhost:3000.
- Keys in `.env` (gitignored). Push traces: `set -a; source .env; set +a; python observability/trace_to_otel.py
  <traces-dir>` (defaults to the local Langfuse OTLP endpoint).
- Loaded: the scale scout + the over-fetch tracediag traces (dated today after the fix). Some duplicate
  2020-dated copies linger from diagnosis — harmless, hidden by the default time filter.

## Pending / next steps (prioritized)
1. **Failure analysis of the ~0.75 scale ceiling (NOT started).** Analyze the 98% traces (both ops arms, all 3
   reps in `traces/results-calc-scale-p2*/`): where does each fail — wrong-coordinate selection, entity-not-found,
   mis-aggregation, early bail, turn cap? The per-kind split (nearest-owned↔interactive, reachable-nearest↔raw)
   is the clue. THE key to understanding the ceiling; would complete the scale writeup.
2. **Maxops merge decision** + a live maxops run: does the maximal operator set beat basic ops, and does
   `raw-maxops` hit the same scale ceiling?
3. **Live-evaluate the decision-corpus kinds** (cf-vacate etc.) under the calculator — the actual
   maximal-calculator experiment (the whole point of the pivot).
4. **`constraint-site` is NOT built** despite the corpus doc listing it "built" — clean follow-up (spec in
   `maximal-calculator-plan.md`).
5. **`t3-threat`** is the thinnest survivor under maximal ops — watch for saturation; if so, retire the
   unique-dominator framing and keep the frontier `ChoiceSet` forms (adv-assault-target, settle-site).

## Budget / ops
- Session cloud spend ≈ **$3.2** (cross-model ~$2 + scale scout $0.41 + reps 2-3 $0.82). Budget is getting thin.
- Rebuild remote before cloud runs (`cargo build -p civ-cli --features remote`). Source `.env`, never key on CLI.
- Isolate builds/arms in git worktrees. Noise floor ~12–18%/item → pool/vote ≥3 reps for sub-0.05 claims (the
  scale test's rep1 inversion is the cautionary example).

## Process note (honest correction)
I referred to a "failure-analysis agent running" in earlier messages — it was **never actually launched** (I got
diverted mid-turn by the Langfuse fix). It is pending task #1 above, not in flight.

## Housekeeping
- Merged worktrees cleaned (decision-corpus, bug-fix). The bug-fix worktree *directory* is OS-locked (git no
  longer tracks it; clears on reboot). The **maxops worktree is still present** (unmerged, intentionally).
