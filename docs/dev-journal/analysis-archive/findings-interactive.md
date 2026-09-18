# Findings — interactive (queryable) board access vs. static encodings

**Date:** 2026-07-28 · **Board:** `testcontroller_T677.sav` (84×56, dense), full board, difficulty
`hard`, seed 1, **think-off**, model `deepseek/deepseek-v4-flash` (OpenRouter). Design + rationale in
`interactive-board-access-design.md`. This is the first live evidence for the tool-loop board-access
mode built this session (`--encoding interactive`).

**Why this mode exists (recap):** the static `hierarchical` encoder front-loads every leaf, so it is
structurally *always* more tokens than `raw` and can never reach hierarchy's real payoff — *not paying
for detail you don't fetch* (`findings-t677-crossover.md`). Interactive gives the model an overview + a
tool loop so it queries coarse-to-fine and pays only for what it pulls. It also dissolves the
summary-tuning validity problem: the model chooses what to fetch; we measure navigation efficiency.

---

## Stage B — the head-to-head (n = 12 / mode; terrain + region-count)

| mode | accuracy | mean prompt tok | mean turns | mean tool calls | $ / correct |
|---|---|---|---|---|---|
| raw | 9/12 | 55,851 | 1 | 0 | $0.0027 |
| hierarchical | 9/12 | 79,293 | 1 | 0 | $0.0036 |
| **interactive** | **12/12** | **6,868** | 3.3 | 10.7 | **$0.0006** |

Per category (n=6 each):

| category | raw | hierarchical | interactive |
|---|---|---|---|
| terrain | 4/6 | 6/6 | **6/6** |
| region-count | 5/6 | 3/6 | **6/6** |

### Headline: interactive won on **both** axes at once
- **~8× fewer tokens than raw** (6.9k vs 55.9k), ~11× fewer than hierarchical — the "pay only for what
  you fetch" promise, measured. The token gap widens at steady state (static still pays ~56k board-read
  per question at the cache rate; interactive pays for a few small fetches).
- **~4.5× cheaper per correct answer** than raw.
- **Higher accuracy** — 12/12 vs 9/12 for both static modes.

### The most interesting result: interactive cracked region-count
Region-count is *the wall* — the aggregation skill that survived reasoning and beat every static encoding
(`findings-deepseek-fullboard.md` F4, `findings-t677-crossover.md`). Interactive scored **6/6**, vs raw
5/6 and hierarchical 3/6. The trajectories show why: the model `scan`s the bounded radius window piece by
piece and **tallies from exact leaf data over a small, focused context**, instead of eyeballing a
56k-token board and miscounting. **Counting from fetched tiles is more reliable than counting from a
dump** — querying may solve the aggregation problem that better *encoding* could not.

---

## Stage A — the smoke test (n = 6; terrain + region-count + nearest)

The 6-question smoke that de-risked the paid run and surfaced the failure mode. It confirmed:
1. **DeepSeek drives the tools** — 214 tool calls across 6 questions; the full loop (request → tool_calls
   → execute → tool result → answer → score) works end-to-end against the live model.
2. **Caching across turns works** — `cached_tokens` climb into the 100k+ range on multi-turn trajectories.
3. **A clear pathology: `nearest`.** It is an *unbounded global search* ("distance to the nearest Gold")
   with no supporting verb (excluded by design — a `find` verb would be answer-shaped). The model
   brute-force scanned: **17 turns (hit MAX_TURNS=16 + force-answer), 90–99 tool calls, 200–280k tokens**
   per question (>4× raw's whole board), and still missed one (empty forced answer → invalid).

**The shape of the result:** interactive is cheaper *and* more accurate when the question is **local**
(terrain) or **bounded-aggregation** (region-count over a radius); it **blows up** when the question needs
**unbounded global search** (nearest). This mirrors the static story inverted — front-loading wins when
you need the whole board at once; querying wins when you need a focused slice.

---

## Stage C — the siting comparison (n = 24 / mode; best-site + terrain + region-count)

The decisive test: does interactive win on a domain-real *decision* — `best-site` ("which of these K
candidate tiles has the most {terrain} in its work radius"), a best-of-K `Choice`? DeepSeek think-off, T677.

| mode | accuracy | mean prompt tok | mean turns | mean tools | $ / correct |
|---|---|---|---|---|---|
| raw | 16/24 | 55,870 | 1 | 0 | $0.0028 |
| hierarchical | 18/24 | 79,312 | 1 | 0 | $0.0032 |
| **interactive** | **24/24** | 17,114 | 3.8 | 17.3 | **$0.0011** |

Per category (n=8):

| category | raw | hierarchical | interactive |
|---|---|---|---|
| **best-site** | **4/8** | 6/8 | **8/8** |
| region-count | 6/8 | 4/8 | **8/8** |
| terrain | 6/8 | 8/8 | **8/8** |

### Headline: raw is at chance on siting; interactive is perfect
On `best-site`, **raw scored 4/8 — a coin flip** on which candidate is best — while interactive got **8/8**.
Same mechanism as region-count, now on a *decision*: raw must eyeball vicinity counts for 4 candidates
across a 56k-token board and picks wrong half the time; interactive `scan`s each candidate's neighborhood
exactly and counts reliably. **Front-loaded encodings fail at spatial decisions that need local
aggregation — exactly where the query loop shines.** Interactive went **24/24 across all three kinds**
(lookup, counting, siting) at **~3× fewer tokens and ~2.5× cheaper per correct** than raw. (The token gap
is narrower than Stage B's 8× because best-site evaluates K=4 candidate vicinities — more fetching.)

### New caveat — a cost fat-tail
`best-site` trajectories were mostly cheap (2–5 turns, 4–22 tool calls, 5–22k tokens) **except one outlier:
15 turns, 196 tool calls, 197k tokens** (still correct). Some candidate configurations trigger heavy
scanning, so interactive's per-question cost has a **fat right tail** — the mean (17k) is inflated by rare
blowups (median ~15k). Watch it as n grows.

### The consolidated story
Across three question types now: **querying beats front-loading on local, bounded, AND decision-shaped
spatial questions — cheaper *and* more accurate — losing only on unbounded global search (`nearest`).**

## Cost / mechanics notes
- DeepSeek pricing used: uncached in $0.09/M, cache-read $0.028/M, output $0.18/M. Stage B total ≈ $0.064
  (raw $0.024 + hierarchical $0.033 + interactive $0.008); Stage A ≈ $0.03. Cheap.
- `MAX_TURNS = 16` behaved as intended: **not binding** on well-behaved questions (terrain 2–3 turns,
  region-count 2–7 turns), only firing on the pathological `nearest`. It is a backstop, not the economic
  lever — the token budget / per-fetch cost is.
- **Wall-clock, not dollars, is interactive's real cost:** region-count used 20–53 sequential round-trips.
  Cheap in $, but many turns → higher latency than a one-shot.

## Caveats
- **Small n** (12/mode, 6/category). The **cost result is a firm measurement**; the accuracy edge (12/12
  vs 9/12) is a strong *direction* with wide CIs — needs a larger-n confirmation.
- **The two kinds here favor interactive.** The known loser (`nearest`, global search) is excluded, so the
  honest claim is "interactive dominates on local + bounded questions," not "interactive wins everything."
- Single board, single model, think-off. The designed target — **siting** questions (coarse-to-fine
  *survey*, not just bounded counting) — is not yet built; that is the decisive next test.
- Interactive uses coordinate referents (question rendered via the raw vocabulary); its board access is a
  tool loop, not a static block, so it has no "board-block size" row.

## Next steps
1. **Larger-n confirmation** (per-kind 16, maybe a second seed) to firm up the 8/8-vs-4/8 siting gap and the
   cost fat-tail — cheap (~$0.30).
2. A **think-on** pass (does reasoning change the picture, as it did for the static encodings? — likely
   lifts the static encoders toward interactive, which would sharpen "encoding matters most when reasoning
   is off/cheap" into "board *access* matters most when reasoning is off/cheap").
3. **`nearest` mitigation / cost fat-tail** — either exclude global-search kinds from interactive, add a
   bounded search verb (careful: not answer-shaped), or cap/penalize the over-fetch tail.
4. Deferred: the open-board `Coord` siting form (needs a real-valued desirability score to avoid ties);
   `list_cities`/`list_units` verbs; unit *type* in `describe.rs`.

(`best-site` siting question: BUILT, and interactive wins it — Stage C above.)

---

## Stage D — larger-n confirmation (n=16) + think-on pass (2026-07-29)

Both cheap follow-ups from the Stage C "next steps." Same board/model (T677, DeepSeek V4 Flash),
kinds best-site + terrain + region-count, encoders raw / hierarchical / interactive.

### D1 — confirmation at n=16/mode, think-off (`results-siting-confirm-n16.jsonl`)

| mode | accuracy | mean tot_tok | max tot_tok | mean lat |
|---|---|---|---|---|
| raw | 34/48 (0.708) | 56,894 | 62,172 | 10.9 s |
| hierarchical | 35/48 (0.729) | 80,355 | 86,658 | 11.6 s |
| **interactive** | **48/48 (1.000)** | **12,780** | 61,405 | 24.0 s |

Per category (n=16): best-site **raw 12/16, hier 12/16, interactive 16/16**; region-count raw 9/16,
hier 8/16, **interactive 16/16**; terrain raw 13/16, hier 15/16, **interactive 16/16**.

- **The interactive win holds decisively at larger n** — a *perfect* 48/48 across all three kinds, at
  ~4.4× fewer tokens than raw and ~6× fewer than hierarchical.
- **Walk-back on "raw at chance on siting":** at n=16 raw scores **12/16 (0.75)** on best-site, not the
  4/8 (chance) seen in Stage C — the Stage-C cell was an unlucky small sample. The honest claim is
  "front-loaded encodings are materially *worse* on siting/counting (≈0.7 vs interactive's ~1.0)," not
  "at chance."
- **Cost fat-tail persists but is bounded:** interactive mean 12.8k, but the max trajectory hit **61.4k
  tokens** (9 turns / 11 tool calls) — the five priciest trajectories are ALL best-site, ALL correct.
  So the worst interactive question ≈ an average raw question; the tail no longer reaches the earlier
  197k pathology at this sample.

### D2 — think-ON pass, n=8/mode (`results-siting-thinkon.jsonl`)

| mode | accuracy | mean tot_tok | mean lat |
|---|---|---|---|
| raw | 23/24 (0.958) | 65,816 | 119.8 s |
| hierarchical | 24/24 (1.000) | 86,438 | 68.4 s |
| interactive | 24/24 (1.000) | 10,042 | 30.1 s |

- **Reasoning collapses the ACCURACY gap:** with think-on, all three converge to ~perfect (raw 23/24,
  hier + interactive 24/24). The static encoders are *lifted to interactive's level* — reasoning
  compensates for front-loading. This mirrors the T50 finding (F2, `findings-deepseek-fullboard.md`)
  that reasoning collapses the raw–ascii encoding gap, now on the dense board and for board *access*.
- **But interactive keeps a large EFFICIENCY win:** ~10k tokens / 30 s vs raw's 66k / **120 s** (raw
  think-on is slow because it reasons over a 56k-token board) and hierarchical's 86k / 68 s. So the axis
  interactive dominates shifts from *accuracy* (think-off) to *cost + latency* (think-on).

### Consolidated
**Board access matters most when reasoning is off/cheap.** Think-off: interactive wins on accuracy AND
cost. Think-on: accuracy converges (reasoning rescues the static encoders), but interactive still wins
cost (~5×) and latency (~4×). The sharpened thesis: *with reasoning on, board access affects how much
you pay far more than whether you're right.*
