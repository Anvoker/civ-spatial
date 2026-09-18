# Findings — first live smoke of the non-saturated kinds: the frontier kinds resist maximal tooling, and interactive ≥ raw

**Date:** 2026-08-07 · the FIRST live cloud run of the reasoning-frontier kinds, and the first head-to-head of
`raw-maxops` vs `interactive-maxops`. Board: **`corpus_medium_a6_s1337` T200, `--fog 1`** (Atawallpa/Inca — one of
only 3 corpus boards where P1 fires). DeepSeek V4 Flash, **think-off**, per-kind 8, cache on, concurrency 6, single
seed. Kinds = the **7 "non-saturated" set** (the ones maxops does NOT trivially ace): the two tool-fit survivors
(`constraint-site`, `compare-two-attacks`), the counting-wall encoding discriminator (`region-count`), and the four
reasoning-frontier kinds (`forward-posting`, `fogged-assault`, `hidden-force`, `surprise-strike`). Data:
`results-smoke-nonsat.jsonl` (+ full LLM I/O in `traces/results-smoke-nonsat-*`). **n = 7–8 / kind, one board, one
seed — a smoke, not a measurement; the ~12–18% per-item noise floor dominates per-kind differences.**

## Cost (the question that motivated the run)

**Total $0.62** for 110 trials (55 questions × 2 surfaces) — under the $1–2 estimate.

| surface | $ | prompt tok | cache-hit | compl tok | mean tool_calls (max) | mean turns | $/q |
|---|---|---|---|---|---|---|---|
| raw-maxops | 0.369 | 10.5M | 93.1% | 177k | 8.9 (33) | 3.5 | 0.0067 |
| interactive-maxops | 0.249 | 5.3M | 78.2% | 171k | 15.2 (61) | 5.9 | 0.0045 |
| **total** | **0.62** | 15.7M | 88.1% | 349k | — | — | — |

Caching is what makes it cheap: 88% of input tokens were cache-reads (~$0.028/M vs $0.09/M). Tool-loop depth was
deeper than the pre-run guess (~9 / ~15 calls, not ~4–6), but each re-read of the ~58k masked board was cached.
raw-maxops burns ~2× the prompt tokens of interactive (full board re-sent every tool call: mean prompt ~187k vs
~93k tok/question).

## Accuracy

| kind | interactive-maxops | raw-maxops |
|---|---|---|
| region-count | 8/8 | 8/8 |
| constraint-site | 8/8 | 7/7 |
| compare-two-attacks | 8/8 | 8/8 |
| **forward-posting** (P4) | **3/7** | **3/8** |
| **fogged-assault** (P7f) | **5/7** | **3/7** |
| **hidden-force** (P3) | 8/8 | 7/8 |
| **surprise-strike** (P1) | 8/8 | 5/7 |
| **TOTAL** | **48/54 = 0.889** | **41/53 = 0.774** |

(Denominators < 55 because `error`/`invalid`-status rows are excluded: interactive 1 invalid + 1 error; raw-maxops
3 invalid + 2 error.)

## Finding 1 — the frontier kinds are genuinely non-saturated under maximal tooling

`forward-posting` (~0.40 both surfaces) and `fogged-assault` (0.43–0.71) are **hard for both surfaces even with the
full calculator**. This is exactly what the reasoning-frontier kinds were built to demonstrate: a maxops calculator
operating on what the model can see cannot trivially solve them — for the hidden-info kinds because the answer
depends on masked-out occupants the calc can't query, for `forward-posting` because the underdetermined
pressure↔survival trade-off has no canonical weight. By contrast `region-count` / `constraint-site` /
`compare-two-attacks` sit at ceiling here (the calc solves them). So the set **cleanly separates tool-resistant from
tool-solved**, which is the whole point of the frontier line.

Caveat: `hidden-force` (P3) and `surprise-strike` (P1) scored high (7–8/8) rather than mid — either the model reasons
them well from cues, OR this board's decisive-band instances are on the easy side. n=7–8 can't tell; replicate.

## Finding 2 — interactive-maxops ≥ raw-maxops on BOTH accuracy and cost (suggestive)

interactive **0.889 vs** raw **0.774** (+11.5 pts), while being **cheaper** ($0.25 vs $0.37) and using **half the
prompt tokens**. The gaps concentrate on the hidden-info kinds — `surprise-strike` (8/8 vs 5/7) and `fogged-assault`
(5/7 vs 3/7) — where fetching relevant tiles beats swallowing a 187k-token board. raw-maxops also failed more (5
invalid/error vs 2), plausibly an artifact of those enormous prompts. This is the **blog claim-#3 axis** (front-loaded
vs query at full tooling) and it points interactive's way — but **at n=7–8 the ±11 pt gap is inside the noise floor;
this is a direction to confirm, not a result.**

## What this does and does not establish

- **Does:** the pipeline runs live end-to-end on the fogged corpus; the frontier kinds generate, render (no leak),
  and are answered by a real model; cost is trivial (~$0.62); the non-saturated set behaves as designed (frontier
  hard, calc-solved kinds easy).
- **Does not:** any per-kind accuracy claim (n too small), or the interactive>raw claim (needs ≥3 seeds × ≥3 boards).

## Next

1. **Replicate**: same 7 kinds × ≥3 boards (incl. one where P1 fires) × ≥3 seeds, to lift Findings 1–2 above the
   noise floor and get Wilson CIs. ~$2–4.
2. **Failure-mode analysis** of the frontier-kind misses (forward-posting especially) from the traces — is the model
   making a reasoning error, or is the decisive-band curation admitting litigable instances?
3. Add `-enum` surfaces only for the efficiency/cost-frontier story (enumeration is efficiency-only —
   `findings-clean-ablation.md`), not accuracy.
