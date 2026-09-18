# CivSpatial — Related Work & Positioning

**Purpose:** capture the prior art found while scanning the literature, state honestly what is
already established (cite as *foundation*, not *discovery*), and pin down the narrowed novelty
CivSpatial actually owns. Also records the designs/metrics we're borrowing → roadmap.

> **Verification status (2026-07-27):** the top-5 papers were read *in full*; facts confirmed against
> the paper abstract/body are unmarked. Figures read from a plot or a single body-fetch are marked
> **(fig)** or **(body-only)** — directionally reliable, spot-check the source table/figure before
> quoting an exact value in a formal writeup.

---

## 1. The closest twins — parts of our thesis are already prior art

- **Talk like a Graph** (Fatemi, Halcrow, Perozzi — Google, ICLR 2024, arXiv 2310.04560). The direct
  methodological ancestor: freeze a graph, vary *only* the text encoding, score vs computed ground
  truth (NetworkX) on their **GraphQA** benchmark (7 tasks: edge existence, node degree, node count,
  edge count, connected nodes, cycle check, disconnected nodes — **all topological/counting; shortest
  path is *not* in this paper**). It factors the encoder into **node-labeling × edge-phrasing** and
  sweeps each (integers / letters / real names / South-Park / GoT / politician names × "(0,1)" /
  "are friends" / "wrote a paper together" / arrows / incident). **Confirmed:** encoding choice swings
  accuracy **4.8–61.8%** (verbatim abstract); **no universal winner** — "incident" wins specifically
  on *node-degree* and *connected-nodes*, integer labels help arithmetic-output tasks, semantic names
  help others; performance also depends heavily on graph structure. Models: PaLM 62B / PaLM-2 (not GPT).
  *A reviewer will say "CivSpatial is Talk-like-a-Graph for game boards"; we cite it and position
  against it.* Gaps (by omission, not disclaimed): no metric/2D-spatial content, no token-cost axis,
  synthetic graphs only, no perception-isolation framing.
- **Text2Space / "Learning to Draw ASCII Improves Spatial Reasoning"** (Huang et al., arXiv 2604.14641,
  Apr 2026). Isolates perception from reasoning: *"the bottleneck is not reasoning itself but the
  quality of the representation it operates over"* (body). **Confirmed read/write asymmetry** — models
  read ASCII far better than they write it (Qwen3-30B **55.8% read vs 29.2% write**; gaps ~13–27 pp
  across 6 models). Supplying *ground-truth* ASCII lifts reasoning over text-alone (Qwen3-30B
  75.0→83.7, +8.7 pp). Causal ingredient is learning to *construct* the representation (comprehension-
  only training didn't help). Synthetic scenes, ASCII-vs-NL, angle is *training*.
- **Lost in Aggregation** (Jiang, Luo, Meng — arXiv 2606.22219, Jun 2026). Maze navigation decomposed
  into **Fine** (local passability) / **Meso** (junction topology, dead-end recognition) / **Macro**
  (global heading). **Confirmed:** end-to-end navigation *"collapses to near zero by 10×10 for every
  model,"* yet isolated single-level probes hold at **30–75%** far beyond that; first-error split is
  **Meso 59% / Fine 39% / Macro 1%** — i.e. *"the barrier is the cross-scale aggregation of individually
  available competences… not any single perceptual deficit."* Structured **coordinate text beats
  rendered images** (mean success 0.34 vs 0.07 **(body-only)**). Models: GPT-4o, DeepSeek-V3,
  Llama-3.3-70B.
- **SpatialEval** ("Is a Picture Worth a Thousand Words?", Wang et al., NeurIPS 2024, arXiv 2406.14852).
  Canonical **"text beats image"** source. **Confirmed (verbatim abstract):** *"(1) …competitive models
  can fall behind random guessing; (2) Despite additional visual input, VLMs often under-perform
  compared to their LLM counterparts; (3) …models become less reliant on visual information if
  sufficient textual clues are provided."* Counting is a first-class dimension across its 4 tasks.

## 2. Results we CONFIRM, not discover (cite as foundation)

1. *Encoding/serialization choice materially changes accuracy* — Talk-like-a-Graph (4.8–61.8%), tables
   (~16 pt spread, 11-format study), chess, grids.
2. *Best encoding is model-/task-dependent, no universal winner* — Talk-like-a-Graph (explicit); chess
   (o4-mini format-sensitive, Grok-3-mini indifferent — **body/snippet**).
3. *Text/coordinates beat images* — SpatialEval (verbatim); Lost-in-Aggregation (coordinate ≫ picture).
4. *"coords > ASCII > natural language"* is roughly the field's headline.
5. *Counting/aggregation is the wall, and it's architectural* — "Why LLMs Struggle to Count Letters"
   (8 models; *"the models are capable of detecting the letters, and their limitation is counting
   them"*; failure scales with item **multiplicity**, not tokenization/frequency; once→~5%, twice→
   50–80%, thrice→80–90% error **(fig)**); Lost-in-Aggregation (aggregation, not perception, is the wall).

## 3. Supporting works by theme

**Encoding/serialization ablation.** NLGraph (NeurIPS 2023; generate→serialize→score-vs-truth — our
harness for graphs; degrades with size/density). "Which table format…?" (11 formats; Markdown-KV best
~60.7%, CSV cheap-but-weak ~44.3%; token cost *inverts* the ranking — an explicit accuracy-vs-cost
frontier — **blog/snippet**). Table Meets LLM (WSDM 2024; markup/HTML helps — **snippet**). **Let Me
Speak Freely** (EMNLP 2024 — *output*-format restriction degrades reasoning; complementary axis; our
elicitation is held constant, so control for it).

**Spatial benchmarks (isolated).** StepGame (arXiv 2204.08292; multi-hop text spatial, controllable
difficulty — our question-generator lineage). SpartQA, bAbI 17/19 (ancestors; Text2Space reports +49.0
pp / +20.6 pp transfer from ASCII-construction training). FloorplanQA (arXiv 2507.07644; a JSON-vs-XML
ablation found **near-invariance** — because both are coordinate-structured; representation-invariance
holds *within* a family but breaks *across* families = our axis). "Stuck in the Matrix" (arXiv
2510.20198; ASCII-grid probes collapse with size; a **delimiter/tokenization ablation** — removing
spaces so runs tokenize as units *helped* — **snippet**). Grid/maze/Sokoban text-format studies (2026).
Chess representation (FEN vs ASCII vs PGN; FEN's uneven row tokenization "hinders spatial understanding";
PGN→FEN is hard → explicit state encodings beat implicit — **snippet**).

**Vision-vs-text.** SpatialEval (above). "Why Is Spatial Reasoning Hard for VLMs? An Attention
Perspective" (arXiv 2503.01773). VLM counting/subitizing (arXiv 2605.30170, 2604.10039; precise ≤~4
objects — predicted cliff for a future image region-count encoding).

**Domain (Freeciv/Civ).** CivRealm (ICLR 2024, arXiv 2401.10568; ships two of our encodings — BaseLang
= 5×5 egocentric window, Mastaba = hierarchical 15×15→9-block pyramid; no clean encoding ablation).
**CivRealm-SAGA** (arXiv 2606.29932, 2026; bottleneck = "information architecture"; failure mode #1 =
"scene blindness — raw coordinates lack semantics"; ablation: removing its Scene Graph drops score −25%
*while tokens rise*; but its scene graph is **lossy** (close/medium/far buckets + local neighborhoods)
→ would FAIL our round-trip gate; cite as motivation, not clean perception evidence — **table via HTML**).

## 4. What CivSpatial genuinely owns (narrowed, defensible)

Position on the *intersection*, not the pieces:
1. **Real, frozen Freeciv board** — heterogeneous terrain + units + cities + resources; not toy grids/
   mazes/floorplans/8×8 chess or topological graphs.
2. **Breadth of encodings head-to-head + a queryable tool surface** on one **accuracy-vs-token**
   frontier — the literature ablates 2–3 formats, usually *within* one family.
3. **Model-dependence of the winning encoding as a first-class result** (cloud→raw, local Qwen→ascii)
   — the field mostly asserts a single global best (FloorplanQA even reports invariance).
4. **Perception isolated from strategy/valuation on a real strategy game** with computed ground truth.
5. **A possibly Pareto-*dominant* encoding** (better *and* cheaper) — stronger than the tabular study's
   "forced tradeoff."

**One-line positioning:** *"Talk-like-a-Graph, extended to real 2D spatial game state, with a perception
isolate and an accuracy-vs-token frontier."*

## 5. Borrowed designs → roadmap (prior-art-motivated)

- **Difficulty/scale-crossover** *(prioritized; quadruple-motivated: SAGA, NLGraph, Lost-in-Aggregation,
  Stuck-in-the-Matrix)* — does raw's advantage over adjacency **invert** as board size/density/difficulty
  rises? We already have crop/full-board + `--difficulty`.
- **Region-count accuracy vs. TRUE COUNT** *(prioritized; free from existing JSONL)* — mirror the
  letter-counting multiplicity curve; predict a sharp cliff as the count grows.
- **Factored labeling axis** — from Talk-like-a-Graph's **node-labeling × edge-phrasing**: isolate
  *labeling* (integer coordinates vs named/semantic tile IDs) as its own knob.
- **Ground-truth-representation control** — from Text2Space (75.0→83.7): hand the model a clean canonical
  layout and measure the reasoning lift, to prove our failures are *read/representation* failures, not
  reasoning failures.
- **Fine/Meso/Macro probe decomposition** (Lost-in-Aggregation) — split region-count into
  perceive → filter-by-terrain → aggregate, to localize *where* it breaks (their result predicts it's a
  cross-tile aggregation failure, not per-tile reading).
- **Tokenization/delimiter ASCII ablation** *(cheap follow-up)* — from chess + Stuck-in-the-Matrix;
  likely explains local-Qwen→ascii.
- **Scene-graph: bucketed-vs-exact two-arm** — SAGA's discretized distances likely help relations but
  hurt exact counts; keep both, and keep it information-complete (surface + backing), unlike SAGA's
  lossy compressor.

## 6. Sources
- Talk like a Graph: https://arxiv.org/abs/2310.04560 · code https://github.com/google-research/talk-like-a-graph
- NLGraph: https://arxiv.org/abs/2305.10037
- Text2Space / ASCII: https://arxiv.org/abs/2604.14641
- Lost in Aggregation: https://arxiv.org/abs/2606.22219 · https://yuhanjiang415.github.io/lost-in-aggregation/
- SpatialEval: https://arxiv.org/abs/2406.14852 · https://spatialeval.github.io/
- Why LLMs Struggle to Count Letters: https://arxiv.org/abs/2412.18626
- StepGame: https://arxiv.org/abs/2204.08292 · FloorplanQA: https://arxiv.org/abs/2507.07644 · Stuck in the Matrix: https://arxiv.org/abs/2510.20198 · GRASP: https://arxiv.org/abs/2407.01892
- Chess representation: https://www.aidancooper.co.uk/pgn2fen-benchmark/ · https://github.com/google-deepmind/searchless_chess
- Table formats: https://blog.iptek.web.id/posts/2025-10-05-llm-table-format/ · Table Meets LLM: https://www.microsoft.com/en-us/research/wp-content/uploads/2023/12/wsdm24-SUC.pdf
- Let Me Speak Freely: https://arxiv.org/abs/2408.02442
- VLM spatial/counting: https://arxiv.org/abs/2503.01773 · https://arxiv.org/abs/2605.30170 · https://arxiv.org/abs/2604.10039
- CivRealm: https://arxiv.org/abs/2401.10568 · CivRealm-SAGA: https://arxiv.org/abs/2606.29932 · repo https://github.com/bigai-ai/civrealm
