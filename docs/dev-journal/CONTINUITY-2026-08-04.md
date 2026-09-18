# CivSpatial — Continuity / Handoff, 2026-08-04 (end of day)

Read this first, then `RESUME.md` (evergreen state + commands). Supersedes `CONTINUITY-2026-08-02.md`.

> **Headline — a full harness-fidelity hardening pass landed, then a replay VIEWER was built; the harness is
> feature-complete and green on master, BUT a newly-decided FOG fidelity change re-opens it before the experiment.**
> Everything below is MERGED TO MASTER unless marked PENDING.

## The arc of the day
Goal set at the start: get the project **"maximally ready"** for a **definitive multi-board robustness experiment**
(the ≥10-board run), because N=2 boards is the weakest external-validity link. That meant closing loops, auditing the
question corpus + tool parity + **solver faithfulness**, and fixing bugs — before spending on the big run. User chose
the ambitious path (go for the headline claims #2/#3/#4) and accepted schedule risk; deadline de-prioritized in favor
of a *defensible* result.

## MERGED TO MASTER today (all green — `just check` + `verify-oracle` PASS, 636 trials, 212 q × 3 enc)
1. **Parity ops** (`phase1-parity-fixes`, merge `e0e0bcc`): `def_eff_base` op (fixes unit-strength defense-axis tool
   giving terrain-inflated value); constraint-site predicate ops (`is_defensible`/`within_water_radius`/
   `not_enemy_territory`); t3-retreat `tile_cover` + `support` ops. All share the solver's `rules.rs` fns verbatim.
2. **Movement Dijkstra** (`movement-dijkstra`, merge `1e4930d`): replaced uniform land BFS with classic move costs —
   **rail free, road/river 1, terrain 3/6/9** (fragments, SINGLE_MOVE=3). Scoped to nearest-owned/reachable-nearest +
   their ops. **Divergence: 66% of nearest-owned and ~96% of reachable-nearest answers changed on T677** (old BFS
   scored rail-reachable tiles as unreachable — it was WRONG, not just simpler). `analysis/findings-movement-dijkstra.md`.
   Caveat: free rail makes ~41% of T677 nearest-owned instances trivial (0–1 turns) → **reach kinds want
   earlier-turn / less-railed boards** in the corpus.
3. **Combat win-probability** (`combat-odds`, merge `19e649a`): `win_probability` (HP/firepower race, p=A/(A+D) DP) +
   `win_prob` op, integrated into compare-two-attacks / cf-vacate / triage capture-state. **Divergence: 28.6% of
   compare-two-attacks answers flipped on T677** (a big strength ratio ≠ a good win chance once HP enters).
   `analysis/findings-combat-odds.md`. Deliberately did NOT convert t3-threat/adv-assault-target (would collapse their
   two-axis design) — handled next.
4. **Combat-kinds redesign** (`combat-kinds-finalize`, merge `b46c34d`): **t3-threat → single-axis "most likely to
   fall" argmax** (`city_capture_prob`, `THREAT_FALL_MARGIN=0.15`, Answer `ChoiceSet`→`Choice`); **adv-assault-target →
   two orthogonal axes {capture-win-prob, city size}** frontier (size-variation guard on the generator); **`city_fall_prob(x,y,player)`
   parity op**; **triage/compare-two-attacks honest magic-placement wording** (render-only, oracle unchanged).
   BONUS: **fixed near-broken generation** — t3-threat went from ~2/2800 kept to hitting its cap on T677.
5. **Tracer replay-completeness** (`tracer-replay-additions`, merge `469b6f3`): a per-run **`run.json` manifest**
   (seed/per_kind/difficulty/kinds/board/crop/fog/encodings/model/think — no secrets) + **`reasoning` capture** in
   `Reply`/`ChatReply`→trace (fixes a data-loss where a model returning content AND reasoning dropped the reasoning).
   Both serve the standing "persist all LLM I/O" directive.
6. **Docs correction** (`7df48ac`): killed the false "combat constants machine-extracted" claim — they're **hard-coded
   from `classic`** (extraction is a §8 TODO); flagged the **ruleset-mismatch hazard for multi-board harvesting**
   (a differently-ruled save silently mismatches → gate ruleset when collecting boards).
7. **Phase 1 REPLAY VIEWER** (`phase1-viewer`, merge `12fb015`): a new `viewer/` (vanilla TS + HTML5 canvas) + a Rust
   `civ-cli viewer-export` command (board+trace→JSON contract). Steps through questions, highlights referents/
   candidates, shows **model answer vs correct** + reasoning, interactive-trajectory stepper (lights up fetched tiles),
   zoom/pan, and **Freeciv `trident` GPL sprites** (inlined, self-contained, flat-color fallback + toggle). Loader is
   forward-compatible (uses `run.json` if present, else parses item_id). **Verified in a real browser** (screenshot in
   scratchpad): answer panel + sprites + zoom all working.

## Design/decisions recorded
- **Fidelity merge bar** (see memory `fidelity-merge-bar`): merge a fidelity/modeling upgrade only if it flips >10–15%
  of answers; else document as "tested, immaterial, kept simple." Movement (66/96%) and combat (28.6%) both cleared it.
- **Solver-faithfulness findings** (Phase-1 corpus review): combat is a deterministic scalar (now odds where it
  matters), movement was terrain-uniform (now road/rail), decision kinds use **magic placement** (labeled honestly,
  no reach-gating). Value proxy for adv-assault-target = **size only** (Palace/production deferred — not exposed/absent).
- **Renderer choice** (research doc): rejected freeciv-web (AGPL + coupled + poor overlay control); chose **our own
  renderer + borrowed GPL trident sprites** (Option B, web/TS canvas) over egui/WASM, for blog-embeddability.

## ⚠️ PENDING — not started, these re-open the harness before the experiment
1. **FOG must match the ACTUAL GAME = three-state fog (DECIDED, not built).** Today `mask_to_known` reveals every
   *ever-explored* tile IN FULL (terrain + live units + cities) — a v1 over-approximation (flagged at `board.rs:203`).
   Real Freeciv has THREE states: **unexplored** (Unknown/black) · **fogged** (explored, terrain remembered, but
   **live units hidden** & enemy cities shown last-known) · **visible** (full live). We collapsed fogged→visible, so
   the model currently **over-sees live enemy positions** on tiles it isn't watching. **Decision: implement real
   three-state fog.** Compute current visibility (sight radius around the player's OWN units/cities); on
   explored-but-unwatched tiles render terrain only. **CRITICAL: the SOLVERS must use the same visibility** (only
   currently-visible enemies count for threat/combat/assault) or model-view and ground-truth diverge. This is a
   **fog + solver** pass → scope as a design doc first, then implement, then RE-LOCK the harness. (Note: our `known`
   bit is correctly ever-explored, decoded from the save's `map_t`, `'u'`=unexplored — parser.rs:383-408. So the
   disjoint fogged maps the user saw are GENUINE never-explored gaps [captured/sea-settled cities + scouted enemies,
   6 owners visible], not a fog/unexplored merge. The real gap is the fogged↔visible collapse above.)
2. **Viewer polish iteration (PENDING).** (a) **Owner legibility**: cities/units are owner-colored only by a thin halo
   under sprites, and the hash-mod-10 palette **collides** — confirmed "Unassigned5" and "Testcontroller" both render
   `#e1bc29` yellow in the sample. Fix: assign colors by **stable index over owners present** (distinct up to palette),
   add an **owner legend** (swatch→player, flag the perspective player), bold owner ring even under sprites. (b) **Fog
   on/off toggle** — show "what the model saw" vs full ground truth (needs export to carry both fogged + unfogged
   board). (c) **Legend for the numbered markers** (clarify what the number means). All pure-viewer, no eval impact.

## Still-open pre-experiment work (unchanged)
- **Finalize kind roster + re-pre-register the matrix** (task): cut the saturated-on-raw kinds
  (direction/distance/nearest/reachability/adjacency) from RUNS (keep code); reflect t3-threat/adv-assault-target
  redesign; reach kinds → less-railed boards. Consolidated sweep = raw / raw-maxops / raw-maxops-enum /
  interactive-maxops-enum × decision+dispersed kinds × ≥3 seeds × both boards (~$10–15). Covers blog claims #2/#3/#4.
- **Budget top-up on OpenRouter** (user action) before the multi-board run.
- **Multi-board corpus** for the robustness experiment: harvest ≥10 varied boards (community saves or AI self-play),
  **ruleset-gated to `classic`** (per the docs-correction hazard), characterize by size/density/contest.

## Repo state
- **master** = `12fb015`, green. Only stray worktree left: `agent-a599176d2636a0826` (old, holds 3 untracked scratch
  scripts — safe to prune when convenient). Viewer at `viewer/` (run: `cd viewer && npm install && npm run build`,
  open `index.html`; export: `cargo run -p civ-cli -- viewer-export --trace-dir traces/<run> --out viewer/export.json`).
