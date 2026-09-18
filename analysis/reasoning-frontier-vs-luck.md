# Reasoning-bound vs information-bound — and the frozen-board ceiling theorem

*Status: DRAFT (2026-08-13). This is the methods/evidence artifact behind the day's finding. The final
blog framing is the owner's — this doc supplies the argument and the receipts, it does not "lock" the
thesis. Companions: `analysis/surprise-strike-redesign.md`, `analysis/hidden-force-audit.md`,
`analysis/kind-roster-preregistration.md`, `CONTINUITY-2026-08-13.md`.*

## 1. The question this answers
We built a "reasoning-frontier" family — kinds that maximal tooling fails to push to ceiling — on the bet
that they separate *genuine game understanding* from *tool-augmented lookup*. This doc records what we
found when we audited that bet: **three of the four frontier kinds weren't measuring reasoning at all**,
and *why that was inevitable* on a frozen board.

## 2. Two reasons accuracy plateaus — only one is a frontier
Maximal tooling can fail to reach ceiling for two fundamentally different reasons:

1. **Reasoning-bound (a real frontier).** The answer *is* fully determined by what the model can observe,
   but computing it needs reasoning no single tool bundles — adversarial lookahead, constraint
   composition, multi-step search, trade-off judgment. The model has everything it needs; it just has to
   *think*. **A smarter model would climb.**
2. **Information-bound (luck; a fake frontier).** The answer depends on facts *not in the observable
   inputs* — hidden placement, hidden tech, stochastic outcomes. No reasoning helps; the plateau is set by
   the environment's entropy, not the model's capability. **A smarter model does *not* climb.**

**The diagnostic:** *would twice the intelligence cross the plateau?* Yes ⇒ reasoning-bound. No ⇒ luck
wearing a difficulty costume.

## 3. The evidence — our three fog kinds were all information-bound
| Kind | Old ground truth | Diagnostic result | Receipt |
|---|---|---|---|
| **surprise-strike (P1)** | scariest *actual* hidden striker within reach (reads `unmasked`) | LUCK → after making it deducible it **SATURATES 8/8** both surfaces | `results-hiddeninfo-test.jsonl`; `surprise-strike-redesign.md` |
| **fogged-assault (P7f)** | capture vs *actual* hidden garrison (later worst-case, but over the *unmasked* repertoire) | LUCK/risk-posture — model scores **below the majority baseline** (raw 1/8) reasoning prudently | test above; `d904de7` |
| **hidden-force (P3)** | largest *actual* massed force on fogged tiles (reads `unmasked`) | LUCK — perturbation test **flips the answer** with a byte-identical masked board | `hidden-force-audit.md`, `rules.rs:1001` |

The decisive move each time: **is the ground-truth answer a function of the *masked* (observable) board,
or of hidden state on the *unmasked* board?** All three keyed on the unmasked board. surprise-strike is the
cleanest proof — remove the luck (make it deducible), and DeepSeek goes straight to 8/8. The plateau was
never the reasoning; it was the unknowability. Unknowability is not a skill.

## 4. The ceiling theorem — why a frozen-board frontier can't exist
Trying to "fix" the fog kinds into reasoning-bound ones surfaced the general result:

> **Any deterministic function of a fully-observable frozen board is tool-computable by construction.**

So a kind "resists tools" only because we *withhold the bundling tool*. constraint-site resists at 0.79
because there's no `find_constraint_site()` on the menu — provide it and it saturates. forward-posting
resists because no primitive computes "sound under the enemy's best reply" — provide that and it saturates.
The ideal tool *always exists* for a deterministic board-function. The "frontier" is a property of the tool
menu, not of the problem.

**Corollary — genuine tool-resistance has exactly three sources, and no others:**
1. **Luck** — the answer isn't a function of the observable state. *Rejected: not reasoning.*
2. **Intractability** — the ideal tool exists but can't run in feasible time (deep game trees, NP-hard
   optimization). *Cost: grading goes fuzzy (best-known vs optimal).*
3. **Time horizon** — the "answer" is the emergent outcome of a long interaction you can't precompute
   without playing. *Cost: latency, cost, variance — the price of the only honest escape.*

There is no free frozen-board lunch: tool-resistance costs luck, intractability, or time, and we correctly
reject luck.

## 5. What this *does* to the thesis — it sharpens it
- **Reasoning-boundness is a property of task STRUCTURE** — adversarial, compositional, temporal — **not of
  information-hiding.** The genuine reasoning-bound kinds (forward-posting, constraint-site) are fully
  *deducible from the visible board* and resist only because the reasoning isn't bundled into a primitive.
- **Frozen-state benchmarks structurally cannot demonstrate a game-competence gap.** There is always an
  ideal tool, so at most they measure tool-orchestration. The competence that decides real games lives in
  the *open, temporally-extended* decision problem — which is exactly why benchmark scores and actual play
  diverge. That divergence is not a mystery to explain away; it is a prediction of this theorem.
- **Auditing our own kinds out of the frontier is a credibility asset**, not a retraction: "we built
  hidden-info kinds, tested whether a smarter model could cross the plateau, found it couldn't, and
  excluded them on a documented validity defect." That is the trust signal a methods-focused reader
  rewards.

## 6. What we keep, and where the signal is
On a frozen board with maximal tools, **almost everything saturates — because everything is
tool-computable.** That is the theorem casting its shadow, not a defect. The microscope's real signal is:
- **Tool–task fit** — the arm spread (no-ops ≈ spatial-ops ≪ maxops): only the *matching* tool helps.
- **The calculator lever** — perception and compute are separable; cheap full perception + compute wins.
- **Won't self-select** — models don't pick the efficient tool even when it's on the menu.
- **The two withheld-tool survivors** — constraint-site (0.79), compare-two-attacks (0.88) — reframed as
  the *live demonstration* that difficulty equals the tool we chose not to provide.
- **The cost frontier** — at equal (saturated) accuracy, cost still discriminates (raw-maxops vs
  roster-maxops: same answer, ~2.6× the tokens).

## 7. The frontier that remains (post-frozen-board, not built)
Only the expensive escapes are honest: **bounded adversarial rollouts** (decide → simulate 2–3 contested
turns → score the outcome) buy *time-horizon* resistance and land the "LLMs don't model tempo" point (Vox
Deorum's world-model is verified thin on timing / no lookahead). That is the deliberate post-Saturday
direction, entered with eyes open about latency and variance — not another frozen-board puzzle chasing a
theorem it can't beat.
