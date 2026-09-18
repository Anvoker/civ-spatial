# Compute externalization & the reasoning frontier — design note

**Status:** idea capture, 2026-07-30. Not yet scheduled into an experiment. Records the "spatial
operators / query-DSL" fork and, more importantly, the benchmark-design tension it forces.

## 1. The compute-externalization spectrum

Today's `interactive` surface is one point on a broader axis: **how much of the computation the LLM
externalizes to deterministic tools vs. does itself in-context.** The axis, from least to most
externalized:

1. **Perception-only** (today): observation verbs return raw state (`scan_grid`, `get_tile`,
   `region_summary`, `list_cities/units`). The LLM does all arithmetic and integration itself.
2. **Compute operators**: add deterministic *spatial primitives* — `distance(a,b)`,
   `count(region, terrain)`, `path_cost(a,b)`, `nearest(resource, from)`. The LLM still decides
   *what* to compute and *how to combine* results, but offloads the mechanical math.
3. **Operators + composition (the DSL / program-once)**: the LLM emits ONE program with control
   flow, variables, and aggregation; the environment executes it deterministically and returns the
   result. Externalizes not just the primitives but the *integration* between them.

Forks #1 (compute operators) and #4 (DSL) from the fork discussion are the **same axis at different
power levels**. The dial matters: a weak operator (`count`) offloads a little; a strong one
(`nearest_unexploited_resource(player)`) *is* the answer. The scientifically interesting object is
not any single point but the **accuracy-vs-operator-power curve** — where on that curve the model's
deficit vanishes tells you the *grain* of spatial computation it can't do itself.

## 2. The DSL (program-once) fork — concrete

Instead of a turn-by-turn tool loop, the model writes one program the environment runs in a single
round-trip. Example — "from player P's nearest city, distance to the closest *unexploited* Iron
within travel horizon 6":

```
cities = filter(list_cities(), owner = P)
iron   = filter(tiles(has_resource = "Iron"), unexploited = true)
answer(
  min( travel_cost(c, t)
       for c in cities
       for t in iron
       if travel_cost(c, t) <= 6 )
)
```

### Is it just batched tool calls? No — and the distinction is precise.
- **Batched tool calls** = several *independent* calls issued together (`get_tile(a), get_tile(b)`).
  Fewer round-trips, but the **model still integrates the results in-context**. Solves *latency*,
  not *integration*. A batch is a **static list** you must fully specify upfront.
- **A program** carries **control flow, intermediate variables, and data-dependent steps** — step 3
  depends on what steps 1–2 return, which a batch cannot express. The **executor** does the
  integration. Solves *latency* AND *integration*.

The non-batchable part is exactly **data-dependent composition**. Corollary: if a task's calls are
all independent, a batch is just as good and far simpler — the DSL only earns its keep when later
steps depend on earlier results (the "dispersed integration" the model fails at think-off).

### Where it might fit
- As a **diagnostic arm**, not a replacement for perception-only modes.
- Best paired as a **factorial: perception (static | interactive) × compute (none | operators |
  DSL)** — this decomposes the two things `interactive` currently conflates (access vs. compute).
- Open build questions: the DSL surface/grammar, its executor, and its Oracle gate (the program's
  result must match the pure solver on generated items). Non-trivial; a project, not an afternoon.

### The validity caveat (applies to the whole spectrum)
The more computation you externalize, the more you measure **"can the model *formulate* the
computation"** and the less you measure **"can it *reason spatially*."** That's a real and different
skill (tool orchestration / formalization), legitimate to study — but it can't be the *only* mode or
you've defined away the thing the benchmark exists to measure. Keep perception-only as the anchor.

## 3. The tension this creates — and why it's productive

As operators/DSL absorb perception + arithmetic + some spatial reasoning, the LLM does *less* direct
reasoning. That is *correct* engineering — don't make the LLM recompute a travel-cost function a tool
does better and cheaper. But it puts **pressure on the benchmark to test "higher reasoning,"** and
higher-reasoning questions that are *also* meaningfully scorable **on a single frozen map state** are
hard to author.

**Reframe: the pressure is the benchmark working, not breaking.** Externalizing the mechanical layers
*purifies* the benchmark toward the frontier we actually care about — judgment, counterfactual,
adversarial, structural reasoning. The "struggle to invent questions" is the signal that the easy
(tool-solvable) questions have been retired and only the genuinely-reasoning ones remain.

**Design principle: co-design the operator set and the question set.** The operators define the
*floor* (what's free); the questions define the *ceiling* (what's asked). The benchmark's
higher-reasoning content lives in the **composition gap** between "primitives provided" and "answer
required" — and you *control that gap* by choosing which operators to withhold.

## 4. Frozen-state is less limiting than it feels — a question taxonomy

The map is frozen, but the space of **deterministic functions over it** is huge. Any function with
(a) checkable ground truth, (b) reasoning that composes/judges rather than retrieves, and (c)
resistance to a single-operator solve is fair game. Generative axes (all scorable on one snapshot):

1. **Counterfactual injection** — pose a hypothetical placement/move; ask its deterministic
   consequence. *"If you founded a city at (X,Y), how many enemy units could reach its center within
   2 turns?"* (integer; requires simulating reach from a hypothetical). Extends the T3 pattern.
2. **Multi-factor dominance** — choose-best where the answer needs weighing ≥2 factors; Pareto-scored
   (`ChoiceSet`). Already built for settle-site; push toward genuinely multi-factor trade-offs.
3. **Constraint conjunction** — find tile(s) satisfying several spatial predicates simultaneously.
   *"Name a tile that is defensible (hills/forest/mtn), within 2 of fresh water, and not in enemy
   territory — or 'none'."* (Optional answer; the *composition* is the reasoning.)
4. **Adversarial perspective** — evaluate from the *opponent's* valuation. *"Of your cities {A,B,C},
   which is the most attractive assault target for red, given walls, garrison, reachable attackers?"*
   Dominance-scored; tests opponent-modeling / theory-of-mind, still static.
5. **Structural / topological** — connectivity, containment, chokepoints, cut-vertices. *"If tile
   (X,Y) were blocked, could your capital still reach city B over land?"* (yes/no; global structure).
6. **Comparative counterfactual** — "which of two hypothetical actions is better" (1 + 2). The
   `t3-retreat` flavor.

Each resists single-operator solving *to the degree you withhold the matching operator* (§3).

## 5. The honest boundary of frozen-state benchmarking

Frozen-state can richly test **single-step counterfactuals + static judgment + structure**. It
**cannot** test genuine **multi-turn planning** (sequences of moves, opponent responses, long-horizon
strategy) — that needs *dynamics*: a game loop / simulator, not a snapshot. If the ambition reaches
"does the LLM plan strategically over turns," that is a different harness (a stepped environment),
plausibly a future **T4**. Worth knowing where the wall is so we invest in frozen-state questions up
to it, and don't try to fake dynamics with a snapshot. **A concrete proposal for that T4 —
using FreeCiv itself as a dynamic ground-truth oracle — is captured in
`dynamic-ground-truth-sim-idea.md`, along with its two hard problems (probabilistic scoring needs
many rollouts; ruleset/AI-idiosyncrasy confound).**

## 6. Candidate higher-reasoning question kinds (successor to the T3 draft)

Six concrete kinds along the §4 axes, each a deterministic function of the frozen board with a
checkable ground truth, chosen to resist single-operator solving. Answer types reference the existing
enum where possible; **(NEW answer)** flags a variant we'd have to add.

1. **`cf-settle-threat`** (counterfactual + reach). *"If you founded a city at (X,Y), how many enemy
   units could reach its center within N turns?"* → **Int**, scored exact. Solver: `ReachField` from
   each enemy unit to (X,Y), count ≤ N. Reasoning: hypothetical placement + per-enemy reach + tally —
   with only a `path_cost` operator the model must still loop and aggregate. Reuses ReachField.
   Admissible when the count is decisive (not 0, not saturated).

2. **`adv-assault-target`** (adversarial / theory-of-mind; = the planned `t3-attack-target`).
   *"Of your cities {A,B,C…}, which is player R's most attractive assault target?"* → **ChoiceSet**
   (acceptable = the non-dominated *most-vulnerable* cities on axes: low defense_multiplier, low
   garrison HP, high reachable enemy att_eff). Dominance-scored. Reasoning: flip to the opponent's
   valuation + multi-factor. Reuses `ThreatField` / `defense_multiplier` / `ChoiceSet`.

3. **`struct-chokepoint`** (structural / topological). *"If tile (X,Y) were impassable, could city A
   still reach city B over land? (yes/no)"* → **Bool (NEW answer)**, scored exact. Solver: land-graph
   BFS with (X,Y) removed; compare A→B reachability. Reasoning: global connectivity / articulation —
   a `path_cost` operator has no "blocked" arg, so the model must reason about whether the only route
   passes through (X,Y). Needs a connectivity-with-removed-node solver. Balance yes/no; pick real
   articulation points for the decisive "yes".

4. **`constraint-site`** (constraint conjunction; multiple-choice framing for clean scoring).
   *"Which of these K tiles is defensible AND within travel-2 of fresh water AND not in enemy
   territory — or 'none'?"* → **ChoiceSet** over the K offered tiles (acceptable = those meeting all
   predicates; "none" is a real option). Set-membership scored. Reasoning: compose several spatial
   predicates + a search/none proof. Reuses terrain/`ReachField`/territory; MC framing avoids an
   open-ended coordinate answer.

5. **`reach-race`** (adversarial + reach race / interdiction). *"There is unclaimed {resource} at R.
   Whose nearest unit reaches R first over land — yours, enemy E's, or a tie?"* → **Choice
   {you, enemy, tie}**, scored exact. Solver: min travel-turns to R for each side via `ReachField`;
   compare. Reasoning: comparative two-agent reach to a contested point. Reuses ReachField; admissible
   when the margin is unambiguous (avoid near-ties unless "tie" is intended).

6. **`struct-encirclement`** (structural, aggregative). *"How many independent land escape routes does
   city A have — border directions leading to reachable friendly/neutral territory?"* → **Int**,
   scored exact (or a **Bool** "encircled?"). Solver: examine A's territory boundary + land
   reachability into non-enemy space. Reasoning: integrate neighborhood + ownership + reachability
   into a global containment property. Modest new solver.

**Coverage:** counterfactual (1), adversarial/ToM (2, 5), structural/topological (3, 6), constraint
conjunction (4), multi-factor dominance (2, and 4/5 implicitly). Build cost is lowest for 1/2/5
(reuse ReachField/ThreatField/ChoiceSet) and higher for 3/6 (new connectivity solvers + a Bool answer
variant). Suggested first build: **`cf-settle-threat`** (cheapest, sharp counterfactual) then
**`adv-assault-target`** (already on the T3 follow-on list).

## 7. Project direction (decided 2026-07-30): move toward a maximal calculator + decision corpus

The intended trajectory is to **give the LLM a maximal *measurement* calculator** (distance, travel
cost, counts, unit strength, `def_eff`, threat field, …) and **rebuild the corpus so questions are
spatial DECISIONS over exact measurements** — not the LLM doing spatial arithmetic itself. This is a
deliberate **thesis shift**: "can LLMs reason about raw space" → "can LLMs make good spatial
*decisions* given oracle measurements" — the deployment-realistic setup (you'd never ship an agent
that mentally counts tiles; you give it tools). The existing **T3 dominance/`ChoiceSet` kinds are the
prototype** for that corpus.

Key mechanic (from §3–§4): a question **survives a calculator iff you withhold its *selection*
operator.** Measurement operators never save a question; only the "choose-among-K" step does. So a
maximal-*measurement* calculator (with the *selection* operators withheld) collapses the corpus onto
the **decision kinds** (settle-site, retreat, threat, best-site, …) and strips the *spatial* content
out of everything else — leaving multi-criteria **judgment** as the measured skill. That is the
target benchmark.

**Sequencing (deliberate de-risk):** roll out the **BASIC primitives-only calculator first**
(distance / travel_turns / count_terrain / count_resource), learn from it — does it confirm the
interactive deficit is *arithmetic* (calculator ≈ think-on), and do the surviving decision kinds
discriminate? — THEN commit to the full maximal-calculator pivot + corpus rebuild. The primitives run
is the evidence that tells us whether the pivot is warranted. Trade-off to stay honest about: the
pivot narrows the claim from "spatial reasoning" to "spatial decision-making with tools" — evolution,
not abandonment (the earlier encoding/access findings become "*why* you need the calculator").
