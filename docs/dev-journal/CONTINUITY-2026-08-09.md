# CONTINUITY — 2026-08-09 EOD

Dated handoff snapshot. Durable state + command reference live in `RESUME.md`. The owner's
verbatim to-do list for next session is in `NEXT-SESSION-AGENDA-2026-08-09.md`.

## What landed today (all on master, every commit `just check` green — clippy · tests · verify-oracle ×4)

| Commit | Change |
|---|---|
| `746c1ea` | **ZOC-everywhere merged** — ZOC-aware reachability is the default for every owner-attributed movement/reachability tool + solver. Merged after owner reviewed the yield (nearest-owned 15% changed / 8.3% now-unreachable, reachable-nearest 8.9%/4.4%, reachability 0.6%; monotone). `threat`/ThreatField left enemy-blind by decision (keeps its consumers in mutual parity). |
| `044aa49` | **Viewer 7-improvements** — Board\|Analytics + Answer\|Tools tabbed windows; question/answer/reasoning collapsibles; legend reorder/prune; **real bug fix** — `lastReasoning()` was falling back to the answer text so "Reasoning" duplicated the answer (now only genuine reasoning shows); board edge coordinate labels; new **Stats/Compare tab** (filter-combined accuracy/latency/token aggregates: avg/median/max + 12-bin histogram, left/right compare mode). |
| `e68e242` | **M1/M2/M4 tool merges** (owner-approved tool-fit-v2 consolidations, lossless): `count_terrain`+`count_resource` → `count(kind,feature)`; `def_eff_base` folded into `def_eff(unit,bare?)`; `is_defensible`+`within_water_radius`+`not_enemy_territory` → `site_check(x,y,player)` (each conjunct separable + `ok`). Maxops surface **25 → ~19 verbs**, parity preserved; enum surfaces retire the single `count`. Updated dispatch/defs/docs/help, ~8 inline roster tests, run_tool parity gate, CLI help, viewer playground (+ a `bool` arg kind), WASM rebuilt. |
| `88b2583` | **region_summary center redesign** — the fixed `RrCc` sector addressing is GONE. `region_summary(x,y)` now summarizes the 10×10 window **centered on any point**, sliding-clamped inward at an edge (full 10×10 unless the board is smaller). New shared `civ_eval::region_window(board,cx,cy)` helper is the single source of truth, reused by the viewer's fetched-region overlay so the drawn box matches. `viewer_export::call_region` decodes the new `{x,y}` form and still falls back to `sector` for historical traces. Dead `parse_sector_id`/`Interactive::sector_bbox` removed. |
| `6717599` | **Viewer feedback batch** — removed the non-functional Replay/Stats view-tabs (Stats/Compare now renders permanently below the replay layout); added **Turns** + **Time-per-turn** (`latency_ms/n_turns`) aggregates (only questions with `n_turns>0`, so static encodings don't skew/÷0; both compare sides); moved prev/next **question** buttons into a `.qnav` between the board and Aggregated Stats; tool-call analytics now seeds the full `TOOL_SPECS` roster so **zero-call tools show a 0 bar**. |
| `5d01a48` | **Repo-wide `cargo fmt`** — formatting only, 0 remaining diffs. |
| `ffc26fd` | RESUME updated. |

**Owner's viewer feedback (6 items) — status:** #1 dead view-tabs removed ✅ · #2 turn/time-per-turn stats ✅ · #3 stale `region_summary` fixed via WASM rebuild ✅ · #4 arbitrary centered window ✅ (`88b2583`) · #5 prev/next repositioned ✅ · #6 zero-call tools shown ✅. (These were TODAY's viewer feedback — distinct from the SIX NEW items below for tomorrow.)

## Housekeeping notes
- **`just check` = `test lint verify-oracle` — it does NOT run `cargo fmt`.** The repo drifts from the local rustfmt over time (mismatched versions), so keep formatting as a periodic manual `cargo fmt --all` pass (did one today). Do NOT reflexively `cargo fmt --all` mid-feature — it touches dozens of untouched files.
- Pruned the 3 worktrees merged today (ZOC, viewer-improvements, viewer-feedback). 4 older worktrees remain (all on merged branches, from prior sessions) — left untouched.
- Untracked at repo root (pre-existing, not ours): `analysis/new 174.txt`, `package-lock.json`, `smoke-nonsat-export.json`.
- Before any cloud run: `cargo build -p civ-cli --features remote` (plain builds strip `remote`). Viewer WASM rebuild: `cd viewer && npm run build:wasm`. Viewer bundle: `npm run build`.

---

## TOMORROW — the owner's 6 items (verbatim in `NEXT-SESSION-AGENDA-2026-08-09.md`), with a head start

These are NEXT-session work. My notes below are scaffolding, not decisions.

**1. Review tools again; specifically get the LLM to use `region_summary` MORE and `get_tile` LESS.**
Ties to the tool-fit track (`analysis/tool-fit-analysis-v2.md`). Now that `region_summary` takes an arbitrary center (today's `88b2583`) AND is enriched, it should be a stronger substitute for pinpoint `get_tile` spam. Levers: (a) prompt/preamble nudges (the interactive overview now says "call region_summary(x,y) … centered anywhere"); (b) look at the tool-call analytics on a fresh interactive run to measure the get_tile:region_summary ratio. Needs a fresh live run to see if the redesign shifts behavior — pre-`88b2583` traces used the old sector form so they're not comparable.

**2. `scan_grid` skepticism — cheap experiment or deduce from existing data.**
Owner's hypothesis: `scan_grid` gives LESS info than `region_summary` → more turns. First try to answer from existing data (no spend): pull tool-call analytics per kind/encoding and compare, for the perception-heavy kinds, trajectories that used `scan_grid` vs `region_summary` on `n_turns` and correctness. `analysis/tool-fit-analysis-v2.md` already noted `scan` (42.9%, n=7) as the weak perception tool but that `scan_grid` (86.5%) looked fine — but that was kind-biased and pre-region_summary-redesign. If existing data is too thin/confounded, devise a minimal A/B: same kinds/boards, one arm with `scan_grid` withheld, compare turns + accuracy. Keep it cheap.

**3. Move Tool-Call Analytics OUT of the Board tab; show it at the BOTTOM of Aggregated Stats, including in Compare mode.**
Viewer change. Today's batch made analytics a tab alongside the Board (item 1 of the 7-improvements). Owner now wants it relocated under the Stats/Compare aggregates and to appear per-side in Compare mode. This overlaps with item 6 (below) — design them together. Files: `viewer/src/main.ts` (`renderAnalytics`, the Stats view render, compare-mode plumbing), `viewer/index.html`. No Rust/WASM.

**4. Consider putting `list_units` / `list_cities` CONTENT in the system prompt (blurs interactive-vs-raw — discuss first).**
Design discussion, not a build. The worry: it collapses part of what distinguishes interactive (query) from raw (front-loaded). Owner thinks it's justified for most cases. Decide the framing before touching encoders — likely a NEW surface variant rather than mutating `interactive`, so the clean interactive-vs-raw contrast is preserved for the blog. Discuss.

**5. `constraint-site` may be trivially "call these 3 tools 100% of the time" in interactive-maxops — analyze.**
Validity concern. `constraint-site` is scored on `is_defensible ∧ within_water ∧ not_enemy` — which today became the single `site_check` tool. In the last smoke, those 3 were called 63/58/55 times (≈always). If the question reduces to "call site_check and read it off," it isn't edifying. Analyze the traces: does the model actually reason, or just call+report? This also interacts with M2 — `site_check` bundling the 3 conjuncts makes the "just call the tool" path even shorter. May need a harder variant (more candidates, a decoy conjunct, or a none-proof that requires real search). Pull from the smoke traces first.

**6. Replay Viewer: show how often a tool is called multiple times across DIFFERENT turns for the same question.**
Viewer analytics feature. Emphasis: DIFFERENT turns (repeat calls of the same tool in the same turn don't count — this measures iterative/looping tool use across turns, a signal of over-fetch or converging behavior). Data is in each question's `trajectory` (per-turn `tool_calls`); the export already has per-turn structure. Compute, per question, for each tool: how many DISTINCT turns it appears in; surface the incidence (e.g. "% of questions where some tool is called in ≥2 different turns", and which tools). Design alongside item 3 (both are Stats/analytics-area viewer work). Relates to the known over-fetch failure mode (`findings-fog.md`, `findings-interactive.md`).

## Bigger unrun arc (unchanged from RESUME §THE BLOG PLAN)
The blog experiment is still the largest open work: claim #3 (raw-maxops-enum vs interactive-maxops-enum, the HEADLINE) has zero data and needs a budget top-up; #2 (maxops-enum efficiency) and #4 (clean raw vs interactive) fold into #1/#3. M3 (`tile_cover`+`support` → retreat_axes) stays deferred until a t3-retreat-inclusive run.
