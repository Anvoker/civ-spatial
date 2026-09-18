# Continuity File — Spatial-Encoding Eval for LLMs on 4X Boards
**Last updated:** 2026-07-26 (handoff to Claude Code for implementation)

---

## TL;DR
Building an **eval that measures how the *encoding* of a game board affects an LLM's ability to reason about it spatially.** Freeze a real Freeciv board, describe it to the model several different ways (raw coordinates vs. adjacency lists vs. egocentric vs. ASCII map vs. image), ask the same spatial questions about each, score against ground truth. The finding is *which encoding best preserves spatial information for an LLM* — and, secondarily, whether an image underperforms every text form, and whether the ranking holds across model tiers.

This is a **frozen-state eval, not an agent.** No game runs in the loop. That is the entire point: it iterates in seconds, scoring is deterministic, and it dodges the wall-clock problem that killed every other version of this project.

**First action:** write the parser (see below). It's the only piece that gates everything else, and the input has been verified to be tractable.

---

## Why this project (context in one paragraph)
Career pivot toward applied AI / evals / reliability. Prior work is systems, testing, record-replay, optimization (C#/Rust; Labster platform work; PSO projects). Spent several days researching "LLM as 4X strategist" and concluded: (a) the *interesting* open problems there are perception and long-horizon coherence, not "can the LLM win"; (b) the field already shows LLM strategists roughly *tie* hand-coded AI, so "does the LLM win" is the wrong question; (c) every published number mixes together four distinct failure modes — **perception, valuation, protocol-compliance, long-horizon coherence** — and nobody isolates them; (d) running full games is expensive and slow (Civ V: ~2h and ~$15/game, one machine, no parallelism), so anything needing many iterations must be developed on *frozen data*, not live play. This project attacks failure mode #1 (perception) in isolation, cheaply.

## Why it's the right project (the properties it satisfies at once)
- **No game in the loop** → iterates in seconds, not hours. Fixes the binding constraint of every prior version.
- **Deterministic scoring** → exact-match against computed ground truth. No LLM-judge, no rater variance, no statistical-power problem.
- **On the actual interest** → spatial encoding, not a compromise away from it.
- **Semi-novel** → prior work (Spatial-Gym) showed VLMs do *worse* with images than text on *abstract* spatial puzzles; nobody has done a systematic encoding sweep on *real 4X boards*.
- **Portable** → the same harness can later run over Vox Deorum telemetry or any other platform's state. Upstream of everything else (if an encoding halves spatial error, that's the encoding you'd feed any future agent).
- **Legible on a résumé in one sentence:** "built an eval measuring how spatial-information encoding affects LLM board comprehension across model tiers."

---

## The core loop (conceptual, NOT prescriptive architecture)
Leave the actual program design to implementation. Conceptually there are three roles, and keeping them separate matters:

- **parser**: `.sav` file → a neutral, format-agnostic grid representation (facts keyed by coordinate: terrain, resource, river/road, ownership, unit, city). Runs once per board. Touches Freeciv format; nothing downstream does.
- **encoder**: neutral grid → a prompt representation. **This is the independent variable.** Each *style* of encoder is one experimental condition. (Note on naming: internally call it whatever; in any writeup use "encoding"/"representation" — the literature term — not "serializer".)
- **scorer**: model's answer → correct/incorrect, exact-match vs. computed ground truth. Independent of which encoder produced the prompt.

**Build order discipline:** get one end-to-end path working (crappy encoder + real scorer) BEFORE building good encoders. If you can't score, you can't tell whether a new encoding helped. Don't build five polished encoders with no way to measure them.

## The independent variable — encodings to compare
Same board, same ground-truth answers, N representations:
- raw coordinate list (the naive baseline — what most naive harnesses do)
- adjacency lists (relations precomputed, coordinates hidden)
- egocentric framing (everything relative to the player's capital / a chosen origin)
- ASCII map (2D spatial layout preserved as text)
- scene-graph / structured prose (the SAGA-style approach)
- (later) a rendered image — to test the multimodal-collapse hypothesis directly

Each encoder is conceptually a pure function `grid → string` (or `grid → image`).

## The questions (the eval content)
Generate questions *programmatically* so ground truth is free (never hand-label). Organize by the spatial skill each probes — the *shape of results across categories* is the finding, not a single aggregate number. Categories to cover:
- **adjacency** — "what's NE of tile (x,y)?"
- **distance** — "how many tiles between A and B?"
- **nearest** — "which resource is closest to the capital?"
- **region / counting** — "how many forest tiles within 3 of (x,y)?"
- **path / reachability** — "can a unit reach B from A in 2 turns avoiding mountains?"
Aim ~30–50 questions/category so encoding differences clear the noise.

## Hypotheses being tested (each is a publishable result on its own)
1. Precomputing relations (adjacency, egocentric) beats raw coordinates.
2. The image underperforms *every* text encoding — reproducing Spatial-Gym's "vision makes it worse" result on a *real* 4X board (strong, counterintuitive, would surprise most people incl. the "just screenshot it" instinct).
3. The encoding ranking either holds across model tiers or stronger models close the gap — tells you whether this is a scaffolding problem or a capability problem.

## Analysis
Accuracy by (encoding × question-category × model). Watch the *pattern*, not just the top line: e.g. direct lookups ~fine everywhere, distance/path collapse under raw coordinates but recover under egocentric, image worst across the board.

---

## Ground truth source — VERIFIED TRACTABLE
The CivRealm repo (github.com/bigai-ai/civrealm) **ships ~75 Freeciv save files** spanning the whole game arc — a ready-made corpus with a built-in difficulty gradient (sparse early boards → dense turn-677 boards). **You do NOT need CivRealm to run.** You need it only to hand you boards once. Ignore its dead agent stack entirely.

Confirmed by direct inspection of `myagent_T50_2023-08-09-08_30_01_fix.sav`:
- **Plaintext**, INI-like sections. `rulesetdir="classic"` (the harder ruleset — good).
- Map is **78×52**. Terrain is rows `t0000=`, `t0001=`, … of single-char codes.
- **Decode legends are in the same file:** `terrident` maps terrain name→identifier char; `terrain_vector` lists terrain names; `extras_vector` lists extras (Iron, Gold, Wheat, Whales, River, Road, Railroad, Hut, …).
- **Resources/rivers/roads** are bitplane rows `e00_XXXX=` (per-extra layers) — decode against `extras_vector`.
- Compressed saves (`.sav.xz`, `.sav.zst`) just need decompressing first; the T50 fix save is already plaintext.

Corpus locations in the repo:
- `myagent_T50_2023-08-09-08_30_01_fix.sav` (mid-game, plaintext — **start here**)
- `tests/game_save/myagent/` (incl. a T401 endgame, various T1 battle scenarios)
- `tests/game_save/testcontroller/` (~60 saves, turns 27–677, many scenarios)

### Parser options, cheapest first
1. **Parse the `.sav` directly.** It's documented-ish plaintext; map section is human-readable. An afternoon, zero installs, full control over ground truth. **Recommended.**
2. **Reuse CivRealm's decode logic without the server** — `src/civrealm/freeciv/map/map_state.py` and `map/tile.py` contain terrain/extras decoding. Risks pulling deps; only if hand-parsing the bitplanes is annoying.
3. **Stand up the full stack once** to dump states to JSON, then never again. Last resort (Docker + freeciv-web + broken submodule = the thing we're avoiding).

Start with option 1 on the T50 save. If the bitplane `e00_*` decoding gets fiddly, peek at option 2 for reference logic only.

---

## Model / cost notes (from this project's earlier legs)
- Prompt-cache the encoded board across all its questions → the board tokens are paid once per (board×encoding), questions are cheap. Makes a full sweep cost pennies.
- Start with ONE cheap model to debug the pipeline; add tiers only once it works.
- Cheap+capable tier for volume: Kimi K2.5 on OpenRouter (~$0.375/$2.025) — also has a *published* competence data point on Civ (CivBench). Haiku 4.5 ($1/$5) is the fast iteration model. Add a frontier anchor (Sonnet/Opus) only for the final tier comparison.
- **Pin the provider** on OpenRouter (it routes across providers at different quantizations) — silent provider switching is an uncontrolled variable in a comparison.
- Local Qwen exists on the dev machine for free overnight volume if ever needed (but this eval is so cheap it likely won't be).

## Deliberately left open (decide during implementation)
- Program architecture, language, file layout — implementer's call. (Dev is fluent in Rust and C#; Python is the path of least resistance for LLM tooling and save parsing, but not mandated.)
- Exact prompt wording per encoding.
- Whether questions are one-per-call or batched.
- How many boards from the corpus to use (start with 1, scale to the difficulty gradient later).
- Scoring tolerance for near-miss numeric answers (probably exact-match first; consider ±1 band later, but decide explicitly).

## Explicitly OUT of scope (don't scope-creep back into these)
- No live game, no agent loop, no Docker, no freeciv-web, no Vox Deorum in the loop for THIS eval.
- Not measuring strategy quality, win rate, or "was the decision good" — only *can the model read the board*.
- Not building a better Civ AI. The artifact is the eval and its findings.

## First session plan
1. Write the parser on the T50 save → neutral grid dict. Sanity-check a few tiles by hand against the raw rows.
2. Write the scorer + one trivial encoder (raw coordinates) + ~10 questions in 1–2 categories. Get one number out end-to-end against one cheap model.
3. Only then: add encodings and question categories, and start comparing.

## Related prior artifacts / threads (for continuity)
- Separate continuity file exists for the *job search* (not duplicated here).
- Earlier abandoned framing: a "wonder-racing" agent eval on CivRealm/SAGA — dropped because it had a null-result branch and needed the game in the loop. This project is the survivor of that search: same underlying interest (LLM perception + long horizons), reframed as a frozen-state eval with no null branch.
- The four-failure-modes decomposition (perception / valuation / protocol-compliance / long-horizon coherence) is the through-line; this project isolates **perception**. Protocol-compliance is the natural *next* frozen-data project (analyzable from a Haiku Vox Deorum telemetry run already captured).
