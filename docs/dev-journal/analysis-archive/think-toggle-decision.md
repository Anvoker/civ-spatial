# Decision memo — think ON or OFF for a *playable* board-reading agent? Ship **think-OFF (raw)**, with the calculator as a gated escalation

**Date:** 2026-08-01 · Decision memo (no new evals run; pure analysis of existing data).
Sources: `findings-calc.md` (small-board calculator + §Scale test), `findings-crossmodel-thinkon.md`
(cross-model think-ON + frontier), `findings-fog.md` (game-phase gradient), `RESUME.md` /
`CONTINUITY-2026-07-31.md` (calibration: "thinking dominates LATENCY, not token count"; ~2.1× tokenizer
multiplier). **User story:** an agent that plays real turns; the board-interpretation step must land in
**< 10 s** or the compounded turn (many reads + planning + acting) becomes impractical to play with.

## Recommendation in one line
**Default to think-OFF raw perception. It is the ONLY measured arm that fits the < 10 s budget, and it is the
only one whose latency is structurally safe as the board grows.** Reason: think-ON's cost is *intrinsic*
(sequential reasoning generation, ~83 s) and cannot be engineered under budget; the calculator's cost is
*round-trip-bound* (~52 s) and only *partially* fixable by batching. Keep the calculator as a **gated
escalation** for the specific late-game / high-stakes reads where raw's accuracy has actually decayed below a
usable floor — accepting that when you pull that lever you are knowingly leaving the real-time budget.

## The binding constraint: which arms clear 10 s?

All numbers DeepSeek V4 Flash, T677/p4 dispersed kinds (the regime that has carried every prior deficit),
pooled 3 reps unless noted. Latency is wall-clock as-measured.

| arm | acc (small board) | **latency** | tot tok | fits < 10 s? | why the latency |
|---|---|---|---|---|---|
| **raw (think-off)** | 0.792 | **5.6 s** | 5.7k | **YES** | 1 turn, ~350 gen tok; prefill + tiny generation |
| raw **think-ON** | 0.875 | 83.2 s | 12.5k | no (8×) | **sequential reasoning generation (~7k gen tok)** |
| raw-ops (calculator) | 0.986 | 51.6 s | 59.5k | no (5×) | **7 sequential tool turns / 20 round-trips** |
| interactive | 0.819 | 92.5 s | 89k | no (9×) | over-fetch, ~10 turns / 42 calls |
| interactive-ops | 0.806 | 78.4 s | 104k | no (8×) | over-fetch + operators, worst on both axes |

**Only raw think-off clears the gate, and it clears it with ~4 s of headroom.** Every accuracy-buying arm is
5–9× over budget. On a single board-read that would be tolerable; multiplied across the many reads in one game
turn it is disqualifying, which is exactly the user story's premise.

## Is the over-budget latency intrinsic or fixable? (The crux — and the two arms differ)

This is where think-ON and the calculator are **not** interchangeable, even though both currently sit ~50–83 s.

- **Think-ON latency is intrinsic.** The calibration note ("thinking dominates LATENCY, not token count") and
  the numbers agree: think-ON generates ~7k reasoning tokens *sequentially* before the answer. You cannot
  parallelize a chain of thought; the wall-clock IS the generation. 83 s is not an engineering artifact you can
  cut — it is the mechanism. Think-ON is therefore **permanently out** of a < 10 s loop on this hardware/model.
  (It also only buys +0.08 over raw and still leaves 9/72 answers *wrong* — see below.)
- **Calculator latency is round-trip-bound, i.e. partially fixable.** raw-ops spends 51.6 s across **7 turns /
  20 tool calls** with only ~3.8k generated tokens — the time is sequential HTTP round-trips + transcript
  re-send, not thinking. In principle a **batched / program-once operator interface** (issue many independent
  measurements in one turn instead of 20 serial calls) could collapse 7 turns toward 1–2 and cut latency
  several-fold. But it will **not** reach 5.6 s and cannot be assumed to reach 10 s: (a) some operators depend on
  prior results (genuine sequential dependency remains), and (b) this is an untested engineering claim. So the
  calculator is a *candidate* for the budget after batching work — not a current fit. Treat "batch the operators
  and re-measure latency" as the single highest-value follow-up if we ever want it in the hot loop.

## Accuracy at stake — and where the floor flips the decision

Think-off raw sacrifices **~0.08 vs think-ON** and **~0.19 vs the calculator** *on the small board*. Is that
acceptable for a 9–15× latency saving in a real-time-ish loop? My weighting (stated explicitly below) says
**yes, by default** — with two sharp exceptions driven by the *failure mode*, not the headline accuracy:

| arm | correct | **wrong** | non-answer |
|---|---|---|---|
| raw (baseline) | 57 | **15** | 0 |
| raw think-ON | 63 | 9 | 0 |
| raw-ops (calculator) | 71 | **0** | 1 |

Raw's 0.792 is *all wrong answers, zero non-answers* — it is **confidently wrong ~1 read in 5** on the hardest
dispersed kinds. That matters because a confidently-wrong board read **silently poisons** the downstream
planning step, whereas a non-answer is a visible "I don't know" the planner can route around. The calculator's
signature property is **0 wrong answers** — every arithmetic slip is gone. So the decision flips **not on a
scalar accuracy floor alone, but on stakes**:

- **The answer stays think-OFF** wherever a wrong read is *recoverable* — most reads, most turns, where the
  read is one signal among many and the planner is robust to occasional error.
- **The answer flips to the calculator** for reads that are (a) load-bearing for an irreversible/high-value
  decision (settle here, commit an attack) AND (b) on the dispersed kinds where raw is weakest, where a silent
  wrong answer is worse than a 50 s pause. This is the escalation path, not the default.

Numerically: if the agent's *required* board-read accuracy on the load-bearing kinds is set **above ~0.85**,
raw think-off (0.79 on dispersed kinds) no longer clears it and you are forced to escalate. Below that, raw wins
on the latency gate.

## Board-size dependence forces an ADAPTIVE policy, not a flat toggle

A real game board grows monotonically as the player explores, and **both the accuracy gap and the right answer
move with it:**

| regime | raw think-off acc | calculator acc | note |
|---|---|---|---|
| early (6.8% explored) | **0.931** (full corpus) / 0.792 (dispersed) | **0.986** | raw strong & cheap; small gap on most kinds |
| mid (52%) | 0.667 | — | static encoders begin to anti-scale (`findings-fog.md`) |
| late (98%) | **0.597** (full corpus) | **0.736** | raw decays hard; calculator's win also **evaporates** to 0.736 and ties interactive-ops (0.750) |

Two facts collide here. (1) **raw think-off anti-scales** — its accuracy falls 0.93 → 0.60 as the board fills,
because "eyeball one giant block" gets harder (`findings-fog.md`). (2) **the calculator's win also evaporates at
scale** — 0.986 → 0.736 — because a *second* bottleneck (locating *what* to point the operators at inside a huge
board) re-emerges and caps both ops arms at ~0.75 (`findings-calc.md` §Scale test). Crucially, **raw think-off's
latency does NOT blow up with board size** — generation stays ~350 tokens; only the prefill grows and it caches
— so raw stays *fast but increasingly inaccurate* late-game, while the calculator is *slow but more accurate*
(0.736 vs ~0.60).

**Implication:** a fixed on/off policy is wrong. The honest policy is **adaptive**: think-off raw as the
default hot path across all phases (it never breaks the latency gate), escalating to the calculator only when
(exploration is high) AND (the read is load-bearing) — precisely the late-game, high-stakes cell where raw has
decayed to ~0.6 and the extra latency buys the most. Note the escalation ceiling is itself only ~0.74 at scale,
so escalation is a mitigation, not a cure.

## Cost ($) — secondary, and it does not change the answer

For a real-time loop the binding cost is **wall-clock**, not dollars, and all arms are cheap in absolute terms.
For completeness: raw think-off is cheapest by far (5.7k tok, 96% cached). The calculator is ~10× the raw
tokens but 88% cached → only ~2–3× in dollars; its real cost is the sequential round-trips (latency), which
caching does not touch. Think-ON is 12.5k tokens. None of this out-votes the latency gate.

## My assumptions / weighting (explicit, so you can disagree with the right thing)

1. **Latency is a hard gate, not a smooth trade.** The user story says < 10 s per read or the agent is
   unplayable. I treat 10 s as a pass/fail line: arms over it are disqualified from the *default* path
   regardless of accuracy, unless their latency can be engineered under it. This is the single most consequential
   assumption; if latency were a soft cost the calculator would look far more attractive.
2. **Accuracy has diminishing returns above ~0.8 for a single read**, because (a) there are many reads per turn
   so one is rarely pivotal, and (b) a robust planner tolerates some read error — EXCEPT when a read is
   load-bearing for an irreversible decision.
3. **A *wrong* read costs more than a *non-answer*** (silent poisoning vs visible abstention). This is why the
   calculator's "0 wrong" is worth escalating for on high-stakes reads even though its headline accuracy edge
   shrinks at scale.
4. **Dollars are tertiary** in a real-time loop; latency is the real cost.
5. **Model/hardware fixed** at DeepSeek-Flash-class latencies. Faster inference (or a model that reasons in far
   fewer tokens) would move the think-ON line and could reopen that arm.

## Where the recommendation FLIPS

- **Latency stops being a hard gate** (e.g. board-read is pre-computed off the critical path, or turns are not
  real-time) → the calculator becomes the default (0.986 small-board, 0 wrong) and think-off is just the cheap
  fallback.
- **Batching brings the calculator under ~10 s** (the fixable-latency bet pays off) → promote the calculator to
  default on small/early boards; keep raw only as the sub-5 s floor.
- **Required accuracy on load-bearing kinds > ~0.85** → raw think-off no longer clears it; escalation becomes
  the common case, not the exception.
- **Inference gets much faster / a terse-reasoning model appears** → think-ON's intrinsic-latency objection
  weakens and it re-enters contention (though it still trails the calculator on accuracy and leaves wrong
  answers on the table, so it is unlikely to become *the* answer).
- Note what does **not** flip it: a bigger/frontier model. GPT-5.6 raw think-off = 0.771, same 0.77–0.81 band
  as the cheap models (`findings-crossmodel-thinkon.md`) — no capability-tier rescue for raw, so "just use a
  smarter model think-off" is not an out.

## Follow-up measurements I'd want (none run here)

1. **Batched-operator latency** (highest value). Implement a program-once / multi-call-per-turn operator
   interface and measure raw-ops latency at fixed accuracy. This is the one experiment that could move the
   calculator inside the 10 s gate and change the default. *(Engineering + one eval; the `raw-maxops` branch is
   a starting point but expect the same scale ceiling.)*
2. **raw think-off latency vs board size**, explicitly. I'm asserting raw stays < 10 s late-game (prefill grows,
   generation stays ~350 tok, board caches); confirm the prefill of a 98%-explored board doesn't push the first
   uncached read over budget.
3. **The ~0.75 scale-ceiling failure analysis** (already pending, `CONTINUITY` task #1): if the late-game
   calculator ceiling is "wrong-coordinate selection," a cheap entity index — not more operators — may be the
   real late-game lever, and would change what "escalation" should even be.
4. **Cross-model latency replication.** All latencies here are DeepSeek-Flash single-model; a second lineage
   would tell us whether the 10 s gate cleanly separates the arms everywhere or just here.

## Caveats
- **Single model** (DeepSeek V4 Flash) for the latency figures; latencies are provider/network-dependent and
  will differ on other models/endpoints. The *ordering* (think-off ≪ calculator < think-on) is the robust claim.
- **Corpus-specific.** Numbers are the three dispersed kinds — the regime where the deficit lives. Local-
  aggregation kinds already sit near ceiling on raw, so raw think-off looks even better on the full slate; the
  dispersed kinds are the pessimistic case and the right one to design against.
- **Small-board vs scale.** raw-ops 0.986 is a 6.8%-explored result; it drops to 0.736 at 98% (pooled 3 reps).
  Any "calculator is +0.19" statement is a small-board statement.
- **Noise floor ~12–18%/item**; sub-0.05 accuracy gaps are pooled/replicated per source docs, but treat single
  point values as suggestive, rankings as the claim.
- The **calculator's operators are oracle-exact** by construction — its 0-wrong property is "given free exact
  measurement, the model composes correctly," not "the model can measure." A real deployed operator layer must
  actually be that accurate for this property to hold.
