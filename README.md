# CivSpatial

**Eval harness and LLM tooling for analyzing frozen 4X strategy-game boards.**

Freeze a real [Freeciv](https://www.freeciv.org/) board, ask a model programmatically-generated spatial questions about it ("can this unit reach that tile?", "which of these two attacks wins more often?", "where is the best city site?"), and score every answer against a static verifier. Vary the "surface" — how board data is exposed to the LLM and what tools it is given access to — then watch how the accuracy and cost change.

This project was inspired by CivRealm, SAGA, and Vox Deorum, after making the simple observation that those 4X LLM-as-AI-player projects chose to give their LLM no tools, or only high-level tools. In particular tools for spatial reasoning were absent. This resulted in the LLM spending a lot of effort dealing with classes of problems best left to traditional algorithms. Hand your LLM a family of helper functions and you'll see accuracy improve from 0.71 to 0.95 overall, and double on specific questions. Let it query the board with these tools instead of giving it the raw map and you've more than halved the cost.

The core is written in **Rust** (deterministic board parser, question generators, solvers, and a self-checking Oracle); the live path talks to cloud and local models over HTTP/HTTPS with prompt-caching and a concurrent runner.

## Headline findings

All results below are on **fogged Freeciv boards**, questions generated deterministically, answers scored against a computed Oracle.

- **Providing tools has a huge impact.** With no tools, DeepSeek answers **0.71** of the questions correctly. Hand it a calculator of game-mechanics operators and that jumps to **0.95** — and the lift is flat across task types, including raw *perception* (region-count 0.56 → 1.00; reachability 0.62 → 0.94). What looks like a "spatial reasoning" deficit is just a withheld tool.
- **A query loop dominates a board dump on cost (2.5–3.7×).** Front-loading the whole masked board into the prompt and then running a tool loop re-sends the entire board every turn. Letting the model *query* for what it needs hits the **same accuracy at ~2.5× fewer tokens overall, and 3.7× fewer on a large board** — the gap widens as the board grows. If you build this into a real game: don't dump the board, let the model ask.
- **Tools can also *hurt* when the interpretation of its usage is ambiguous to the LLM.** One counterfactual question ("if this secure city pulls its best defender, does it fall?") gets *worse* with the obvious tool, because the tool reports the *current* garrison, not the post-move one. The model trusts the stale number and skips the reasoning it would otherwise get right. This reproduces on both models — a clean cautionary exhibit for tool design.
- **Tools dominate LLM performance.** Any question you could ask that's a deterministic function of visible board state on a frozen board is tool-computable by construction. As a result, any question that is hard for an LLM has a tool you could write that turns it into an easy question. There is no particular area in this frozen-board context where the LLM can excel other than calling tools. Genuine tool-resistance on a frozen board requires luck, intractability, or a real time horizon. This reframes what a board-reading eval can and can't show.

## What it measures

Each question is asked under one of three **surfaces**:

| surface | what the model gets | what it isolates |
|---|---|---|
| `raw` | the full masked board dumped into the prompt, one shot, no tools | unaided perception + reasoning |
| `raw-maxops` | the same board dump **+ the operator menu** in a tool loop | the accuracy value of tools (the *compute* lever) |
| `roster-maxops` | a compact roster **+ a query loop** that fetches detail on demand, same operators | the *cost* frontier vs. front-loading |

Questions span movement/reachability, counting/region aggregation, nearest-site search, combat-odds comparison, city defense, and multi-step siting/retreat decisions. All ground truth is computed from the board; nothing is hand-labeled.

## The self-consistency invariant

Because ground truth is computed and the Oracle model is deterministic, **the Oracle must score 100%** on every question under every surface. That gate (`just verify-oracle`, plus a test) catches any drift between generation, prompting, extraction, and scoring — with **zero API spend** — and makes the eval regression-testable in CI, which most LLM-eval projects can't do. Two real result-corrupting bugs (a unit-stacking render bug and an answer-extraction bug) and a wrong ground-truth oracle were caught this way and are documented in [`analysis/`](analysis/).

## Results at a glance

Accuracy:

| surface | DeepSeek V4 | Haiku 4.5 |
|---|---|---|
| `raw` | 0.712 | 0.715 |
| `raw-maxops` | 0.948 | 0.880 |
| `roster-maxops` | 0.958 | 0.912 |

Token usage (tokens per answer):

| surface | DeepSeek V4 | Haiku 4.5 |
|---|---|---|
| `raw` | 40k | 40k |
| `raw-maxops` | 340k | 161k |
| `roster-maxops` | 143k | 78k |

Cost per 1,000 answers, at list prices (DeepSeek `$0.04984`/`$0.09968`, Haiku `$1`/`$5` per 1M in/out):

| surface | DeepSeek V4 | Haiku 4.5 |
|---|---|---|
| `raw` | ~$2 | ~$44 |
| `raw-maxops` | ~$17 | ~$168 |
| `roster-maxops` | ~$7 | ~$85 |

The query loop is **~2.4× cheaper than the full board dump at tied accuracy** (up to 3.7× on the large board — `raw-maxops` re-sends the whole board every tool turn, so its cost grows with board size). These are no-cache list prices; the runs cached the board (~97% hit), so real spend was several-fold lower (e.g. `roster-maxops` ≈ $1.8/1k on DeepSeek).

Full writeups: [`analysis/findings-3arm-clean-run.md`](analysis/findings-3arm-clean-run.md) (the DeepSeek headline), [`analysis/findings-subs-haiku.md`](analysis/findings-subs-haiku.md) (cross-model), [`analysis/findings-cfvacate-tooltrap.md`](analysis/findings-cfvacate-tooltrap.md) (the tool-trap), [`analysis/findings-tool-parity.md`](analysis/findings-tool-parity.md) (tool–task fit: only the *matching* operator helps), and [`analysis/reasoning-frontier-vs-luck.md`](analysis/reasoning-frontier-vs-luck.md) (the ceiling theorem). The pre-registration is in [`analysis/run-preregistration-3arm-2026-08-17.md`](analysis/run-preregistration-3arm-2026-08-17.md).

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

The live path (cloud/local models) is behind `--features remote`; it needs an API key in a (gitignored) `.env`. See [`DESIGN.md`](DESIGN.md) for the full architecture and rationale.

## Scope & honest limitations

- This measures **board-reading fitness** (accuracy and cost on well-defined questions on a *frozen* board), **not** anything resembling full gameplay.
- Haiku 4.5 was run through a Claude subscription where temperature cannot be controlled, so there is some non-determinism involved in its execution.

## Data provenance

`data/saves/myagent_T50.sav` is a mid-game Freeciv save from the [CivRealm](https://github.com/bigai-ai/civrealm) corpus, used only as a source of a realistic board. CivRealm itself is not a runtime dependency. Prior art is surveyed in [`RELATED-WORK.md`](RELATED-WORK.md).

## License

MIT — see [`LICENSE`](LICENSE).
