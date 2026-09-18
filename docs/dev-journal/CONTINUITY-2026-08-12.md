# CONTINUITY — 2026-08-12 EOD

Dated handoff snapshot. Durable state + command reference live in `RESUME.md`.

## Headline
A **harness-hardening + publication-planning** session. We did NOT run the big experiment — instead we
de-risked it: two audit agents swept the harness for validity confounders and prompt errors, we fixed
everything found (prompts + **C1/C2/S1/S2** + a resume feature), a **smoke slice validated the fixes
live**, and we drafted (but did not run) the first *clean* experiment matrix + cost estimate. Also: a
big **strategic/thesis discussion** (parked, owner-owned) and **Vox Deorum research** for the blog.
Three commits landed on master; everything green. **Nothing is running. Review the plan, then run
tomorrow.**

## Git state
- master = **`ba0ff61`**. Today's commits, oldest→newest:
  - `2ea027e` fix(prompts): stale/false tool text in maxops preambles (7 prompt fixes)
  - `bdde824` feat(eval): cache-deduct budget, off-menu tool guard, Answer:-line scoring, resume/flush
  - `ba0ff61` fix(resume): key the 6-tuple on model.name(), not the --model flag
- Worktrees pruned. `just check` green (verify-oracle ×4), remote build green, remote regression test green.
- **Uncommitted (intentionally left for owner review):** `viewer/src/sample-data.js` (re-bundled with the
  merged sgd2 + raw-maxops smoke data — 120 questions), `viewer/samples/latest/` (the per-arm export
  JSONs), and the pre-existing untracked root exports. Decide tomorrow whether to commit the viewer bundle.

## What we hardened (the technical core of the day)
Two read-only audit agents + one implementation pass + one bug-fix. **Reassuring bottom line: the
fog seam, tool↔solver parity, encoder fidelity (stacking/walls), perspective threading, and determinism
are all CLEAN.** The damage was localized to the interactive-loop budget and answer extraction.

| Fix | What it was | Status |
|---|---|---|
| **Prompt fixes (7)** | stale `get_tile` ref in the live preamble; nonexistent `{unit #id}` marker on interactive; duplicated unit-id text; raw-maxops unknown-tool error listed 10/15 (now generated from source); budget header overclaimed "missing answer" (now truthful re: the free forced-answer turn); raw "see every tile" vs fog; misleading "Unlisted tiles are {default}" | `2ea027e` |
| **C1 — budget asymmetry (MAJOR confounder)** | the interactive loop's `token_budget` summed every turn's FULL `prompt_tokens`, re-counting the cached re-sent board each turn → raw-maxops (~55k board) tripped the 200k cap at ~turn 3 while its 16-turn allowance sat untouched; interactive (small overview) rarely bound. Asymmetrically starved raw of iterations → **all prior raw-vs-interactive numbers were confounded.** Fix: `spent = prompt_tokens - cached_tokens + completion_tokens`, and `token_budget` 200k→**2,000,000** (a silent runaway/OOM backstop; **turns now govern both surfaces**, `max_turns` kept at 16). The model prompt is turns-only (LLMs can't count tokens — by design). | `bdde824` |
| **C2 — off-menu tools still executed (latent confounder)** | the loop ran ANY tool name; withheld/removed tools were dropped from the menu text but still answered by name — only a strict provider stopped it. Clean for DeepSeek (0 off-menu calls) but would break the planned **Gemini** cross-model run + any withhold A/B. Fix: guard `tools.iter().any(|t| t.name == call.name)`; off-menu → error result, not execution. | `bdde824` |
| **S1 — extraction hardened on only some types** | Int/Coord/OptionalInt required an explicit `Answer:` line (B2 fix); Direction/Bool/Choice/ChoiceSet/Compare3 fell back to whole-reply and mined tokens. Fix: require the `Answer:` line for ALL types. **Watch-item:** on a model that answers in prose without an `Answer:` line this now scores *invalid* — stricter/correct, but pre-fix numbers on those types aren't directly comparable. DeepSeek was clean (0 invalid in the smoke). | `bdde824` |
| **S2 — "none"/"incomparable" false positives** | for OptionalInt/Compare3 the none/incomparable verdict won before the value match and scanned the whole field, so `Answer: 3 (none of the others qualify)` scored WRONG. Fix: value-first, none/incomparable only as fallback, token search confined to the answer line. | `bdde824` |
| **Resume + incremental flush** | new crash-safe sink: append+flush one JSONL line per trial (concurrency-safe `Mutex<BufWriter>`), and a `--resume`/`--fresh` truth table keyed on the 6-tuple **(encoding, item_id, board, model, think, effort)** so a crashed run never re-pays for done trials. Semantics per owner spec: `--resume` opt-in (errors "nothing to resume" if no match); neither-flag refuses to clobber a matching file (asks to disambiguate); `--fresh` truncates. | `bdde824` |
| **Resume model-key bug** (found by the smoke) | the resume key used the bare `--model` flag, but rows persist `model.name()` which for a live model folds in think state (`"…[nothink]"`). So a live `--resume` matched nothing → errored; and the neither-flag guard saw zero matches → **silently truncated + re-spent** (it wiped the smoke JSONL; recoverable). Oracle (name==flag) masked it → the offline test missed it. Fix: key on `model.name()` via a shared `model_label(id,think)`; regression test asserts `Model/ChatModel::name() == model_label`. | `ba0ff61` |

## Smoke slice — the fixes validated LIVE (DeepSeek, medium board, per-kind 2, fp+hf+nearest-owned)
- **C1 validated (the money result):** raw-maxops turns went **3→17** (now hits the 16-turn cap, +1 forced
  answer); cache-deducted spend ~28k–110k, far under the 2M backstop. **Turns govern both surfaces now →
  the raw-vs-interactive comparison is fair for the first time.** (Also retro-confirms keeping 16: fp and
  nearest-owned legitimately want all 16 on both surfaces.)
- **S1/S2 held:** 0 invalid, 0 error.
- **Resume works live:** `--resume` skips the done trial; neither-flag refuses instead of clobbering.
- **Per-trial economics (real):** interactive-maxops median **$0.012/trial**, raw-maxops **$0.030/trial**;
  cache hit 96–99%. **Money is NOT the constraint — wall-clock is** (C1 ~tripled raw's time: 3→16 turns).

## The experiment plan — PARKED, ready to run tomorrow (owner to review/adjust)
Proposed first *clean* matrix (a focused subset of the frozen 19-kind roster):
- **Encodings:** raw-maxops + interactive-maxops (headline, now unconfounded).
- **Boards:** medium `corpus_medium_a6_s1337-T0200` + large `corpus_large_a3_s42-T0220`.
- **Kinds (~8):** *frontier* forward-posting, hidden-force, fogged-assault, surprise-strike, constraint-site;
  *contrast* region-count, settle-site, compare-two-attacks.
- **per-kind 8** (n=16 pooled over 2 boards), **1 seed** to start (add seed 2 only where marginal).
- **Size** 256 trials · **cost ≈ $5/seed** · **wall-clock ≈ 1–3 h/seed** at concurrency 8 (the real budget).
- **Deliberate choices:** nearest-owned LEFT OUT (unbounded-search latency bomb, >2 min/trial); single
  model (Gemini is a separate later decision + needs its own mini-smoke first).
- **Cheap pre-flight (do FIRST):** the same matrix at `--per-kind 1` = 32 trials, ~$0.70, ~10–20 min —
  calibrates real wall-clock and catches zero-yield / timeout / format problems before the full n.

### Pre-flight checklist before the full spend (from the "anything else to check?" discussion)
1. ✅ resume fix landed + verified live.
2. **Pre-register + cost the matrix** (above — owner to freeze kinds/boards/per-kind/seeds).
3. **Per-kind-1 dry run** of the exact matrix (folds in the zero-yield/timeout/format checks).
4. **Ruleset consistency** across the corpus boards (combat constants are `classic`-only; corpus is
   self-play-classic so likely fine — confirm the matrix's boards are classic).
5. **If Gemini is in scope:** a 2-trial Gemini mini-smoke (its caching feeds C1, format feeds S1, C2 exists
   because of it).
6. **Power/n** vs the ~12–18% noise floor and the specific blog claims.

## Strategic / thesis discussion — PARKED (owner owns the framing; do NOT declare it "locked")
The project's story has sharpened past "raw vs interactive." The working thesis (owner is refining it):
**the real problem in 4X AI is fair-difficulty competence (challenging on Prince without cheat bonuses),
which needs a genuine model of the game — timings, wonder value, tempo, spatial reasoning — that current
LLMs lack. They add strategy *diversity* (not LLM-specific; scriptable; ≠ competence), not understanding.**
Two poles, both collapse via the same tool-task-fit "squeeze":
- **Pole A — LLM as autonomous player (CivRealm):** give it more to do → it fails. **Our eval is the
  controlled microscope** proving it — tool-resistant micro-decisions plateau at ~0.4 even maximally tooled.
- **Pole B — LLM as high-level tuner over a scripted AI (Vox Deorum):** competence converges to the script.
- Distinguishability ("can a human tell LLM vs scripted?") measures the WRONG thing — it rewards any
  difference, not competence.

### Vox Deorum intel (for the blog's Pole B — two research agents, primary sources)
- **Repo/paper:** `github.com/CIVITAS-John/vox-deorum`; arXiv **2512.18564** (John Chen, U. Arizona) — a
  **2,327-game controlled study** (itself an eval-with-replay-viewer portfolio piece; a peer/related-work
  anchor and a signal the genre lands).
- **Architecture:** LLM sets grand/economic/military strategy, tech, policy, diplomatic *persona*; the
  scripted Vox Populi C++ AI executes all tactics/combat/city-micro/tile-improvements. LLM outputs are
  menu-constrained (tool-calling) → can't emit illegal moves, which also bounds its influence.
- **Performance CONVERGES:** win-rate statistically indistinguishable from base VP AI (dev: *"not better,
  but not worse"*). Behavior *statistics* diverge (victory mix, strategy-switch cadence) but **human "feel"
  is unmeasured** — and since tactics/diplomacy execution stay scripted, felt difference is likely at the
  strategic-arc level only.
- **World-model is THIN (verified from the code):** no wonder/production tool at all (only a single "Wonder"
  flavor weight) → **cannot evaluate a specific wonder's value/cost/timing/contestedness**; enemy production
  + research are stripped; military is one strength scalar; **no lookahead** and briefers are *explicitly
  barred from prediction* → **no timing-push reasoning.** Best described as *rich descriptive snapshot, thin
  mechanistic/predictive model* — the paper's own §7 concedes weak spatial reasoning, "strategic
  stubbornness," "wishful thinking." **Confirms the owner's "it barely models the game, just says credible
  things" read — structurally, not as anecdote.**

## Publication push — Saturday 2026-08-15 (v1-acceptable, chiselable later)
Goal (portfolio artifact for **eval / agent-eng** roles; also informative to 4X-LLM practitioners):
1. **Public GitHub repo**, documented + usable, framed around the design-space map (not "raw vs interactive").
2. **Viewer on GitHub Pages** — confirmed feasible (fully self-contained, relative paths, no CSP blockers;
   ~10-line Action pointing Pages at `viewer/`). Repo hygiene checked: no secrets tracked, corpus save-blobs
   + traces excluded, only 2 small demo `.sav`s + GPL trident assets committed. Remote not yet set — publish
   = create repo + push (outward-facing; get owner go before flipping public).
3. **Blog post** on the two-pole squeeze (Pole A = our eval microscope, Pole B = Vox Deorum), with the
   viewer as the live artifact and the harness rigor as the methods/credibility backbone. Blog lives at
   `C:\Projects\anlog-blog` (Astro; owner was investigating — left to owner).

## NEXT SESSION (tomorrow)
1. Owner reviews this plan; freeze the matrix knobs (kinds/boards/per-kind/seeds; Gemini in or out).
2. Run the **per-kind-1 pre-flight** → then the full matrix (resume protects it; ~1–3 h/seed).
3. Decide whether to commit the viewer sample bundle (`sample-data.js`, currently uncommitted with the
   merged 120-question data).
4. Begin the Saturday deliverables (repo public + Pages + README + blog) — parallelizable with the run.

**Health:** `just check` = test + clippy(-D warnings) + verify-oracle, green. Remember `cargo build -p
civ-cli --features remote` before any cloud run; key is in `.env` (`set -a; source .env; set +a`). Do NOT
`cargo fmt --all` (rustfmt-version drift). Cloud `run` now supports `--resume`/`--fresh` (opt-in; 6-key).
