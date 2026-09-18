# CivSpatial

**An eval harness that measures how well LLMs read a 4X strategy-game board — and shows that, on a
frozen board, the win is almost never better reasoning. It's the right tool.**

Freeze a real [Freeciv](https://www.freeciv.org/) board, ask a model programmatically-generated
spatial questions about it ("can this unit reach that tile?", "which of these two attacks wins more
often?", "where is the best city site?"), and score every answer against **computed** ground truth.
Then vary what the model is given — the raw board, the board plus a calculator of game-mechanics
operators, or a query loop that fetches board detail on demand — and watch what actually moves
accuracy and cost.

The core is written in **Rust** (deterministic board parser, geometry, question generators, solvers,
and a self-checking Oracle); the live path talks to cloud and local models over HTTP/HTTPS with
prompt-caching and a concurrent runner.

> **Companion write-up:** the full story, prior-art survey (CivRealm, SAGA, Vox Deorum), and the
> argument for *why* any of this matters for playable game AI is in the blog post *(link on
> publication)*. An interactive board-replay **viewer** lets you step through any recorded model
> trajectory *(live demo on publication)*.

---

## Headline findings

All results below are on **fogged Freeciv boards**, questions generated deterministically, answers
scored against a computed Oracle. The clean, citable numbers are **DeepSeek V4 Flash** (think-off,
954 trials, on OpenRouter); a second-model arm on **Claude Haiku 4.5** replicates the two
load-bearing results.

- **Tools are a compute lever, not a crutch (+24 pts).** With no tools, DeepSeek answers **0.71** of
  the questions correctly. Hand it a calculator of game-mechanics operators and that jumps to
  **0.95** — and the lift is flat across task types, including raw *perception* (region-count
  0.56 → 1.00; reachability 0.62 → 0.94). What looks like a "spatial reasoning" deficit is just a
  withheld tool.
- **A query loop dominates a board dump on cost (2.5–3.7×).** Front-loading the whole masked board
  into the prompt and then running a tool loop re-sends the entire board every turn. Letting the
  model *query* for what it needs hits the **same accuracy at ~2.5× fewer tokens overall, and 3.7×
  fewer on a large board** — the gap widens as the board grows. If you build this into a real game:
  don't dump the board, let the model ask.
- **Both results replicate on a second model family.** Claude Haiku 4.5 (exploratory arm — see
  caveats) shows the same **+17–20 pt** tool lift and the same **2–3× cost frontier**. The cost
  result is a property of the arithmetic, not of one model.
- **Tools can also *hurt* — when they answer a subtly different question.** One counterfactual
  question ("if this secure city pulls its best defender, does it fall?") gets *worse* with the
  obvious tool, because the tool reports the *current* garrison, not the post-move one. The model
  trusts the stale number and skips the reasoning it would otherwise get right. This reproduces on
  both models — a clean cautionary exhibit for tool design.
- **The ceiling theorem.** Any deterministic function of a fully-observable frozen board is
  tool-computable *by construction* — so "this question is hard for the LLM" and "we haven't built
  the tool for it" are the same sentence. Genuine tool-resistance on a frozen board requires luck,
  intractability, or a real time horizon. This reframes what a board-reading eval can and can't show.

## What it measures

Each question is asked under one of three **surfaces**:

| surface | what the model gets | what it isolates |
|---|---|---|
| `raw` | the full masked board dumped into the prompt, one shot, no tools | unaided perception + reasoning |
| `raw-maxops` | the same board dump **+ the operator menu** in a tool loop | the accuracy value of tools (the *compute* lever) |
| `roster-maxops` | a compact roster **+ a query loop** that fetches detail on demand, same operators | the *cost* frontier vs. front-loading |

Questions span movement/reachability, counting/region aggregation, nearest-site search, combat-odds
comparison, city defense, and multi-step siting/retreat decisions. All ground truth is computed from
the board; nothing is hand-labeled.

## The self-consistency invariant

Because ground truth is computed and the Oracle model is deterministic, **the Oracle must score 100%**
on every question under every surface. That gate (`just verify-oracle`, plus a test) catches any drift
between generation, prompting, extraction, and scoring — with **zero API spend** — and makes the eval
regression-testable in CI, which most LLM-eval projects can't do. Two real result-corrupting bugs (a
unit-stacking render bug and an answer-extraction bug) and a wrong ground-truth oracle were caught this
way and are documented in [`analysis/`](analysis/).

## Results at a glance

Accuracy, excluding one corpus-degenerate question type (`cf-vacate`, discussed in the tool-trap
finding):

| surface | DeepSeek V4 (clean) | Haiku 4.5 (exploratory) |
|---|---|---|
| `raw` | 0.712 | 0.715 |
| `raw-maxops` | 0.948 | 0.880 |
| `roster-maxops` | 0.958 | 0.912 |

Cost frontier (tokens/answer, DeepSeek): `roster-maxops` **146k** vs `raw-maxops` **367k** (2.5×
overall, 3.7× on the large board) at tied accuracy.

Full writeups: [`analysis/findings-3arm-clean-run.md`](analysis/findings-3arm-clean-run.md) (the
DeepSeek headline), [`analysis/findings-subs-haiku.md`](analysis/findings-subs-haiku.md) (cross-model),
[`analysis/findings-cfvacate-tooltrap.md`](analysis/findings-cfvacate-tooltrap.md) (the tool-trap),
[`analysis/findings-tool-parity.md`](analysis/findings-tool-parity.md) (tool–task fit: only the
*matching* operator helps), and [`analysis/reasoning-frontier-vs-luck.md`](analysis/reasoning-frontier-vs-luck.md)
(the ceiling theorem). The pre-registration is in
[`analysis/run-preregistration-3arm-2026-08-17.md`](analysis/run-preregistration-3arm-2026-08-17.md).

## Layout

```
civ-core/   neutral Board + Tile/Unit/City, grid geometry, Freeciv .sav parser (no deps)
civ-eval/   TileFacts, encoders, Question/Answer/EvalItem + solvers, scorer, Model + Oracle, runner
civ-cli/    the `civ` binary
viewer/     browser-based replay viewer (TS canvas + Freeciv trident sprites)
data/saves/ Freeciv boards
analysis/   the key findings + the run pre-registration
docs/design/     design & decision notes    docs/dev-journal/  raw session notes (unpolished)
```

## Quick start

Requires a Rust toolchain. [`just`](https://github.com/casey/just) is the task runner.

```sh
just check          # tests + clippy + the Oracle-100% invariant
just dump           # parse a board, print stats + an ASCII overview
just questions      # generate the question set and print samples + ground truth
just eval           # run the offline Oracle sweep -> results.jsonl
just verify-oracle  # the harness self-consistency gate
```

The live path (cloud/local models) is behind `--features remote`; it needs an API key in a
(gitignored) `.env`. See [`DESIGN.md`](DESIGN.md) for the full architecture and rationale.

## Scope & honest limitations

- This measures **board-reading fitness** (accuracy and cost on well-defined questions on a *frozen*
  board), **not** full gameplay and **not** whether the resulting opponent is *fun*. The motivation
  for the latter is in the blog; this repo is the microscope, not the game.
- The **Haiku 4.5 arm is exploratory, not a clean number**: it runs through a Claude Max subscription
  under a system prompt not controlled here, and is non-deterministic at temperature 0. It is reported
  as a majority vote over three independent passes, with a variance metric, and is used only to test
  whether the DeepSeek effects generalize — never as a headline figure. The clean, deterministic arms
  are on OpenRouter.
- Reported accuracy carries a per-item noise floor; sub-0.05 claims are pooled over replicates.

## Data provenance

`data/saves/myagent_T50.sav` is a mid-game Freeciv save from the
[CivRealm](https://github.com/bigai-ai/civrealm) corpus, used only as a source of a realistic board.
CivRealm itself is not a runtime dependency. Prior art is surveyed in [`RELATED-WORK.md`](RELATED-WORK.md).

## License

MIT — see [`LICENSE`](LICENSE).
