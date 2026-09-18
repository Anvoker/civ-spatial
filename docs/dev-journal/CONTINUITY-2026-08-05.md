# CivSpatial — Continuity / Handoff, 2026-08-05 (end of day)

Read this first, then `RESUME.md` (evergreen state + commands). Supersedes `CONTINUITY-2026-08-04.md`.

> **Headline — the two PENDING items from 2026-08-04 are DONE and merged (three-state fog + viewer polish),
> and the day pivoted to a new conceptual track: a REASONING-FRONTIER question line — kinds where no simple
> tool solves them at 100%. First such kind (P4) is built + green (unmerged). Three feature branches await a
> single clean civ-eval integration tomorrow; the multi-board corpus sweep is running to completion.**
> master = `048ef7a`. Everything below is MERGED unless marked PENDING/UNMERGED.

## Arc of the day
Continued from 2026-08-04's "get maximally ready for the multi-board experiment." Closed the two fog/viewer
loops, then the user opened a new, higher-value thread: **design question kinds that resist tooling** (the
natural next chapter of the tool–task-fit thesis). Heavy design iteration with owner feedback, plus the first
implementation (P4) and the corpus generation run.

## MERGED TO MASTER today (all green — `just check` + verify-oracle PASS)
1. **Viewer polish** (`viewer-polish`, ff to `a833b99`): owner labels by **nation** (fixes the confusing
   "Unassigned5" → "Uyghur"; those are AI slots whose save leader-name defaulted to "Unassigned"); a
   collapsible **full-prompt / system panel**; **narrower board + wider Answer** layout; owner-color
   collision fix (stable-index palette + legend + bold rings); **fog on/off toggle** (export carries both
   fogged + `unfogged`); numbered-marker legend; **unfogged default sample** (no more dead-barbarian "you");
   **question-box `\n\n` trim**.
2. **Three-state fog** (`fog-three-state` merge `14ab55b`): faithful `classic` fog (Unexplored/Fogged/Visible)
   with **real per-unit `vision_radius_sq`** read from the local install — land **2**, aircraft/fast-warships/
   Spy/Leader **8**, AWACS **26**, Explorer **2**, **city 5** — **squared-Euclidean** sight (not Chebyshev).
   **Additive & gated:** `Board.visibility=None` is byte-identical to before; three-state only activates under
   `--fog`, so the **non-fog baseline is unchanged**. Fogged tiles keep terrain/owner + enemy city (last-known)
   but hide live enemy units; one masked Board feeds solvers + encoders → parity is structural. Also shipped:
   the **`rulesetdir=="classic"` gate** (`--allow-ruleset`) and a **fog-perspective guardrail** rejecting
   dead/barbarian/pirate/animal `--fog` targets (`--allow-any-fog-player`). Tech-gated Fortress/mountain vision
   deliberately NOT modeled (Invention not decoded). verify-oracle green fogged+unfogged, both boards/tiers;
   new `fog_parity` test.
3. **Docs**: `analysis/findings-multiboard-corpus.md` (`bf12184`); **`QUESTION-CATALOGUE.md`** (every current
   question kind + coverage/gaps); the **reasoning-frontier** design docs (see next section).

## The REASONING-FRONTIER design track (today's main conceptual output)
Goal: question kinds where **no simple deterministic tool solves them at 100%** — so tooling can't trivially
win (global aggregates etc. are DISQUALIFIED — a counter nails them). Docs: `analysis/reasoning-frontier-questions.md`
(v1 proposals) + `analysis/reasoning-frontier-questions-v2.md` (**v2 revision + v2.6 addendum**, the current one).
- **Three engines of tool-resistance:** (a) **hidden information** (answer isn't a function of the board the
  model is shown — unlocked by three-state fog; the generator holds the UNMASKED board to score); (b)
  **underdetermined objective** (score a necessary condition, Pareto non-dominance); (c) intractable+partial-verify (weak).
- **Luck/tool-resistance duality** (the load-bearing result): under fog, tool-resistance ⟺ hidden-content
  dependence ⟺ irreducible scoring luck. So "done right" = three disciplines: **provably-safe/reasoning-rejectable
  decoys** (a luck-free credit floor) + **decisive, grounded true answers** + **report lift-over-baseline at
  large N with a CI** (never raw accuracy). This is the honest answer to the owner's hindsight-scoring worry.
- **Verdicts after two owner-feedback rounds:**
  - **P4 Robust forward posting** — CHAMPION, **BUILT** (see below). Cheapest (engine b, no pipeline).
  - **P1 Surprise-strike exposure** — CHAMPION, buildable now (roster disclosure handles the latent speed-leak).
  - **P7-fogged garrison assault** — standing rose; **cleanest snapshot-honest hidden-info kind** (static
    garrison escapes both the speed-leak and turn-history critiques).
  - **P3 Hidden-force localization** — CHAMPION but **history-hungry**.
  - **N1 Exposed-maneuver** — **DEFERRED** (no honest snapshot grounding for a single-turn strike; reframed to a
    selection, not a probability; blocked until the corpus carries turn-history).
  - **P5 Contested land grab** — MODIFIED (denial axis + feasibility gate; drop if redundant with settle-site).
  - **P2, P6 — DROPPED** (tool-trivial core / duality-forbidden).
- **Recommended build order:** P4 (done) → **P1 + P7-fogged** (snapshot-honest hidden-info pair; P1 needs the
  unmasked-board pipeline seam) → P3 (wants history) → N1 (waits for turn-window corpus). P5/P7 conditional.

## New constraints/decisions captured today
- **Experiment will be run FOGGED** (owner chose) → hence the perspective↔player coherence build.
- **Perspective↔player coherence:** for player-relative kinds the fog perspective MUST equal the question's
  `player` (and sight is computed around that player). Being built (see PENDING).
- **Turn-history requirement (owner directive):** future corpus saves must include the ~3 turns BEFORE the
  presented turn (maybe `saveturns 1` / every turn) so history-dependent reasoning is possible. The CURRENT
  sweep uses `saveturns 20` (no history window) — fine for P4 + existing kinds, but a future regen is needed
  for N1 (and strengthens P3). Cheap — reproducible from the seeded `.serv` scripts.
- **Board-support reality (confirmed):** whole boards can legitimately yield ZERO instances for a kind
  (generators drop non-decisive instances; e.g. cf-vacate → 0 under a blind fog perspective). The run matrix
  must guarantee enough *supporting* boards per kind; narrowest-window kinds need the widest sweep; keep
  per-kind×board yield telemetry.

## ⚠️ PENDING — three unmerged branches → one clean civ-eval integration tomorrow
**master = `048ef7a`. Integration ORDER matters (coherence changes the generator signature; P4 adds a kind):**
1. **`feat-fog-perspective-coherence`** (worktree `agent-a9ec57ce13d8796ce`) — **IMPLEMENTED but NOT COMMITTED**
   (branch still at base `bf12184`; the agent was on a slow T677 verify-oracle run and hadn't committed at EOD).
   Threads `perspective: Option<&str>` through `generate_questions`→`QuestionKind::generate`, restricting the 7
   player-relative kinds to the fog player; wires CLI (`fog_perspective_name`) + `viewer-export`; adds
   `civ-core/tests/fog_coherence.rs`; updates design §9 to AS-BUILT. **TOMORROW:** check if it committed +
   passed; if not, the changes are in the worktree working dir — commit them, run `just check`, then MERGE
   FIRST (it's the broad signature change).
2. **`feat-p4-forward-posting`** (worktree `agent-a0c077b78bfce5aa6`, commit **`8618a7a`**) — DONE + green.
   New T3 kind `forward-posting`: pressure (Σ enemy `att_eff` + enemy-city `city_capture_prob` in attack reach)
   × survival (1 − max over enemy **1-turn repositions** of `win_probability` — the novel enemy-moves-to-exploit
   search); Pareto `ChoiceSet`; `POSTING RULES` block; 2 oracle tests; verify-oracle PASS all 4 gate commands.
   **TOMORROW:** merge AFTER coherence, **threading `perspective` into `forward-posting`** (it's player-relative)
   + resolve the civ-eval dispatch conflict; `just check` + verify-oracle green. (Note: pre-existing `cargo fmt`
   diffs across untouched files from an old rustfmt version — not P4's; `just check` doesn't run fmt.)
3. **`corpus-selfplay-tooling`** (worktree `agent-a28f9b25f4fe0df32`, commit **`0de16fd`**) — tooling done +
   smoke-verified vs Freeciv 3.2.5. **The 18-game SWEEP is RUNNING in the background** (job `bk9tqp9k1`, saves
   for all 18 configs already present as of EOD → near or at completion). On completion the corpus agent (told
   to stay quiet until done) builds the manifest + **selects 5 boards/band from distinct games** (early ≤60 /
   mid 61–140 / late >140; drop steamrolls/over-railed) + commits. **TOMORROW:** check it finished, review the
   chosen 15, then merge (scripts-only — no civ-eval conflict). Prune all three worktrees after merge.

## Background jobs left running overnight (will commit to their branches; no cost, local)
- **Corpus sweep** (`bk9tqp9k1`) — 18 games → T220, then manifest + selection commit.
- **Coherence build agent** — finishing its verify-oracle run, then commits `feat-fog-perspective-coherence`.

## Housekeeping done today
Freeciv 3.2.5 installed (path `C:\Program Files\Freeciv-3.2.5-win64-10-client-gtk3.22\`). Pruned the old fog-design
worktree + the stale `agent-a599176…` worktree (scratch scripts backed up to the session scratchpad). Ideation
worktrees pruned after their docs landed.

## The BLOG PLAN — still the payoff (unchanged from 2026-08-04)
Claims #2/#3/#4 (maxops-enum vs maxops; **raw-maxops-enum vs interactive-maxops-enum = HEADLINE**; clean raw vs
interactive). Prereqs: **finalize the kind roster + pre-register the run matrix**; **OpenRouter budget top-up
(USER action, still pending)**; **multi-board corpus** (sweep in progress). The reasoning-frontier kinds are a
NEW complementary chapter — "what reasoning survives even good tools."

## START HERE TOMORROW
1. **Integrate the three branches** in order (coherence → P4-with-perspective → corpus), each behind `just check`
   + verify-oracle + remote clippy; prune worktrees.
2. Then: **finalize kind roster + pre-register the matrix** (fogged, per the decision), and — once the corpus
   selection lands — characterize the 15 boards. Budget top-up gates the actual LLM run.
3. Reasoning-frontier next builds: **P1 + P7-fogged** (pay the unmasked-board pipeline seam once for the
   hidden-force family). A **history-window corpus regen** (`saveturns 1`) unlocks N1 and strengthens P3.
