# Analysis — key findings

The load-bearing results, each a standalone writeup. Earlier and superseded analyses live in
[`../docs/dev-journal/analysis-archive/`](../docs/dev-journal/analysis-archive/).

| doc | what it establishes |
|---|---|
| [`findings-3arm-clean-run.md`](findings-3arm-clean-run.md) | **The headline.** DeepSeek V4, clean 3-arm run (954 trials): tools lift accuracy +24 pts (C1); the query loop ties on accuracy at 2.5–3.7× fewer tokens (C2). |
| [`findings-subs-haiku.md`](findings-subs-haiku.md) | **Cross-model replication.** Claude Haiku 4.5 (exploratory) reproduces both C1 (+17–20 pts) and C2 (2–3×), plus a temperature/variance analysis of the subscription path. |
| [`findings-cfvacate-tooltrap.md`](findings-cfvacate-tooltrap.md) | **The tool-trap.** A tool that reports current (not counterfactual) state makes a question *worse*; reproduces on both models. |
| [`findings-tool-parity.md`](findings-tool-parity.md) | **Tool–task fit.** Generic spatial ops ≈ no tools; only the *matching* operator helps, and the surface must be parity-complete with the solver. |
| [`reasoning-frontier-vs-luck.md`](reasoning-frontier-vs-luck.md) | **The ceiling theorem.** Why frozen-board difficulty is only ever a withheld tool, and why the "hidden-information" frontier turned out to be luck. |
| [`run-preregistration-3arm-2026-08-17.md`](run-preregistration-3arm-2026-08-17.md) | The pre-registered hypotheses, matrix, and deviation log for the clean run. |
