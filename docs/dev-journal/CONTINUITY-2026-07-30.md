# CivSpatial — Continuity / Handoff, 2026-07-30

Where we left off. Read this first, then `RESUME.md` (evergreen state), `analysis/findings-fog.md`
(newest results), `T2-T3-design.md` (valuation, now incl. §4.3 T3b), and the design notes
`interactive-compute-forks-design.md` + `dynamic-ground-truth-sim-idea.md`. Supersedes
`CONTINUITY-2026-07-29.md`.

> **Headline:** big session. Built + **replicated-isolated** the small-map interactive levers; the
> **enriched region_summary is the accuracy lever, scan_grid+nudge the efficiency lever**; interactive is
> **no longer accuracy-dominated early-game (but still ~13× tokens)**. Built + merged **t3-threat** (city
> threat/exposure). Built + merged an **LLM I/O trajectory tracer** (all runs now persist full prompts/tool
> calls/replies). Designed the **compute-operators→DSL fork** and a **FreeCiv-sim T4**. Decided the
> **observability stack** (OTel GenAI → self-hosted Langfuse); adapter build in flight.

## Repo state — all on `master`, green (`just check` equiv: tests, clippy default+remote, Oracle-100% both boards both tiers)
Key commits this session (oldest→newest):
- `1009cb3` scan_grid bulk-region fetch + fetch-whole-when-small nudge (interactive).
- `e6a856c` merge **t3-retreat** dominance kind.
- `fe6dc45` enriched region_summary (cities-by-owner + per-owner unit tally + resource total).
- `18e001b` **moving cache breakpoint** over the interactive transcript.
- `3fcc75d` resource deposits counted **per type**.
- `0664dd7` cross-model Gemini findings + think-off agentic-effort asymmetry.
- `4c92622` design notes: compute-externalization forks (operators/DSL) + FreeCiv-sim T4 idea.
- `25ce137` / `e8b658d` replicated isolation sweep findings **(+ overview-size correction)**.
- `7e1e6d6` merge **t3-threat** city-threat exposure kind; `2e4866a` §4.3 margin wording aligned.
- `0d1b7a3` merge **LLM I/O trajectory tracer**.

Untracked: `results-sweep-*.jsonl`, `log-*.txt`, `traces/` (git-ignored).

## What shipped (2026-07-30)

### 1. Small-map interactive levers + the replicated isolation (`analysis/findings-fog.md`)
- **scan_grid** — a new uncapped bulk-region verb (compact glyph grid + exact-detail appendix); one call can
  cover the whole small board. **nudge**: prompt tells the model to fetch a small relevant region in one call.
- **Enriched region_summary/overview** — cities named with owner, units tallied per owner, resources **per
  type**. Exact on what/whose/how-many/how-rich; coordinates still lossy (leaves only).
- **Moving cache breakpoint** — marks the last transcript message each turn so the growing transcript is
  cache-read, not re-billed. Cuts $ (not token-count/latency).
- **Isolation (3-arm × 3-rep, majority-voted, paired McNemar; T677/p4, DeepSeek):** enriched summary is
  **~⅔ of the accuracy gain** (net +8, fixes dispersed-integration kinds: reachable/nearest-owned); scan_grid
  +nudge is the **efficiency lever** (−40% calls) that **alone regressed settle-site −3** (big-grid grab +
  early termination miscounts a per-candidate count; enriched exact summaries recover it). Combined Arm0→Arm2
  **net +12, b=0**. **Early-game reversal is ACCURACY-only** (interactive consensus 0.917 vs static 0.861;
  pooled CIs overlap; cost still ~13× tokens, ~9× round-trips). Corrected the noisy single-run A/B (true
  baseline **0.750**). **Noise floor ~12–18% per-item** (DeepSeek temp-0 non-determinism) → replicate + vote
  for any sub-0.05 accuracy claim.
- **Token decomposition:** the 13× is the **tool-loop re-send protocol × over-fetch (~2.5–3× the whole board
  fetched cumulatively)** — NOT a big overview (overview is ~1k tok, ~0.15× raw). 74% cached → ~7–8× in $;
  **latency is the unclosed cost**.

### 2. Cross-model (Gemini 2.5 Flash) — replicates, but confounded (`findings-fog.md`)
Early-game interactive collapse replicates on Gemini, but the think-off comparison is confounded by an
**agentic-effort asymmetry**: at the same `[nothink]`, DeepSeek works the tool loop ~13× harder (11.9 turns /
50.8 calls) than Gemini (3.8 / 7.3). "Think-off" is not behaviorally equal across models. **A think-ON
cross-model pass is the clean comparison — flagged priority.**

### 3. T3 valuation — t3-retreat + t3-threat (`T2-T3-design.md` §4.3)
- **t3-retreat** (retreat-tile dominance) merged.
- **t3-threat** (city threat/exposure) built + merged: **two Pareto axes** — incoming force (↑, `max`-aggregated
  ThreatField) vs own defense (↓, T2b `def_eff`); "most threatened" = the unique dominator; **decisive margin =
  on each axis a tie or ≥1.25× lead, ≥1 axis a genuine lead** (§4.3 aligned to this). Dense-board kind: T677
  ~12/25 kept, **T50 only 2/2821** (near-binary city set) — effectively T677-only.

### 4. LLM I/O tracer + observability (`civ-eval/src/trace.rs`)
- **Tracer** (default-on for live `run`, off for Oracle): one JSON per question under `traces/<run>/`, full
  per-turn request/reply/tool-calls/usage. Pure side-channel (Oracle byte-identical). **Standing directive:
  persist all LLM I/O** (memory `persist-all-llm-io`).
- **Observability decision** (`observability-tooling-decision.md`): emit **OTel GenAI spans → self-hosted
  Langfuse** (local, MIT), Phoenix as drop-in. No third-party cloud. Adapter build in flight (Python subdir).

### 5. Design notes (not yet built)
- `interactive-compute-forks-design.md` — the perception→operators→DSL externalization spectrum; the DSL
  (program-once) fork (data-dependent composition, not just batched calls); the benchmark-purification tension;
  a frozen-state **higher-reasoning question taxonomy** + **4 candidate kinds** (attack-target=**done as
  t3-threat**; **constraint-site**, **reach-race**, **triage: which threatened city can you save** — TO BUILD).
- `dynamic-ground-truth-sim-idea.md` — FreeCiv-as-oracle **T4** (probabilistic scoring needs many rollouts;
  ruleset/AI-idiosyncrasy confound). Far-future.

## ⏸️ WHERE WE LEFT OFF / open threads
Nothing running in the main dir. In flight (background agent, worktree): the **OTel→Langfuse adapter**.
Prioritized next steps (my read — see the 2026-07-30 handoff message for rationale):
1. **Diagnose interactive over-fetching with the new tracer** — one live traced run, read the actual tool
   sequences, confirm the ~2.5–3× over-fetch and find the redundant-fetch patterns. Cheap, newly-enabled,
   validates the tracer investment, and directly informs batch-fetch design.
2. **Compute-operator verbs experiment** — the sharpest science: decompose how much of the interactive deficit
   is *arithmetic* (fixed by operators) vs *genuine spatial reasoning*. Factorial: perception {static,
   interactive} × compute {none, operators}. See the forks note.
3. **Build the remaining higher-reasoning kinds** — constraint-site, reach-race, triage (all reuse
   ThreatField/ReachField/ChoiceSet). Deepens the benchmark as compute externalizes.
4. **Think-ON cross-model pass** — resolves the agentic-effort confound (validity).
5. **Frontier-anchor model** (Claude/GPT via OpenRouter) for the top tier point.

## Ops notes
- **Rebuild remote before every cloud run** (`cargo build -p civ-cli --features remote`).
- **`.env`** holds `OPENROUTER_API_KEY`; source it, never on the command line.
- **Replication discipline:** single DeepSeek runs have ~12–18% per-item flip noise; majority-vote ≥3 reps and
  use paired McNemar for any small accuracy claim.
- **Isolate levers/arms by pinned commit in a git worktree** (main dir stays free for parallel work).
- Traces now auto-write to `traces/` on every live run (git-ignored).
