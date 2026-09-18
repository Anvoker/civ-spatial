# Dynamic ground truth via FreeCiv simulation (a possible T4) — idea capture

**Status:** idea capture, 2026-07-30. Distinct from the frozen-state benchmark; a major project, not
incremental. Records the concept and its serious difficulties so we can weigh it later.

## The idea
For *predictive* questions ("what happens next?"), don't hand-write a solver — use **FreeCiv itself as
the oracle.** Take the frozen save, **spin the game forward** (let the built-in AIs play N turns),
capture what actually happened, and have a **non-LLM script** compare the LLM's prediction to the
realized outcome. This escapes the frozen-state wall (§5 of `interactive-compute-forks-design.md`):
it's the only way to test genuine **multi-turn planning / prediction** — the ground truth is the real
game dynamics, not a snapshot function.

Example targets: "which of your cities does red capture within 5 turns (or none)?", "where is unit U
after 3 turns?", "does the enemy stack at S attack city A or bypass it?", "how does the territory
border move?"

## Why the determinism angle helps — but only partway
If the game **and** the AIs are fully deterministic given a seed (probable for FreeCiv's classic AI),
then from a fixed save the future is a **single canonical trajectory** → run **once**, compare. Cheap
and clean **for mechanically-determined predictions** (unit arrival times, forced combat outcomes,
deterministic build/move order).

**But the determinism doesn't fix scoring for *behavioral/strategic* predictions.** A single
deterministic rollout is one *sample* of an implicit distribution over sensible play. A prediction
can be perfectly reasonable yet not match the specific trajectory (the AI tie-broke toward city B
instead of the equally-attractive A). Scoring against one rollout **unfairly penalizes sensible
predictions.** To score fairly you need a **distribution over outcomes** — vary seeds / AI RNG / small
perturbations, run **many** rollouts, and score the prediction against outcome *frequency* (did the
predicted event occur in a reasonable fraction?) or calibration, not a single trajectory. That is the
heavy-duty part: the experiment multiplies by a rollout budget.

## The second hard problem: ruleset & AI idiosyncrasy confound
Prediction quality conflates **spatial reasoning** with **knowledge of FreeCiv's specific ruleset and
AI quirks.** If the AI does something idiosyncratic (a quirky tech/rush heuristic, an odd target
priority), the LLM will "mispredict" not for lack of spatial reasoning but for lack of knowledge we
never conveyed. That contaminates the measurement. Mitigations, each with a cost:
- Restrict to **ruleset-robust / mechanically-determined** predictions → reduces the confound but
  also reduces the higher-reasoning content (back to mechanical questions — the very tension in §3).
- **Tell the LLM the relevant rules / AI policy** in-context → reduces confound, but hard to convey
  faithfully and risks teaching-to-the-test.
- Score only **coarse / directional** predictions ("A is more threatened than B") rather than exact
  outcomes → robust to tie-breaks and quirks, at the cost of resolution.
- Frame it as evaluating the model's **world-model quality**, not fine prediction accuracy.

## Engineering lift (why it's a project, not an afternoon)
Needs: FreeCiv running **headless/scriptable**, **save-state injection** (start from our frozen
board), automated **turn-stepping**, deterministic seed control, **outcome extraction** (parse the
resulting saves into the same neutral `Board` we already have), a **prediction-target schema** +
event-matching scorer, and — for behavioral targets — a **multi-rollout harness** with distributional
scoring. This is a second harness alongside the frozen-state eval, plus real compute per rollout.

## Open design questions
- Prediction targets & horizon (1 turn mechanical vs many-turn strategic).
- How to **define and extract "the event"** the LLM predicted (unit positions? city ownership deltas?
  combat results? territory change?) — needs a crisp, machine-checkable event language.
- Scoring metric: exact-match (mechanical) vs event-occurrence-frequency vs calibration (behavioral).
- Determinism audit: confirm FreeCiv classic AI + engine are seed-deterministic before assuming a
  single canonical rollout is meaningful.
- Rollout budget vs. statistical power for behavioral targets.

## Relationship to the frozen-state track
Complementary, not a replacement. Frozen-state (with `ChoiceSet` dominance etc.) tests **single-step
counterfactual + judgment + structure** cheaply and exactly. The simulation track tests **true
dynamics** but pays for it in engineering, compute, and scoring-fairness complexity. Sequence: mine
frozen-state fully first (it's far from exhausted), keep this as the eventual T4 once the frozen-state
frontier is genuinely tapped.
