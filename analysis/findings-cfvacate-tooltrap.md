# cf-vacate: a current-state tool that misleads on a counterfactual

**Blog exhibit — the "tools can hurt" case study. Status: settled (2026-08-19).**

**One line:** a plausibly-named tool that reports the board's *current* state
(`city_fall_prob`) systematically misleads the model on a question about a *counterfactual*
board — and it does so **asymmetrically**, helping on one answer value and hurting on the other.
This is the sharpest single instance of the project's tool–task-fit thesis: more tools is not the
lever; the *matching* tool is, and a mismatched-but-seductive tool is worse than none.

---

## The question

**cf-vacate** is a yes/no counterfactual decision:

> This city is **currently secure**. If it pulls out its **best defender**, does it then
> **decisively fall**?

The oracle (`civ-eval/src/question.rs`, `CanVacateCity`) encodes exactly that: it only asks about
cities that are secure now (fall-probability with the best defender ≤ 0.3), removes that defender,
and answers **"no, can't vacate"** when the vacated city decisively falls (fall-probability without
it ≥ 0.7), else **"yes, can vacate."** It is a two-step reasoning task: *read the current board,
then imagine one piece removed and re-evaluate.*

## The trap

The maxops tool menu offers `city_fall_prob(x, y, player)`. It faithfully computes the fall
probability of the **current garrison** — the board as it stands. On a currently-secure city that
number is ≈ 0.

That is the right answer to a question nobody asked. The model calls the tool, reads `≈ 0`, and
treats it as "this city is safe" — **skipping the counterfactual re-evaluation the question
actually requires.** There is no vacate-counterfactual primitive on the menu (there is
`op_garrison_defense`, an "after-reinforcement" op for a *different* kind, but nothing that removes
a defender), so the model has no tool that answers the real question — only a well-named one that
answers the wrong one.

Representative traces from the original 3-arm run (raw-maxops arm), verbatim:

> **Wrong "yes"** (tool-armed): *"Only one unit is listed in Nancy… If we pull it out, the city
> would be undefended. But since no enemy can reach it (fall_prob = 0), it would still hold.
> **Answer: yes.**"*

> **Wrong "yes"** (tool-armed): *"the best defender is #537, and there's no second defender. Since
> the fall probability is 0… pulling the defender out leaves an undefended city, but still no
> enemy can reach it this turn. **Answer: yes.**"*

> **Correct "no"** (same city, no tools — `raw` arm): *"If we remove that best defender, the city
> has no other defender… defense = 0… the city would fall. **Answer: no.**"*

In every wrong case the model **correctly notices the city becomes undefended**, then overrides its
own board reading with the tool's current-state `0`. Unaided, it reasons straight to the right
answer.

---

## Getting to a clean measurement (a validity fix)

The effect was first spotted as a headline **inversion** in the 3-arm run: on cf-vacate the
tool-less arm *beat* both tool arms (raw 0.53 vs raw-maxops 0.23, roster-maxops 0.17) — the inverse
of every other kind. That number was an artifact.

An exhaustive oracle sweep over the corpus (216 saves × every fog perspective) showed **both frozen
run boards have zero yes-capable cities**: every threatened frontier city carries a *single*
defender, so vacating always causes a fall → every item's answer is **"no."** On an all-"no" set,
"accuracy" just measures *how often an arm says "no"* — and the tool arms' yes-bias tanked them.
The inversion was a bias artifact, not a competence gap.

The sampler was fixed to draw a **balanced** yes/no set where the board supports it (oracle and
tools untouched — only *which* cities are asked changed), cf-vacate was **excluded from the main
frozen-board cross-kind table** as corpus-degenerate on those boards (logged as a deviation), and
the kind was re-measured on a **standalone board** that supports both answers
(`corpus_medium_a9_s1337`, fog 4 — the corpus's best-balanced perspective, 8 yes / 8 no available).

---

## The result on a balanced set

DeepSeek V4 Flash, think-off, 2 seeds, n = 16 / arm.

**Aggregate — the inversion dissolves:**

| arm | accuracy | 95% Wilson CI |
|---|---|---|
| raw (no tools) | 0.625 | [0.39, 0.82] |
| raw-maxops | 0.500 | [0.28, 0.72] |
| roster-maxops | 0.562 | [0.33, 0.77] |

Confidence intervals overlap heavily — there is no real aggregate ranking. Balancing the set makes
the spurious "tools hurt" gap disappear.

**Per-answer-value — where the real effect lives:**

| arm | expected **YES** | expected **NO** | model "yes"-rate |
|---|---|---|---|
| raw (no tools) | 7 / 8 | **3 / 8** | 0.75 |
| raw-maxops | 7 / 8 | **1 / 8** | 0.88 |
| roster-maxops | **8 / 8** | **1 / 8** | 0.94 |

Reading across:

- **Tools monotonically deepen a "yes" bias** (0.75 → 0.88 → 0.94). The tool arms answer "can
  vacate" almost reflexively.
- **On genuine YES cities, tools help.** Here the current-state `city_fall_prob ≈ 0` reading is
  *correct* (the city is safe and stays safe), so it nudges the right way — roster-maxops is
  perfect (8/8).
- **On NO cities, tools hurt sharply** (3/8 → 1/8). This is the trap made concrete: the tool
  reports the *pre-vacate* state, the question is about the *post-vacate* state, and the model
  over-trusts the tool instead of finishing the counterfactual.

**The mechanism, stated once:** the tool arms trade *no*-accuracy for *yes*-accuracy by importing a
current-state signal that is right for the yes-half of the problem and misleading for the no-half.
It is not "tools hurt" — it is "a current-state tool imposes a current-state prior on a
counterfactual question."

*Caveat: n = 8 per answer-value per arm — wide intervals. This is a mechanism demonstration, not a
powered comparison (as scoped).*

---

## Why it matters for the thesis

The project's spine is **tool–task fit**: adding operators is a *compute* lever only when the
operator matches the task's actual computation. cf-vacate is the negative image of that claim —
a tool that is *almost* right (same object, `city_fall_prob`; wrong tense, present vs.
counterfactual) is not neutral, it is actively harmful on exactly the half of the problem where its
signal is stale. The fix a tool-menu designer would need is a vacate-counterfactual primitive
(`city_fall_prob_if_vacated`, analogous to the existing "after" op for reinforcement); its absence,
paired with a seductively-named current-state tool, is precisely what traps the model.

**Related:** [[fidelity-merge-bar]] (does a change flip enough answers to matter — here, yes, on the
no-half), [[project-direction-maximal-calculator]] (tool–task fit is the calculator thesis),
[[deepseek-gemini-tool-engagement]] (tool-engagement disposition — a Gemini replication of this
asymmetry would test whether the over-trust is model-specific).

**Sources:** oracle `civ-eval/src/question.rs` `CanVacateCity`; tool
`civ-eval/src/encoders.rs` → `rules.rs::city_capture_prob`; balanced sampler fix
`civ-eval/src/generators.rs` `CfVacateKind::generate`; deviation log
`analysis/run-preregistration-3arm-2026-08-17.md` §12; data
`results-cfvacate-aux-medium-a9-s1337-T0200-fog4-s{1,2}-deepseek.jsonl`.
