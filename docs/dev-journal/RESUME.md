# RESUME — CivSpatial session continuity

> **➡️ START HERE (next session, 2026-08-24): the SUBSCRIPTION PATH is now production-viable — prompt
> caching recovered + validated at scale.** master HEAD = `4df3033`; tree clean except the pre-existing
> `viewer/src/sample-data.js` (not ours). The 08-19 block below is fully COMMITTED now (its stale
> "uncommitted" warning is resolved): `e3d5bd3` cf-vacate sampler+finding, `521c0a1` reachability oracle
> fix, `079d476` findings+RESUME docs.
>
> **2026-08-24 headline — running the eval through a Claude Max SUBSCRIPTION (via CLIProxyAPI) now works
> end-to-end, cheaply.** Setup/caveats: **`scripts/subscription-proxy-setup.md`** (committed `01044b5`
> with the toolkit: `run-subs-arm.sh`, `subs-ping.sh`, `cliproxy-config.yaml`). Proxy = CLIProxyAPI at
> `localhost:8317`, OpenAI-compatible, wraps the user's Claude Max OAuth; harness hits it exactly like
> OpenRouter (3 flags: `--base-url http://localhost:8317/v1 --api-key civspatial-local --model <id>`).
> Agents can RUN but CANNOT set it up (interactive OAuth login is user-only) — always `curl -s -m5
> localhost:8317/v1/models` first; if down, ask the user to start it.
> - **CACHING RECOVERED (`c0b8cd6`) — the key fix.** The maxops/interactive arms used to 400 (harness put
>   `cache_control` inside `tool_result.content`; native Anthropic needs it ON the `tool_result` block).
>   The remote encoder now emits a message-level breakpoint off-OpenRouter. **Verified ~99% cached warm on
>   BOTH raw and maxops, on Haiku AND Opus 4.8/5.** So a subscription 3-arm run bills ~22–26M cap-equiv,
>   NOT the ~176M uncached gross. **Use `--cache on`** (run-subs-arm.sh still defaults CACHE=off → override).
> - **THINK mechanics on this proxy:** both `--think off` AND `--think on` map to thinking-DISABLED for
>   Claude (the binary toggle is inert here). Extended thinking is reachable ONLY via
>   `--reasoning-effort low|medium|high` (a real graded flag, overrides `--think`). Opus 4.8/5 think-off
>   returns non-empty, no error. So the subscription default is the CHEAP think-off condition; think-on is
>   opt-in and much heavier.
> - **EXPLORATORY Haiku 3-arm run (Max, think-off, cache-on)** — caching validated at scale (84% cache-read,
>   ~22M cap-equiv, under estimate). Arms BARELY separate: **raw 0.704 / raw-maxops 0.776 / roster 0.748**
>   (CIs overlap) — UNLIKE DeepSeek's crush. Note raw≈DeepSeek's raw (0.71); it's the MAXOPS arms that
>   underperform. **EXPLORATORY ONLY** (hidden Claude Code system prompt; NON-deterministic at temp 0
>   ~64–73%; Haiku≠Opus). Isolated as `results-subs-arm-*.jsonl`; NEVER merge into `results-3arm-*.jsonl`.
> - **Harness note (was a false alarm):** the "non-ASCII `--resume` bug" does NOT exist — item_ids
>   (incl. "François Mitterrand", `ç`=C3A7) round-trip losslessly (proven on the real binary+board:
>   `--resume` skipped all rows, re-ran 0). The Haiku-run duplicates were **two overlapping invocations
>   writing one `--out` file** (op issue, already mitigated by `033f64c` = one file per board/seed/model);
>   the "U+FFFD" was a PowerShell display artifact. Shipped anyway: BOM-tolerant `parse_rows` + non-ASCII
>   round-trip regression tests (`f84554f`). Real remaining harness caveat: subscription path is
>   non-deterministic at temp 0 (`fcccc5d`).
> - Cross-model confirm: Opus 4.8 + Opus 5 both reachable on this subscription and both do think-off+caching.
>
> **⚠️ NEW VALIDITY FINDING (from the Haiku gap analysis) — TWO ANSWER-EXTRACTOR BUGS likely affect the
> PUBLISHED DeepSeek 3-arm numbers.** The small Haiku raw↔maxops gap was NOT model weakness / NOT
> tool-ignoring — it was measurement artifacts: (1) **verbose two-entity "which city" answers mis-extract**
> — on the fogged late board the model says *"Caen is better defended … versus Perm whose defense is hidden"*,
> picks Caen (correct), but the extractor recorded `got="Perm"` (2nd-named/fogged city); re-scored, Haiku
> city-defense maxops = 1.00/0.97 (beats raw). (2) **markdown/paren coord mismatch** (`**83, 39**` vs
> `(83, 39)`) on forward-posting/coord kinds. Corrected, Haiku's tool gap = **+0.12–0.14 ≈ DeepSeek's** (raw
> 0.71 → maxops 0.83/0.86 excl cf-vacate). Residual genuine Haiku weakness = small *under-exploration* on
> search-heavy kinds (constraint-site ~15 tool calls vs DS ~100) = the lazier-agentic-disposition
> ([[deepseek-gemini-tool-engagement]]), not tool inability. **These extractor bugs probably hit
> city-defense/compare/forward-posting in the CLEAN DeepSeek run too → the C1 tool arms may be UNDERSTATED.**
>
> **RESOLVED (`4df3033`) — extractor fixed + clean run re-scored.** Both bugs fixed CONSERVATIVELY in
> `scoring.rs` (`match_option` picks the declared winner on two-entity answers; `norm` strips markdown +
> canonicalizes coord formatting), 32/32 scoring tests, `just check` green. Re-score fidelity proven (pre-fix
> extractor reproduces all 954 recorded statuses exactly; new offline `civ rescore` subcommand). **Clean run:
> 4 rows flip wrong→correct — ALL city-defense, ALL in the tool arms, ZERO regressions (correct→wrong = 0):**
> city-defense raw-maxops 27→30/32 (0.94), roster 31→32/32 (1.00). **Excl-cf-vacate aggregate: raw-maxops
> 0.938→0.948, roster 0.955→0.958, raw unchanged 0.712 → C1 gap now +24–25 pts (STRONGER).** Haiku byproduct:
> +26 recoveries (14 wrong→correct + 12 invalid→correct, both bugs — Haiku uses `**x,y**` markdown coords),
> 0 regressions, interactive 0.785→0.835. `analysis/findings-3arm-clean-run.md` updated ("Post-run correction
> #2"), stacking on the reachability correction.
>
> **NEXT:** decide the real run — Gemini C5 (OpenRouter, clean) vs. an Opus-on-subscription arm (cheap now that
> caching works, exploratory-only) → then the blog. Clean/citable arms stay on OpenRouter.
>
> ---
>
> **➡️ Prior handoff (2026-08-19): the 3-arm clean run is EXECUTED + ANALYZED; cf-vacate
> RESOLVED.** (All committed — see `e3d5bd3`/`521c0a1`/`079d476`.) `civ-eval/src/generators.rs`
> (cf-vacate balanced-sampler fix + 2 tests, `just check` green), `analysis/run-preregistration-3arm-2026-08-17.md`
> (§12 deviation log), new `analysis/findings-cfvacate-tooltrap.md` (blog exhibit). (`viewer/src/sample-data.js`
> was already modified at session start — not ours.)
>
> **2026-08-18 headline — THE 3-ARM CLEAN RUN LANDED (DeepSeek-only; Gemini deferred on cost, `ca34bdd`).**
> 954 trials, hardened harness. **Post-correction numbers (reachability oracle bug fixed + re-scored, see
> below): raw 0.695 · raw-maxops 0.871 · roster-maxops(=interactive-maxops) 0.881 as-run; excl cf-vacate
> 0.712 / 0.938 / 0.955.** (Pre-correction was 0.720 / 0.833 / 0.852.) Full writeup:
> **`analysis/findings-3arm-clean-run.md`**.
> - **C1 CONFIRMED (tools = compute lever):** raw ≪ maxops on decision kinds (0.739 → 0.833/0.851) and
>   crushingly on movement (0.594 → 0.97/0.94). The two maxops arms are statistically tied.
> - **C2 CONFIRMED (cost frontier — the money result):** roster-maxops ties raw-maxops on accuracy but at
>   **2.5× fewer total tokens** (146k vs 367k/ans) and **2.6× fewer per correct** (172k vs 442k). Full
>   front-loading of the masked board is strictly dominated.
> - **C3 NOT supported (after fix):** raw perception is WEAK on both — region-count-raw 0.56 AND
>   reachability-raw 0.62 (was a spurious 0.875 vs the buggy oracle). A "perception wall"; tools rescue both
>   (0.94–1.00) → folds perception into C1.
> - **C4 softer than predicted:** the "survivors" saturated under maxops (constraint-site 32/32,
>   compare-two-attacks 30/32) instead of staying ~0.75–0.9 — consistent with the ceiling theorem, weaker
>   as a withheld-tool exhibit.
> - **C5 unrun** (Gemini deferred). Main story is clean + single-model.
>
> **cf-vacate — the one anomaly, now RESOLVED into a publishable finding.** Original inversion (raw 0.53 >
> raw-maxops 0.23 > roster 0.17) was an **all-"no" corpus artifact**: a 216-save oracle sweep proved BOTH
> frozen boards have **zero yes-capable cities** (every threatened city is single-defender → vacate always
> falls). Two agents: (1) traced it to a genuine tool-trap — the maxops `city_fall_prob` reports the
> **current** garrison (≈0 when secure), but the question is a **post-vacate counterfactual**; the model
> over-trusts the stale signal and flips "no"→"yes". (2) Fixed the sampler to balance yes/no (oracle + tools
> untouched), EXCLUDED cf-vacate from the main cross-kind table as corpus-degenerate (deviation logged), and
> re-ran it **standalone** on a yes-capable board (`corpus_medium_a9_s1337` fog 4, 8y/8n). **Balanced-set
> result:** aggregate inversion **DISSOLVES** (raw 0.625 / raw-maxops 0.500 / roster 0.562, CIs overlap);
> the real effect is an **answer-value asymmetry** — tools HELP on YES (roster 8/8) but HURT on NO (3/8 →
> 1/8), yes-rate climbing 0.75 → 0.88 → 0.94. Data: `results-cfvacate-aux-medium-a9-s1337-*.jsonl`. Full
> writeup + trace quotes: **`analysis/findings-cfvacate-tooltrap.md`** (blog-ready).
>
> **Main findings doc WRITTEN + CORRECTED: `analysis/findings-3arm-clean-run.md`** (post-correction numbers,
> C1–C4, cost frontier, per-kind table, blog takeaways).
>
> **reachability — 2nd apparent inversion, ROOT-CAUSED as an ORACLE BUG (opposite of cf-vacate: the tools
> were RIGHT, our ground truth was wrong).** The reachability oracle's BFS (`question.rs` `reachable()`)
> forbade only Mountains + tactical ZOC and **never checked `is_land`, so its truth path could walk across
> ocean.** On water-crossing items the oracle said "reachable" but land-only `travel_turns`/`reach_turns`
> (and the model) correctly said "unreachable" → scored wrong vs an impossible truth; raw looked good only by
> matching the bug. **Fixed** (`is_land` guard + regression test, commit **`521c0a1`**, `just check` green),
> **re-scored the recorded answers (NO re-run, free)**: 14/32 items flipped expected yes→no, inversion
> **reverses** — reachability raw 0.875→**0.625**, raw-maxops 0.516→**0.94**, roster 0.625→**0.94**. Now
> SUPPORTS C1. Deviation logged (prereg §12). Methods win: cross-surface disagreement caught our own bug.
>
> **NEXT:** (1) **commit** RESUME + `findings-3arm-clean-run.md` (uncommitted docs; code fixes already
> committed — cf-vacate `e3d5bd3`, reachability `521c0a1`); (2) decide **Gemini C5** (run vs publish
> DeepSeek-only w/ deferral noted); (3) consider rebalancing the reachability goal-band (now 6y/26n after the
> ocean fix — a *future* fresh run, not a validity issue); (4) Saturday publication push (repo public +
> viewer on GH Pages + blog). master `521c0a1`; RESUME + main findings doc are un-committed working tree.
>
> ---
>
> **➡️ Prior handoff: read `CONTINUITY-2026-08-13.md` + `analysis/reasoning-frontier-vs-luck.md`.**
> master = `d904de7`; `just check` green; nothing running.
>
> **2026-08-13 EOD headline: a CONCEPTUAL-BREAKTHROUGH day.** By a new diagnostic (**reasoning-bound vs
> information-bound** — *"would 2× intelligence cross the plateau?"*), **all THREE fog / hidden-info kinds
> turned out to be LUCK, not a reasoning frontier**: surprise-strike (redesigned deducible → **SATURATES
> 8/8**, `a0a4fd3`), fogged-assault (observed-repertoire, `d904de7` → model scores *below* the majority
> baseline reasoning prudently), hidden-force (**AUDITED = luck**, perturbation test flips the answer on a
> byte-identical masked board — branch `worktree-agent-ace542d2a5e081e20`/`d527fd1`, UNMERGED). Chasing a
> fix surfaced the **CEILING THEOREM**: any deterministic function of a fully-observable frozen board is
> tool-computable by construction, so "reasoning-bound difficulty" on a frozen board is only ever *the tool
> we withhold*; genuine tool-resistance needs **luck (rejected) / intractability / time-horizon** — no free
> lunch. **DECISION: path (a) — own the microscope's ceiling** (don't chase an un-tool-solvable frozen
> kind). A 2-kind live test proved it (surprise-strike 8/8; fogged-assault 1/8 raw / 3/8 roster;
> roster-maxops beat raw-maxops 11/16 vs 9/16 at ~2.6× fewer tokens). **Re-scoped experiment: `raw` vs
> `raw-maxops` vs `roster-maxops`** (fog matrix CANCELLED) — raw≪maxops = accuracy ablation (tools =
> *compute* lever), raw-maxops vs roster-maxops = COST frontier. **`interactive-maxops` → presented as
> `roster-maxops`** (label only, no code rename). Fog three RETIRE from the run → ceiling-theorem exhibit.
> **NEXT:** merge audit artifacts → amend preregistration (document the validity-based exclusion) → the
> vs-luck methods doc (drafted) → small production edit dropping fog kinds → the 3-arm clean run → blog.
> Full detail: `CONTINUITY-2026-08-13.md`. Path (c) bounded rollouts = the real post-Saturday frontier.
>
> ---
>
> **➡️ Prior handoff (2026-08-12): read `CONTINUITY-2026-08-12.md`.** master was `ba0ff61`; `just check`
> green; nothing running. **The plan is PARKED for owner review — run tomorrow.**
>
> **2026-08-12 EOD headline: a HARNESS-HARDENING + PUBLICATION-PLANNING day; the big experiment was
> DE-RISKED, not run.** Two audit agents swept for validity confounders (fog/parity/fidelity/determinism
> all CLEAN) and prompt errors; we fixed everything + a live-validated smoke: (1) **7 prompt fixes**
> (`2ea027e`). (2) **C1 — a MAJOR confounder fixed:** the interactive `token_budget` re-counted the cached
> re-sent board every turn, so raw-maxops tripped the 200k cap at ~turn 3 while interactive got its full
> 16 — **all prior raw-vs-interactive numbers were budget-confounded.** Fixed to cache-deduct + a 2M silent
> backstop so **turns govern both surfaces**; smoke confirms raw-maxops **3→17 turns** now (`bdde824`). (3)
> **C2** off-menu-tool guard (matters for the Gemini run), **S1** require `Answer:` line for all answer
> types, **S2** value-beats-"none" scoring (`bdde824`). (4) **Resume + incremental flush** (crash-safe,
> 6-key `--resume`/`--fresh`); a **model-key bug** the smoke caught (live rows persist `name()`="…[nothink]"
> but the key used the bare `--model` flag → live resume matched nothing and neither-flag silently
> truncated+re-spent) fixed + regression-tested (`ba0ff61`), verified live. (5) **First CLEAN experiment
> matrix + cost drafted, PARKED:** 8 kinds × 2 enc × 2 boards × per-kind 8 ≈ 256 trials, **~$5/seed** (money
> is a rounding error — **wall-clock is the constraint**, ~1–3 h/seed; C1 ~tripled raw's time). Do the
> **`--per-kind 1` pre-flight** first. (6) **Strategic/thesis work (OWNER-OWNED — do NOT declare it
> "locked"):** the two-pole "squeeze" (Pole A = our eval microscope; Pole B = **Vox Deorum**, researched —
> LLM-as-tuner converges to the scripted AI, world-model verified THIN [no wonder tool, no timing
> lookahead]); Saturday publication push (repo public + viewer on GH Pages + README + blog). **NEXT:** owner
> reviews the plan → per-kind-1 pre-flight → full matrix → Saturday deliverables. Full detail:
> `CONTINUITY-2026-08-12.md`.
>
> ---
>
> **➡️ Prior handoff (2026-08-11): read `CONTINUITY-2026-08-11.md`.** Everything from 2026-08-11 is
> committed to master; tree clean.
>
> **2026-08-11 EOD headline: live-validation, a SHIPPED budget-aware loop, a scan_region FOG-FIDELITY
> FIX, and interactive-maxops slimmed to 16 tools.** (1) **constraint-site Variant C CONFIRMED LIVE** —
> 15/15, trace proves the model computes the hidden ≥3 spacing constraint as a SEPARATE step (415
> `site_check` + 84 `distance`); the "call site_check and read off" shortcut is defeated
> (`findings-constraint-site-variantC-live.md`). (2) **2nd-board replication** on a LARGE board —
> accuracy-when-answered replicates; a first "tool-removal causes thrash" read was CORRECTED on drill-down
> to: forward-posting is budget-marginal at scale BY DESIGN (adversarial 1-ply search, no shortcut
> primitive; ~13/16 turns even with the full menu) (`findings-scanregion-board2-replication.md`). (3)
> **BUDGET-AWARE LOOP SHIPPED** (`runner.rs` + test): system prompt states the actual `max_turns` + the
> priority **never-run-out > correct > efficient**, and every tool result is stamped `[turn n/N]`. Works —
> large-board forward-posting non-answers **3→1**, turns ~14→~12, cap never hit, accuracy held. **This is a
> RE-BASELINE: prior interactive accuracy numbers carry an old-prompt asterisk.** (4) **scan_region FOG
> FIDELITY FIX + scan_grid DROPPED → 16 tools.** scan_grid's only edge was a hidden-force accuracy bump
> (8/8 vs 6/8); the owner pushed "was scan_grid actually USED on the missed items?" → matched-item + trace
> analysis found the real cause: `scan_region` was DISCARDING fogged-but-explored tiles as "Unknown"
> (`known = terrain!="Unknown" && !is_fogged`), so the model misread fogged enemy LAND as empty ocean.
> Fixed scan_region to LIST fogged tiles (terrain/owner/(fogged), units hidden); re-test → **DROP now
> MATCHES KEEP on hidden-force both boards** (MED 8=8, LRG 7=7), so scan_grid earns nothing → **dropped**
> (`findings-scangrid-drop-budgetaware.md`). **NEXT:** cross-model constraint-site (Gemini — is Variant-C's
> difficulty model-dependent?); an OPEN thread — does a higher `max_turns` erase the large-board
> forward-posting non-answer margin (would confirm it was budget, not scan_grid); frontier kinds may want a
> bigger budget at scale. **Blog #3 is DEPRIORITIZED** (see THE BLOG PLAN below).
>
> ---
>
> **➡️ Prior handoff (2026-08-10): read `CONTINUITY-2026-08-10.md` + the full experiment writeup
> `analysis/findings-region-summary-arms.md`. Everything from 2026-08-10 is MERGED to master (`69b1197`);
> tree clean.**
>
> **2026-08-10 EOD headline: a big interactive-maxops PERCEPTION-SURFACE session, all merged & green
> (verify-oracle ×4).** Split perception into an **occupant layer (front-loaded roster, standard now)** and a
> **terrain layer (`region_summary`→`scan_region`, a bounded-10×10 per-tile terrain list)**; killed the quadrant
> **double-listing**; then **B-STYLE tool consolidation** on interactive-maxops (**19→17 tools**: dropped
> `get_tile`+`scan`, kept `scan_grid`+`scan_region`; execute logic retained for other surfaces + the parity gate).
> Also: **constraint-site → Variant C** (region search + non-bundled spacing constraint, defeats "call site_check
> and report" — **needs a live run to confirm**), a real **`--withhold` prose-scrub confound found+fixed** (it was
> filtering only the tool array, not the prompt → phantom-tool thrash), and **two viewer batches** (analytics moved
> under Aggregated Stats + cross-turn repeat metric + qnav moved; compact 2-col stats + 6th "Tool calls" block).
> **Key findings (`findings-region-summary-arms.md`, DeepSeek/corpus_medium/fog p1, n=39/arm/seed, 2 seeds):**
> region_summary's 0-uptake was the OLD sector design (redesign fixed it); the withhold confound FLIPPED item 2;
> roster front-loading is a real efficiency win (−33% get_tile, accuracy-neutral); **tool value is kind-specific** —
> get_tile is a wasteful crutch (forward-posting, pooled C=A) while scan_grid earns its place on the wide-area fog
> kind (hidden-force, repeatable −1/8 without it); models won't self-select the efficient tool (406 get_tile even
> with scan_region present). **NEXT:** (1) constraint-site Variant C **live run**; (2) the BLOG HEADLINE
> (raw-maxops-enum vs interactive-maxops-enum, still zero data, needs budget top-up); (3) replicate the
> scan_region findings on a 2nd board before write-up. **All 2026-08-09 items (the owner's 6-item agenda) are
> DONE** — see below.
>
> ---
>
> **Prior handoff (2026-08-09): read `CONTINUITY-2026-08-09.md` + `NEXT-SESSION-AGENDA-2026-08-09.md`. That day's
> work is MERGED (`5d01a48`).** The owner's 6 to-do items for next session (details + head-start analysis in the
> continuity file): (1) **review tools** — push the LLM toward `region_summary`, away from `get_tile`
> (region_summary is now arbitrary-center + enriched, so it should substitute for get_tile spam; needs a fresh
> interactive run to measure). (2) **`scan_grid` skepticism** — cheap experiment or deduce from existing data
> whether it earns its place (owner suspects it gives less than `region_summary` → more turns). (3) **Viewer:
> move Tool-Call Analytics** out of the Board tab to the bottom of Aggregated Stats, incl. Compare mode. (4)
> **discuss** putting `list_units`/`list_cities` content in the system prompt (blurs interactive-vs-raw — likely
> a new surface variant, not mutating `interactive`). (5) **analyze `constraint-site` validity** — if it's just
> "call site_check 100% of the time" it isn't edifying (M2 shortened that path further). (6) **Viewer analysis:**
> how often a tool is called across ≥2 DIFFERENT turns for the same question (over-fetch signal). Items 3 & 6 are
> viewer/Stats-area work — design together.
> **After the 6:** the BLOG EXPERIMENT is still the biggest unrun work (§THE BLOG PLAN below): #3
> raw-maxops-enum vs interactive-maxops-enum (HEADLINE, zero data, needs a budget top-up); M3 stays DEFERRED.
>
> **2026-08-09 EOD headline: four things LANDED on master, each `just check` green (verify-oracle ×4).**
> (1) **ZOC-EVERYWHERE MERGED** (`746c1ea`) after owner reviewed the yield report (nearest-owned 15% changed /
> 8.3% now-unreachable, reachable-nearest 8.9%/4.4%, reachability 0.6% — monotone; `threat`/ThreatField left
> enemy-blind by decision). (2) **VIEWER 7-IMPROVEMENTS MERGED** (`044aa49`): Board|Analytics + Answer|Tools
> tabbed windows, question/answer/reasoning collapsibles, legend reorder/prune, **a real bug fix** (reasoning
> was falling back to the answer text — `lastReasoning()`), board edge coordinate labels, and a new
> **Stats/Compare tab** (filter-combined latency/accuracy/token aggregates: avg/median/max + 12-bin histogram,
> left/right compare mode). (3) **M1/M2/M4 TOOL MERGES committed** (`e68e242`): `count_terrain`+`count_resource`
> → `count(kind,feature)`; `def_eff_base` folded into `def_eff(unit,bare?)`; `is_defensible`+
> `within_water_radius`+`not_enemy_territory` → `site_check(x,y,player)` (each conjunct separable + `ok`). Maxops
> surface 25→~19 verbs, parity preserved; enum surfaces retire the single `count`; run_tool parity gate + ~8
> inline roster tests + CLI help + viewer playground (+ a `bool` arg kind) updated; WASM rebuilt. (4)
> **region_summary REDESIGNED** (`88b2583`): the fixed `RrCc` sector addressing is GONE — `region_summary(x,y)`
> now summarizes the 10×10 window CENTERED on any point, sliding-clamped inward at an edge (shared
> `civ_eval::region_window` helper, reused by the viewer fetched-region overlay; historical sector traces still
> decode; dead `parse_sector_id`/`sector_bbox` removed). **NOTE:** `just check` = `test lint verify-oracle`; it
> does NOT run `cargo fmt` — the repo has a repo-wide rustfmt-version drift, so don't `cargo fmt --all` (huge
> unrelated churn). **PENDING background agents from 2026-08-08 are RESOLVED** (ZOC merged; tool-fit-v2 was
> already merged).
>
> **2026-08-08 EOD headline: a large parallel-agent session MERGED TO MASTER (`f34f41d`, `just check` green,
> verify-oracle ×4 PASS).** Landed: (1) **VALIDITY FIXES** — fogged-assault re-scored WORST-CASE-ROBUST
> count=1 (owner's strongest fielded defender; balanced **16y/22n/11drop across 18 boards — NOT all-no**) +
> tool-fidelity (fogged city → "unknown"); forward-posting enemy-city exclusion + enemy-aware
> `ReachField::from_origins_tactical` (entry-block + full classic ZOC — verified (70,39) now unreachable);
> unlimited-land-stacking prompt note. (2) **VIEWER/WASM** — tool playground (invoke any tool via WASM
> parity), tool-call analytics panel+filters, per-question latency/tokens, full-title + copy button, city-walls
> export fidelity. (3) **region_summary ENRICHED** (unit types/positions/att-def sums, city pos/size/walls,
> resource positions; overview stays coarse). Smoke export regenerated + viewer sample rebundled (all panels
> live). **⏳ PENDING BACKGROUND AGENTS — check their worktree branches/reports first thing:** (a) ZOC-EVERYWHERE — **DONE**, branch
> `worktree-agent-a66be68deb997d4e4` (`75ab398`), `just check` GREEN (verify-oracle ×4). ⚠️ **HELD FOR OWNER
> REVIEW of the yield report before merge:** nearest-owned 15%/8.3%-now-unreachable, reachable-nearest
> 8.9%/4.4%, reachability 0.6% — MONOTONE (ZOC never opens a new path; strict subset), stricter-not-broken.
> `threat`/ThreatField left enemy-blind (documented — keeps all its consumers in parity); `travel_turns` got an
> optional `player` arg; reachability solver threads perspective. **On owner OK: merge `75ab398` → then implement
> M1/M2/M4** (they edit encoders.rs tool-defs — do after this).
> (b) TOOL-FIT-V2 — DONE + merged (`analysis/tool-fit-analysis-v2.md`): 30 runs/1183 traces + structural map; net 25→~19 verbs, only clean remove = `list_owned_units`; `region_summary` dead on maxops (but that's PRE-enrichment data — re-check); M3 upgraded to HOLD (tile_cover dual-used by forward-posting); confirms M1/M2/M4; NO adds (explicitly against a fog-aware tool). Review tomorrow.
> **✅ APPROVED, implement after ZOC merges (both edit encoders.rs tool-defs — sequence, don't parallelize):**
> M1 (`def_eff` absorbs `def_eff_base`), M2 (is_defensible+within_water_radius+not_enemy_territory →
> `site_check`), M4 (count_terrain+count_resource → `count`). **DEFERRED:** M3 (tile_cover+support → retreat_axes
> — too specific; revisit on a retreat-inclusive run). **Roster context:** `a556c608` first-pass proposal;
> **`scan` (42.9%, n=7) not `scan_grid` (86.5%) is the weak perception tool** — but that's kind-biased, needs a
> full-kind run; region_summary REMOVAL is now MOOT (enriched version earns its place).
>
> **➡️ Prior handoff: read `CONTINUITY-2026-08-07.md` first** — the fogged experiment went REAL. master green.
> **2026-08-07 headline: (1) integrated the 3 pending branches (coherence → P4-with-perspective → corpus), pruned
> worktrees; (2) selected the fogged 15-board corpus via a YIELD-DRIVEN optimizer (`scripts/corpus-yield.py` +
> `select-corpus.py`; frontier-kind support is board-idiosyncratic, not band-driven — lifted every discriminating
> kind to 10–15/15; committed `data/corpus/selection-optimized.txt`); (3) PRE-REGISTERED the kind roster, FROZEN at
> 19 kinds (`analysis/kind-roster-preregistration.md` + `-analysis.md`); (4) BUILT the reasoning-frontier
> HIDDEN-INFO family — unmasked-board seam (`GenCtx`) + P7f + P1 + P3 (`6eb2b38`, `c571c1c`, `161a578`), synthetic
> tests in `civ-eval/tests/fog_hidden_force.rs`; (5) ran the FIRST live DeepSeek smoke — $0.62 — showing the frontier
> kinds resist maximal tooling (forward-posting ~0.40, fogged-assault 0.43–0.71 on BOTH surfaces) and interactive ≥
> raw-maxops (0.889 vs 0.774), `analysis/findings-smoke-nonsat.md`.**
> **⚠️ START HERE TOMORROW (2026-08-08):** (a) analyze the smoke misses in the REPLAY VIEWER from
> `traces/results-smoke-nonsat-*`; (b) start an agent to classify frontier-kind failure modes in parallel
> (model error vs. litigable decisive-band vs. extraction artifact); then replicate ×3 boards/seeds + pre-register
> the full matrix. Remember: `cargo build -p civ-cli --features remote` before any cloud run.
>
> **➡️ Prior handoff: read `CONTINUITY-2026-08-05.md`** — three-state fog + viewer polish merged; a new
> REASONING-FRONTIER question track designed (kinds no simple tool solves at 100%); P4 built. master = `048ef7a`.
> **2026-08-05 headline: the two 2026-08-04 PENDING items are DONE + merged — (1) faithful three-state fog with
> real classic `vision_radius_sq` (additive/gated behind `--fog`, non-fog baseline unchanged; + rulesetdir gate +
> fog-perspective guardrail), (2) viewer polish (nation labels, full-prompt panel, layout, unfogged sample).**
> Then a design pivot: **reasoning-frontier kinds** built on three tool-resistance engines (hidden-info via fog /
> underdetermined objective / intractable) and the **luck↔tool-resistance duality** (provably-safe decoys +
> decisive answers + lift-over-baseline@large-N). Docs: `analysis/reasoning-frontier-questions{,-v2}.md`,
> `QUESTION-CATALOGUE.md`. **P4 (forward-posting) BUILT + green.**
> **⚠️ THREE UNMERGED BRANCHES → one clean civ-eval integration tomorrow (see CONTINUITY-2026-08-05 §PENDING):**
> `feat-fog-perspective-coherence` (implemented, NOT yet committed — merge FIRST) → `feat-p4-forward-posting`
> (`8618a7a`, thread `perspective` in during merge) → `corpus-selfplay-tooling` (sweep running to completion,
> then manifest + 5/band selection). **New constraints:** experiment will be FOGGED; turn-history needed for
> future saves (current sweep has none); per-kind board-support varies (boards can yield 0).
>
> **➡️ Prior handoff: read `CONTINUITY-2026-08-04.md`** — the fidelity-hardening + replay-viewer session.
> **2026-08-04 headline: a full harness-fidelity pass is MERGED and green (parity ops · movement road/rail Dijkstra ·
> combat win-probability · t3-threat single-axis + adv-assault-target {capture,size} + `city_fall_prob` op ·
> tracer run-manifest + reasoning capture · docs correction). A Phase-1 REPLAY VIEWER was built + merged (`viewer/`,
> TS canvas + Freeciv trident sprites, browser-verified).** master = `12fb015`, `just check`+`verify-oracle` green (636).
> **⚠️ TWO THINGS RE-OPEN before the experiment (see CONTINUITY-2026-08-04 §PENDING):** (1) **FOG must match the real
> game = THREE-STATE fog** (fogged hides live occupants; touches solvers — model-view & oracle must share visibility);
> DECIDED, not built. (2) **Viewer polish** (owner colors+legend [confirmed collision], fog toggle, marker legend).
> Still pending: finalize kind roster + re-pre-register matrix; budget top-up; multi-board corpus (ruleset-gated).
>
> **➡️ Prior handoff: read `CONTINUITY-2026-08-02.md`** — the survivor-experiment + tool-parity session.
> **2026-08-02 headline: "more tools" is NOT the lever — TOOL–TASK FIT is.** (1) On decision kinds, generic spatial
> ops don't help (no-ops 0.64 ≈ spatial-ops 0.61 ≪ maxops 0.92) — only the *matching* combat/valuation ops win
> (survivor experiment, `analysis/findings-tool-parity.md`). (2) A tool surface must be parity-complete with the
> solver, or the model shortcuts to a wrong heuristic — proven end-to-end: added the missing multi-source primitive
> **`travel_turns_from_owned`** and the residual hard kind **nearest-owned jumped 0.667 → 1.000** (23/23, cheaper,
> fewer turns). Merged to master (`067be07`); `just check` + `verify-oracle` green (T50 + T677 fogged), 100 lib tests.
>
> **➡️ THE BLOG PLAN — the 4 publishable claims (status):**
> 1. **maxops ≻ ops (tool–task fit)** — ✅ **DONE** (survivor experiment; `findings-tool-parity.md`). The parity-gap
>    fix (nearest-owned 0.667→1.000) is a strong companion mini-result.
> 2. **maxops-enum vs maxops** — ⏳ UNRUN. Add `raw-maxops-enum` as a 4th arm on the 6 decision kinds × 3 seeds.
>    Expect *no accuracy gain, ~2× efficiency* (from the calc-vs-calc-enum analog) — frame on the 2D cost frontier.
> 3. **raw-maxops-enum vs interactive-maxops-enum (was HEADLINE)** — ⏳ UNRUN, **DEPRIORITIZED 2026-08-11.**
>    The project's center moved past a surface bake-off: the current, nearly-complete story is tool–task fit +
>    frontier-resistance + won't-self-select + budget-awareness + the scan_region fog-fidelity work. #3 is now a
>    **cost-frontier appendix, budget-permitting** — not the critical path. (Still zero data on either side; needs a
>    budget top-up if resurrected.)
> 4. **clean raw vs interactive** — ⏳ UNRUN. Post-B1 rerun; REPLACES the suspect pre-B1 numbers (raw-ops 0.986
>    small / 0.736 dense) which were measured on the broken harness and must not be published as-is.
> Sequencing: #2 folds into #1's sweep; #4 folds into #3. Whole-blog clean spend ≈ $12–18 → top up budget before #3.
>
> **➡️ Prior handoff: `CONTINUITY-2026-08-01.md`** — the harness-hardening + clean-ablation session.
> **2026-08-01 headline:** an eval-validity bug sweep found two result-corrupting bug classes — **unit STACKING**
> (dense-board render dropped all-but-one of a tile's ≤37 units; operators resolved the wrong one) and
> **answer-extraction fabricating `got`** from prompt/trailing text. Both FIXED (B1 render-all-units +
> operators-by-id, on the `maxops-enum` branch; B2/B3/B4/B6 harness batch, merged to master). The clean rerun
> **overturns the enumeration story**: the calculator/arithmetic is the accuracy lever (0.54→0.83); **enumeration
> adds NO accuracy, only ~2× efficiency** — its prior accuracy lift was the render bug in disguise. Residual hard
> kind = nearest-owned (enumeration makes it *worse* via exclusion brute-force → cap-outs). See
> `analysis/findings-clean-ablation.md`, `findings-bug-sweep.md`. **Status (EOD): everything is MERGED TO MASTER** —
> the full maxops line + B1 stacking fix (`b2b9f08`), and a new **positional-filter** for nearest-owned
> (`list_tiles` + `exclude_owner`/`exclude_radius`, `4f5a1b3`); master green (98 lib tests, verify-oracle both
> boards/tiers/fogged), all encoders now render stacked units. **Immediate next step:** a small nearest-owned
> re-run (`raw-calc-enum`, T677/p2) to confirm the filter collapses the over-engagement cap-outs (NOT yet run,
> ~$0.5). Biggest unrun thread: the **survivor/decision-kind experiment** (cf-vacate/adv-assault-target/… under
> `raw-maxops` on the clean harness). Older arc below.
>
> **➡️ Prior handoff: `CONTINUITY-2026-07-31.md`** — dated where-we-left-off snapshot. The
> headline: **the basic primitives-only CALCULATOR settled the mechanism — the dispersed-integration deficit
> was ARITHMETIC, not spatial reasoning.** On T677/p4 (6.8% explored) dispersed kinds (DeepSeek, 3 reps,
> n=72/arm), **`raw-ops` (full board + calculator, think-OFF) = 0.986** — above raw think-ON (0.875) and raw
> baseline (0.792), and it drives **wrong answers to zero** (raw 15 wrong → raw-ops 0). The calculator pays off
> **only with full-board perception**: the same operators on the query loop did nothing (`interactive-ops` 0.806
> ≈ `interactive` 0.819 — that loop is **over-fetch/convergence-bound**, its failures are non-answers). So
> **perception and compute are separable levers; the win = cheap full perception + compute.** This greenlights
> the **maximal-calculator decision corpus**. Full writeup: **`analysis/findings-calc.md`**.
> **SCALE CAVEAT (added EOD):** `raw-ops` 0.986 is a **small-map** result. At **98% explored** (large known
> map, 3 reps) it **DROPS to 0.736 and TIES `interactive-ops` (0.750)** — the calculator's win is board-size-
> dependent; a second bottleneck (locating what to measure in a huge board) caps both ops arms at ~0.75. See
> `findings-calc.md` §Scale test.
> **Shipped & MERGED today:** the 4 decision-corpus kinds (cf-vacate, adv-assault-target, triage-reinforce,
> compare-two-attacks + `Answer::Compare3`); the gen-questions overflow fix; Langfuse turnkey + adapter timeline
> fix (traces land "now", visible by default); cloud validity done (think-ON cross-model + frontier anchor,
> `findings-crossmodel-thinkon.md`). **BUILT, NOT MERGED:** the maximal calculator (`raw-maxops`/
> `interactive-maxops`, branch `worktree-agent-ae7abce34c0372381`) — awaiting your merge decision.
> **Top next step:** failure analysis of the ~0.75 scale ceiling (scoped, not yet run).
> Prior arc (`CONTINUITY-2026-07-30.md`, `findings-fog.md`): small-map interactive levers isolated (enriched
> summary = accuracy, scan_grid = efficiency); **over-fetch CONFIRMED on real trajectories** (→ non-answers,
> not wrong answers; dedup-guard now lower-priority given the calculator result); Gemini early-game collapse
> replicates but is **confounded by agentic-effort asymmetry** (think-off ≠ behaviorally equal → think-ON
> cross-model is the clean comparison); t3-retreat + t3-threat merged; LLM I/O tracer default-on;
> observability = OTel→Langfuse.
> **Noise floor ~12–18% per-item → replicate + pool/majority-vote ≥3 reps for any sub-0.05 accuracy claim.**
> This RESUME file is the evergreen state + command reference below it.

**Last updated:** 2026-08-12 EOD (**harness hardening + publication planning; big experiment DE-RISKED not
run: 7 prompt fixes [`2ea027e`] + C1 cache-deduct budget [turns govern both surfaces; raw 3→17 turns] +
C2 off-menu guard + S1 Answer:-line-all-types + S2 value-beats-none + resume/incremental-flush [6-key
`--resume`/`--fresh`; model-name key bug caught by the smoke, fixed `ba0ff61`] — all `bdde824`/`ba0ff61`,
smoke-validated LIVE; first clean matrix drafted + PARKED [8×2×2×per-kind-8 ≈ 256 trials, ~$5/seed,
wall-clock-bound]; Vox Deorum researched for the blog [world-model verified thin]; Saturday publication
push. master `ba0ff61`, green. Findings: `CONTINUITY-2026-08-12.md`.**). Prior: 2026-08-11 EOD
(**constraint-site Variant C confirmed LIVE; budget-aware interactive loop
SHIPPED [max_turns + priority + [turn n/N] stamps, re-baseline]; scan_region FOG-FIDELITY FIX [lists
fogged-but-explored tiles instead of dropping them as Unknown]; scan_grid DROPPED → interactive-maxops now
16 tools [its hidden-force edge was the scan_region gap; post-fix DROP=KEEP both boards]. `just check` green.
Findings: `findings-constraint-site-variantC-live.md`, `findings-scanregion-board2-replication.md`,
`findings-scangrid-drop-budgetaware.md`; handoff `CONTINUITY-2026-08-11.md`**). Prior: 2026-08-10 EOD
(**interactive-maxops perception redesign: front-loaded roster [standard] +
`region_summary`→`scan_region` terrain-list + quadrant double-listing killed + B-STYLE tool consolidation
[19→17, dropped get_tile/scan, kept scan_grid]; constraint-site→Variant C; `--withhold` prose-scrub confound
fixed; two viewer batches — all merged & green, worktrees pruned. Findings: `analysis/findings-region-summary-arms.md`;
handoff: `CONTINUITY-2026-08-10.md`**). Prior: 2026-08-09 EOD (**ZOC-everywhere merged + M1/M2/M4 tool merges [maxops 25→~19 verbs] +
region_summary arbitrary-center redesign + two viewer batches, all merged & green; repo-wide `cargo fmt`; today's
worktrees pruned. Owner's 6 next-session items recorded — see `CONTINUITY-2026-08-09.md` +
`NEXT-SESSION-AGENDA-2026-08-09.md`**). Prior: 2026-08-07 EOD (**3 branches integrated + worktrees pruned; fogged 15-board corpus selected via
yield-driven optimizer [`selection-optimized.txt`]; kind roster FROZEN at 19; reasoning-frontier HIDDEN-INFO family
BUILT — GenCtx seam + P7f/P1/P3; first live DeepSeek smoke $0.62 → frontier kinds resist maximal tooling, interactive
≥ raw — see `CONTINUITY-2026-08-07.md`, `analysis/findings-smoke-nonsat.md`**). Prior: 2026-08-05 EOD (**three-state
fog + viewer polish merged; reasoning-frontier track designed; P4 built**; three branches awaited integration —
`CONTINUITY-2026-08-05.md`). Prior: 2026-08-02 EOD
(**tool–task fit + tool-parity**: survivor experiment shows no-ops ≈ spatial-ops ≪
maxops on decision kinds [`findings-tool-parity.md`]; new `travel_turns_from_owned` primitive solves nearest-owned
0.667→1.000, merged to master `067be07`; the 4-claim BLOG PLAN recorded above). Prior: 2026-08-01 EOD (harness
eval-validity bugs found + fixed — B1 unit-stacking [render-all-units +
operators-by-id, `maxops-enum` branch], B2/B3/B4/B6 batch [merged to master]; clean-harness rerun **overturns the
enumeration accuracy story → efficiency-only**; see `CONTINUITY-2026-08-01.md`, `analysis/findings-clean-ablation.md`,
`findings-bug-sweep.md`, `findings-fog-bug-rootcause.md`). Prior: 2026-07-31 EOD (**calculator confirms the dispersed deficit is ARITHMETIC on small boards
(`raw-ops` 0.986) but does NOT hold at scale — 98%-explored it drops to 0.736 and ties interactive-ops 0.750;
`analysis/findings-calc.md` §Scale test**; 4 decision-corpus kinds + gen-overflow fix MERGED; maximal calculator
built but NOT merged (branch `worktree-agent-ae7abce34c0372381`); cloud validity [think-ON cross-model +
frontier] done; Langfuse live + adapter timeline fixed). Prior (2026-07-30): small-map interactive levers isolated + over-fetch confirmed
(`findings-fog.md`); t3-retreat + t3-threat; LLM I/O tracer default-on; OTel→Langfuse. Prior (2026-07-29):
fog-of-war `--fog` masking; fair real-play corpus; T3 settle-site tier.

Read alongside: `CONTINUITY-2026-07-29.md` (latest handoff), `DESIGN.md` (durable design),
`interactive-board-access-design.md` + `T1g-T3-questions-draft.md` (interactive/siting design),
`RELATED-WORK.md` (prior art), `analysis/findings-*.md` (results — newest **`findings-interactive.md`**),
`T2-T3-design.md` (valuation design).

---

## TL;DR of where we are
The **offline stack is complete and green**; the **live path now speaks HTTPS to cloud models**
(not just local http). The first real encoding comparison **ceilinged** (cropped board too easy).
There are now **two routes off the ceiling**, both built and Oracle-verified:
- **(a) Big-context cloud model — the preferred, publishable route.** Cloud Flash models have
  ~1M context, so the **full board fits every encoding, adjacency included — no cropping at all**.
  Cropping was the *cause* of the ceiling (small board = easy in any encoding), so dropping it is
  the real fix. User is setting up an **OpenRouter** account for this.
- **(b) `--difficulty hard`** — larger question radii/paths/rays; the offline reasoning-burden
  lever, independent of board size. Now the default; `--difficulty easy` restores the old tier.

## RESULTS SO FAR
**Core findings (T50 full-board; DeepSeek V4 Flash; local Qwen a3b — `analysis/findings-deepseek-fullboard.md`,
`findings-tier-sweep.md`):**
1. **Ceiling broken.** No-reasoning DeepSeek separates: raw 0.893 · adjacency 0.869 · ascii 0.750.
   `raw` strictly dominates `adjacency` (higher acc at ~6.8× fewer tokens); frontier is raw vs ascii.
2. **Reasoning collapses the encoding gap — model-conditionally.** think-on lifts all three to ~0.98 on
   DeepSeek (raw−ascii 0.143 → 0.012); Gemini keeps its ~0.19 gap. Encoding matters most when reasoning
   is off/cheap.
3. **Best encoding is model-dependent.** Cloud models (DeepSeek/Gemini/Sonnet) prefer raw; local Qwen
   prefers ascii (0.881 vs 0.714) — no universal winner.
Region-count is the residual hard skill (survives reasoning); a **default-terrain confound was found and
FIXED** (`generators.rs` excludes the board's default terrain).

**T677 scale-crossover — DONE 2026-07-28 (`analysis/findings-t677-crossover.md`).** Dense board, DeepSeek
think-off, n=54/enc:
4. **No inversion — adjacency stays dominated.** raw and adjacency *tie* at 0.778, but adjacency costs
   **4.3× the tokens** (242k vs 56k) for that same accuracy → raw still strictly dominates it on the cost
   frontier. **Adjacency is now RETIRED from default runs** (opt-in via `--encoding adjacency`; code +
   oracle gate retained). `run` defaults to **raw+ascii+hierarchical** (the working set of interest).
5. **Hierarchical de-dominates at scale but does NOT (yet) beat raw** (crossover + confirmation, n=54 then n=108).
   Seed 1 showed it leading (0.852 vs raw 0.778), but the **confirmation (seed 2, n=108) tied them at 0.870**
   — the seed-1 edge was sampling noise (raw's unlucky terrain dip). Pooled it's +2.4 pts, inside the noise.
   Net: hierarchical goes from *dominated at T50* to *accuracy-tied-with-raw at T677*, but at **1.4× raw's
   tokens** it stays **off the cost frontier** (same verdict as adjacency, milder penalty). **raw remains the
   frontier encoder; ascii stays weakest** (0.815). Kept in the working default set as the aggregation
   contender to keep probing. **Likely blocker: our question mix is point/local (terrain, distance, nearest,
   local-radius region-count) — flat-list territory. Hierarchy should pay off on GLOBAL aggregative queries
   (quadrant/whole-board counts, "who controls the most territory") whose granularity matches its region
   summaries — a question-design change, not a bigger board. That's the next experiment; a think-on pass is
   the other open thread.**
Caching validated (98–99% hits); DeepSeek cache-read ~$0.028/M; a full T50 run ~$0.54, reasoning ~6.4× slower.

**INTERACTIVE board access — BUILT + FIRST WINS 2026-07-28 (`analysis/findings-interactive.md`).** DeepSeek
think-off, T677:
6. **Querying beats front-loading on local/bounded questions.** A multi-turn **tool loop** (`--encoding
   interactive`) lets the model query the board coarse-to-fine and pay only for what it fetches — the cost
   lever static hierarchical structurally can't reach. Stage B (n=12/mode, terrain + region-count):
   **interactive 12/12 vs raw/hierarchical 9/12**, at **~8× fewer tokens** than raw (6.9k vs 55.9k) and
   ~4.5× cheaper per correct answer. **It cracked region-count** (6/6 vs raw 5/6, hierarchical 3/6) — the
   counting wall — by tallying *fetched* leaves over a focused context, not eyeballing a 56k-token dump.
   **Failure mode:** `nearest` (unbounded global search) blows up (17 turns, ~99 tool calls, 280k tokens).
   → interactive wins LOCAL/BOUNDED, loses GLOBAL-SEARCH. Small n; wall-clock (many round-trips) is its
   real cost, not $. Verbs are observation-only (overview + region_summary + scan + get_tile); the model
   chooses what to fetch, which dissolves the summary-tuning validity worry.
7. **Interactive wins the settling DECISION too (Stage C, n=24/mode).** On `best-site` (best candidate tile
   for the most {terrain} in the work radius): **raw 4/8 (chance), hierarchical 6/8, interactive 8/8**;
   overall interactive **24/24** vs raw 16/24, hierarchical 18/24, at ~3× fewer tokens, ~2.5× cheaper/correct.
   Front-loaded encodings fail at spatial decisions needing local aggregation; the query loop nails them.
   Caveat: a **cost fat-tail** (one best-site trajectory hit 196 tool calls / 197k tokens, still correct).
   Consolidated: querying beats front-loading on local, bounded, AND decision-shaped questions.

## NEXT ACTIONS
**⏸️ WHERE WE LEFT OFF (2026-07-28):** the **siting comparison is DONE — interactive won again** (Stage C,
`findings-interactive.md`): interactive **24/24** vs raw 16/24, hierarchical 18/24; on `best-site` raw is at
chance **4/8** vs interactive **8/8**. Nothing running; `.env` has the key.

**START HERE TOMORROW (all cheap; `cargo build -p civ-cli --features remote` FIRST):**
1. **Larger-n siting confirmation** — bump the siting-comparison command below to `--per-kind 16` (± seed 2)
   to firm up the 8/8-vs-4/8 gap and the cost fat-tail. ~$0.30.
2. **Think-on interactive pass** — add `--think on` to the same command; does reasoning lift the static
   encoders toward interactive?
3. **`nearest` / cost fat-tail mitigation** (one best-site trajectory hit 196 tool calls / 197k tokens).

## NEXT ACTIONS (older)
**DONE since the last update (2026-07-28):** the **T677 scale-crossover** (findings above /
`findings-t677-crossover.md`); **adjacency retired** to opt-in (`run` defaults to raw+ascii+hierarchical;
code + gate kept); the **`--kinds a,b,c` category filter** (run/verify-oracle/gen-questions); and the
**minimal harness** `scripts/eval-min.sh` (raw+ascii+hierarchical × {terrain,region-count,nearest} ×
think-off on DeepSeek — it cuts cost by pruning *questions*, not encoders — auto-sourcing a gitignored
`.env` for the OpenRouter key). Prior session: Wilson CIs, tier sweep,
reconstruction gate, literature scan (`RELATED-WORK.md`; closest twin **Talk like a Graph**).

Prioritized experiments (prior-art-motivated — see `RELATED-WORK.md`):
1. **Hierarchical confirmation [DONE — did not replicate]** — seed 2, per-kind 12, n=108: raw and
   hierarchical **tied at 0.870**; the seed-1 edge was noise. Hierarchical is NOT promoted (off the cost
   frontier at 1.4× raw). `results-t677-hier-confirm.jsonl`. Remaining thread: a **think-on pass**
   (`THINK=on ENCODINGS="raw ascii hierarchical" bash scripts/eval-min.sh`, drop `--kinds` for the full
   aggregate) — does reasoning move any of raw/ascii/hierarchical the way it collapsed raw–ascii on DeepSeek?
2. **Balanced count sweep [NEXT]** — turn the count-curve into a measurement (multiple boards, balanced
   true counts 0–20+, fixed model, raw-vs-ascii). Cheap, high-value. `analysis/findings-count-curve.md`.
3. **New formats in sequence** — deterministic (NO LLM in the encoder), each held to the reconstruction
   gate: scene-graph (bucketed-vs-exact two-arm, kept info-complete) → landmark-graph → interactive/
   queryable (needs the tool-loop runner; its gate is a `get_tile`-parity check). (hierarchical: DONE, and
   it's the scale winner.)
4. **Factored labeling axis** (integer coords vs named tile IDs — Talk-like-a-Graph factors the encoder
   into node-labeling × edge-phrasing) + **tokenization/delimiter ASCII ablation** (cheap; likely
   explains local-Qwen→ascii).
5. **Harder T1 tier** (de-saturate distance/nearest/reachability) and re-run region-count post-fix.

**T2/T3 valuation — a DIFFERENT AGENT advanced this track** and delivered a complete,
   feasibility-validated design in **`T2-T3-design.md`** (no core code written there). User-approved
   decisions now handed to *our* (core) track to implement:
   - `Answer::ChoiceSet { acceptable, options }` for T3 dominance scoring (`acceptable` precomputed
     at generation → scorer stays pure/board-free); add a *negative* Oracle test (a dominated pick
     must score `wrong`).
   - `City.improvements` decode in `civ-core` (per-city bitstring vs `improvement_vector`:
     City Walls / Great Wall / Coastal Defense / Palace).
   - Rich-rendering seam: encoders surface walls/veteran/HP **only for T2/T3**, T0/T1 blocks stay
     byte-identical.
   - Fetch the **T677** board (dense/contested) for the headline valuation run; combat constants are
     hard-coded from the `classic` ruleset (machine-extraction is a §8 TODO, NOT done — so a
     differently-ruled save silently mismatches; gate ruleset when harvesting multi-board).
     Coordinate before wiring walls into encoders.

The cloud smoke-test + run commands (kept for reference):
```sh
cargo build -p civ-cli --features remote
./target/debug/civ.exe run --board data/saves/myagent_T50.sav \
  --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1 \
  --api-key sk-or-... --cache on --difficulty hard --think off --out results-X.jsonl
```
Local Qwen (LM Studio 32k) works as a cross-check but is ~5× slower/call and can't fit adjacency.

---

## What is built (see DESIGN.md for the full design)
Rust workspace, dependency-free offline core; live provider behind a `remote` feature.
- `civ-core/` — neutral `Board`/`Tile`/`Unit`/`City`, `geometry` (8-dir, north=y0, Chebyshev,
  non-wrapping), Freeciv `.sav` parser, `Board::crop`, seeded RNG. No deps.
- `civ-eval/` — `TileFacts`; **5 static encoders** (`raw`, `ascii` grid+legend, `adjacency`, `egocentric`,
  `hierarchical`) **+ 1 interactive surface** (`interactive`, a queryable tool loop — see below). `run`
  defaults to **raw+ascii+hierarchical** (`DEFAULT_ENCODERS`); adjacency + egocentric opt-in; interactive
  needs `--features remote`. `Question`/`Answer`/`EvalItem` + pure solvers for **T0/T1**
  (terrain, adjacency, direction, distance, nearest, region-count, reachability), **T2** (unit-strength,
  city-defense), **+ `best-site`** (T1 siting: best-of-K `Choice` — most {terrain} in the work radius);
  a `--kinds a,b,c` filter restricts to chosen categories; type-directed `Scorer` (correct/wrong/invalid);
  `Model` + offline `OracleModel`; `ChatModel` (multi-turn) impl'd by `remote::OpenAiCompatible`; runner →
  JSONL with cost fields (incl. `n_turns`/`n_tool_calls`); `remote::OpenAiCompatible` — **`ureq` + rustls
  TLS** (behind `--features remote`), http (local) AND https (cloud), `--provider` pin, `--cache`
  (`cache_control` breakpoint). `Difficulty` in `generators.rs`; `Prompt { cacheable_prefix, tail }`.
- **Interactive board access** (`interactive-board-access-design.md`): `QueryableSurface`/`Interactive`
  (`encoders.rs`) — overview + observation-only verbs `region_summary`/`scan`/`get_tile`; `describe.rs`
  (deterministic qualitative region descriptions); `run_interactive` (`runner.rs`) — the multi-turn tool
  loop (MAX_TURNS + token budget + force-answer, trajectory token sums). Parity gate = reconstruct-via-tools.
- `civ-cli/` — `civ` binary. Commands: `dump-board`, `gen-questions`, `verify-oracle`,
  `ping-model` (remote), `run`.
- `analysis/summarize.py` — stdlib pivot over `results-*.jsonl` (accuracy / tot_tok / gen_tok /
  latency by model×encoding, + per-category).
- `scripts/` — persistent run scripts (see below).

**Health:** `just check` = tests + clippy(-D warnings) + the Oracle-100% invariant (asserted at **both**
difficulty tiers; `verify-oracle` still covers adjacency's contract even though it's off by default), all
green — incl. the remote feature build/clippy. **Committed on `master`** through the minimal-harness work
(`54b46f4`); `ureq` (+ rustls/tokio-free deps) is in `civ-eval/Cargo.toml` behind `remote` only; the
offline core stays dependency-free.

⚠️ **Gotcha:** `just check` (and any plain `cargo run/build -p civ-cli`) rebuilds `target/debug/civ.exe`
**without** `--features remote` — a later cloud `run` then dies with "needs a live provider". Always
`cargo build -p civ-cli --features remote` **again** right before launching a cloud run (or the scripts,
which do it for you).

## Build / run reference
```sh
cargo build --workspace                       # offline
cargo test  --workspace                       # unit + integration (incl. Oracle at both tiers)
cargo build -p civ-cli --features remote      # enables live models (local http + cloud https)
# offline oracle self-consistency gate:
./target/debug/civ.exe verify-oracle --board data/saves/myagent_T50.sav [--crop x0,y0,w,h] [--difficulty easy|hard]
```
`run` flags: `--board P [--crop x0,y0,w,h] [--seed N] [--per-kind K] [--kinds a,b,c] [--encoding E ...]`
`[--difficulty easy|hard] [--model M] [--base-url URL] [--api-key K] [--think on|off]`
`[--provider P] [--cache on] [--concurrency N] [--max-turns N] [--token-budget N] [--out F]`.
`--encoding interactive` drives the multi-turn tool loop (needs `--features remote`); `--max-turns`
(default 16) / `--token-budget` (default 200000) bound each trajectory. Static + interactive can run in one
invocation. `--base-url` accepts http:// (local) or https:// (cloud). **`--concurrency N`** parallelizes —
**cloud only** (fans questions/calls out; warms the cache first). Keep **1 for local LM Studio**.

**Siting comparison (the immediate next run — rebuild remote first, source `.env`):**
```sh
cargo build -p civ-cli --features remote
./target/debug/civ.exe run --board data/saves/testcontroller_T677.sav \
  --seed 1 --per-kind 8 --difficulty hard --think off --cache on --concurrency 4 \
  --kinds best-site,terrain,region-count \
  --encoding raw --encoding hierarchical --encoding interactive \
  --model deepseek/deepseek-v4-flash --base-url https://openrouter.ai/api/v1 \
  --api-key "$OPENROUTER_API_KEY" --out results-siting-compare.jsonl
```
`just` works from Git Bash; recipes in `justfile`.

## LM Studio / model facts (IMPORTANT)
- Base URL: `http://localhost:1234/v1`. Context: **now 32768** (was 16384).
- Two models (JIT-loaded on request):
  - `qwen3.6-35b-a3b-mtp` — **MoE, ~3B active → fast**; the iteration workhorse.
  - `qwen3.6-27b-mtp` — dense, **slow** (~2–4 min/question with thinking on); use only for the
    model-tier comparison, ideally think-off.
- **Thinking toggle:** our `--think off` sends `reasoning_effort: "none"`, which IS honored
  (drops reasoning to ~0). `/no_think` and `chat_template_kwargs.enable_thinking:false` were
  BOTH ignored by this build — do not use them. Even with reasoning off, the model stays fairly
  verbose in its *visible* answer (~500–880 completion tokens), which is fine.
- LM Studio separates `reasoning_content` from `content`; our extractor reads `content` (falls
  back to `reasoning_content`), so the strict `Answer:` line is found cleanly.

## Cloud models (OpenRouter) — the cloud path facts
- Base URL: `https://openrouter.ai/api/v1`. Auth: `--api-key sk-or-...` (Bearer). Pin the upstream
  with `--provider <Name>` so the comparison can't drift across providers/quantizations.
- **All the Flash models below have ~1M context (1,048,576) → the full board fits every encoding,
  adjacency included.** No cropping for cloud runs. (Prices are per 1M tokens, verified 2026-07 via
  openrouter.ai; slugs drift — confirm against `/v1/models` or the model page before a big run.)
  - `deepseek/deepseek-v4-flash` — **$0.09 / $0.18**, 1M ctx, MoE 284B/13B-active. Cheapest; the
    workhorse. DeepSeek does automatic context caching.
  - `google/gemini-2.5-flash` — **$0.30 / $2.50**, 1M ctx. Different lineage → good tier/family
    diversity. Gemini auto-caches server-side.
  - `google/gemini-3.6-flash` — **$1.50 / $7.50**, 1M ctx. A cheap capability step-up for a mid anchor.
  - A true frontier anchor (Claude/GPT, verify the OpenRouter slug) gives the top tier point for
    Hypothesis #3. **Anthropic models need `--cache on`** to cache the board (our `cache_control`
    marker); Gemini/DeepSeek cache automatically, so `--cache on` is harmless-but-unnecessary there.
- **Prompt caching:** `--cache on` sends the board block as a separate content part with
  `cache_control:{type:ephemeral}`; the per-question tail is the variable suffix. Board tokens are
  then paid once per (board × encoding); check `cached_tokens` in the JSONL to confirm hits.
- **Reasoning toggle caveat:** `--think off` still emits Qwen's `reasoning_effort:"none"`.
  OpenRouter forwards it; a cloud *reasoning* model may reject `"none"` (expects low/medium/high) —
  smoke-test with `ping-model` first. This is the one per-model knob flagged in `remote.rs`.

## Subscription models (Anthropic via CLIProxyAPI) — the subscription path facts
Run experiments against the user's **Claude Max** subscription instead of paying per token. Full
how-to (incl. the agent quickstart): **`scripts/subscription-proxy-setup.md`**.
- **How:** a local proxy ([CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI)) re-exposes the
  subscription as OpenAI-compatible at `http://localhost:8317/v1`. No harness change — same
  `OpenAiCompatible` path as OpenRouter/LM Studio, just a different `--base-url` + `--api-key`.
- **Agents can RUN it but can't SET IT UP** — the OAuth login is a browser step only the user can do.
  Check first: `curl -s -m5 http://localhost:8317/v1/models -H "Authorization: Bearer civspatial-local"`.
  Empty → ask the user to start the proxy; don't `--claude-login` yourself.
- **Runner:** `bash scripts/run-subs-arm.sh` (cheap Claude-only smoke; `SMOKE=0 PK=N` scales up). It
  preflights the proxy and lists models. Cheapest model: `claude-haiku-4-5-20251001` (verified
  deterministic @ temp 0, `--think off` works, 2026-08-20).
- **`--cache off` REQUIRED:** the proxy speaks native Anthropic, which 400s on our `cache_control`
  inside `tool_result.content` (interactive arms). Runner defaults it off; flat-rate ⇒ caching moot.
- **EXPLORATORY, not a prereg arm** (hidden Claude Code system prompt). Keep out of the OpenRouter
  clean-run files; outputs isolate as `results-subs-arm-*.jsonl`.
- **Billing:** draws from the Max subscription usage limits (5h + weekly), NOT per-token. At the
  limit it STOPS (no auto-charge unless the overflow toggle is on). A big sweep can burn the weekly
  cap and block the user's own Claude — **check in before anything bigger than the smoke.**

## Key empirical facts / calibration
- **Token estimate:** our cost printout uses chars/4; the model's real tokenizer is **~2.1×**
  bigger. Multiply printed estimates by ~2.1 for model tokens.
- **Full-board T50 model-token sizes:** `ascii` ~10k, `raw` ~32k, `adjacency` ~200k+.
- **Context-fit at 16k (why we cropped):** only `ascii` fit; `raw` (~32k) hard-400'd. At 32k,
  `ascii` fits with room; full-board `raw` fits prompt but ~0 thinking room; `adjacency` never.
- **Crop sizes** (printed-estimate → ~2.1× model): `26,25,12,10` → adj ~3.2k→7k, raw 0.7k→1.5k,
  ascii 0.4k→0.8k. `26,24,14,12` → adj ~4.9k→10k. `22,18,24,22` (528 tiles) → adj ~17k→34k.
- **Truncation confound:** full-board `ascii` + think-on overflows 16k (prompt ~10k + reasoning
  up to ~6.4k pinned exactly at the ceiling → cut off → wrong). 32k fixes this for ascii.
- **Thinking dominates LATENCY** (sequential generation), not token *count* (board still 68–96%
  of tokens). Encoding also affects the *reasoning burden* (two independent levers) — so measure
  total tokens/latency **per correct answer**, not board size alone.
- **Ceiling result (the reason we're pivoting):** cropped `26,25,12,10`, a3b, think-off, 6/cat:
  `adjacency 42/42`, `raw 42/42`, `ascii 41/42` — no separation. Small board ⇒ encoding stops
  mattering. Cost axis was clean though: adjacency ~5× the tokens of ascii/raw for identical
  (perfect) accuracy → strictly dominated on efficiency here.

## Results files present
- `results-qwen3.6-35b-a3b-mtp-on.jsonl` — full-board a3b think-on (seed 1, per-kind 4, ascii),
  18/28 = 0.64 but **confounded by 16k truncation** (~4 questions pinned at ~6440 completion).
- `results-clean-off.jsonl` — the cropped think-off ceiling run above (126 trials).
- Analyze any set: `python analysis/summarize.py results-*.jsonl`.
(These are git-ignored as `*.jsonl`.)

## Running experiments (persistent scripts)
**`scripts/eval-min.sh` — the DEFAULT daily loop (the 80/20 minimal harness).** raw+ascii+hierarchical ×
{terrain, region-count, nearest} × think-off on DeepSeek V4 Flash, T677, cache on, concurrency 8. **The
cost saving is fewer QUESTIONS (the discriminating kinds only), NOT fewer encoders** — it keeps the whole
working set of interest, and you add new encoders here as you build them. It **auto-sources the gitignored
`.env`** for `OPENROUTER_API_KEY`, so no key on the command line. Every knob is env-overridable
(`THINK=on`, `BOARD=…`, `KINDS=…`, `PK=…`, `ENCODINGS="raw ascii hierarchical egocentric"`, `PROVIDER=…`).
The pruned parts are the saturated/duplicate *kinds* (direction/distance/reachability/adjacent-terrain) —
not worthless (they're the always-green floor-check), just off the daily loop. Run the full 9-kind battery
(and add adjacency/egocentric encoders) for milestone/writeup runs.
```sh
bash scripts/eval-min.sh                 # minimal loop, key from .env
THINK=on bash scripts/eval-min.sh        # think-on confirmation variant
```
`scripts/live-eval.sh` runs the encoder trio for one (model, think) condition (local LM Studio origin).
Env vars: `MODEL` (default a3b), `THINK` (off|on), `CROP`, `PK`, `SEED`, `OUT`. Example staged run:
```sh
# think-off first (fast; ceiling check)
MODEL=qwen3.6-35b-a3b-mtp THINK=off CROP=22,20,20,18 PK=6 OUT=results-big-off.jsonl \
  bash scripts/live-eval.sh
# inspect; if encodings SEPARATE (not all ~100%), then think-on (slow, ~1–1.5h):
MODEL=qwen3.6-35b-a3b-mtp THINK=on  CROP=22,20,20,18 PK=4 OUT=results-big-on.jsonl \
  bash scripts/live-eval.sh
python analysis/summarize.py results-big-off.jsonl results-big-on.jsonl
```
Run these with `run_in_background: true` (they take minutes–hours); think-on is the long pole.

## Gotchas (learned the hard way this session)
- **Stopping a background *loop*:** killing `civ.exe` is not enough — the bash loop respawns it.
  Kill the script's **process tree**: find it with
  `Get-CimInstance Win32_Process -Filter "Name='bash.exe'"` (look for the `.sh` path), then
  `taskkill /F /T /PID <pid>`. `TaskStop` on the job wrapper did NOT stop the inner loop.
- **Building while a live run holds `target/debug/civ.exe`:** use a separate target dir for
  offline work: `CARGO_TARGET_DIR=target-exp cargo build ...` (add `--features remote` if you
  need the live model there). `/target-exp/` is git-ignored.
- **LM Studio serves serially** — don't fire a second completion (e.g. a debug curl) while a run
  is going; it queues and skews timing. `/v1/models` is fine (not a completion).
- `/dev/null` works in Git Bash (maps to `nul`).
- `$CLAUDE_JOB_DIR/tmp` is **ephemeral** — persistent scripts live in `scripts/`.

## Open decisions / backlog (also in DESIGN.md §10)
- **Encoding comparison needs a harder regime.** Now addressed two ways: `--difficulty hard`
  (done) + full-board cloud runs with no crop (done — just needs the key). Multiple/sliding crops
  were considered and **rejected** for full-board coverage: each crop is self-contained, so it
  loses the long-range questions (far-tile distance, out-of-window nearest) we care about — the
  big-context cloud model gives true full-board coverage instead.
- **Run the actual cloud comparison** once the OpenRouter key exists (see IMMEDIATE NEXT ACTION).
- Local 27b model-tier comparison (think-off to stay tractable) — now one point in a broader
  cloud tier sweep.
- Egocentric / scene-graph / layered-ASCII encoders; the interactive/queryable board-access mode.
- OTel→LangFuse tracing (deferred; genuinely useful once trajectories/interactive exist).
- DuckDB/Parquet for analysis at scale (currently the stdlib script suffices).
- T2/T3 valuation tiers — **design now DELIVERED by a separate agent** in `T2-T3-design.md`
  (contract decisions approved; implementation handed to the core track — see NEXT ACTIONS #5).
  Needs `veteran`/`hp` (already decoded) + a new `City.improvements` decode + the T677 board.
