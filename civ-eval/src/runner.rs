//! The eval runner: for a matrix of encodings × items, render prompts, get model replies, score
//! them, and emit result rows (with cost fields) as JSONL. Also computes an accuracy summary.
//!
//! Two phases keep the offline Oracle honest: phase 1 builds every prompt and its canonical
//! answer; phase 2 constructs the Oracle from those canonicals and runs the whole matrix through
//! the same `Model` interface a real provider uses — so generation → prompt → extraction →
//! scoring is exercised end to end.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};

use civ_core::Board;

use crate::encoders::{
    encoder_by_name, Encoder, QueryableSurface, RawEncoder, REGION_SUMMARY_DESC_LEGACY,
};
use crate::json::{object, Val};
use crate::model::{ChatModel, Message, Model, OracleModel, Reply, ToolCall, ToolDef, Usage};
use crate::prompt::{assemble, Prompt};
use crate::question::{EvalItem, Tier};
use crate::rules::rules_block;
use crate::scoring::{score, Score, Status};
use crate::sink::RunSink;
use crate::trace::{TraceMeta, Tracer};

/// One scored (encoding × item) trial.
#[derive(Debug, Clone)]
pub struct ResultRow {
    pub model: String,
    /// Reasoning toggle in force for this run (`--think`). Persisted so a resume can key on it (part
    /// of the 6-tuple resume key); stamped by the [`crate::sink::RunSink`], defaults `false`.
    pub think: bool,
    /// Graded reasoning effort for this run (`--reasoning-effort`), or `"none"`. Persisted for the
    /// resume key; stamped by the sink, defaults empty.
    pub effort: String,
    pub encoding: String,
    pub board: String,
    pub item_id: String,
    pub category: String,
    pub tier: String,
    pub status: Status,
    pub expected: String,
    pub got: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
    pub latency_ms: u64,
    /// Interactive board-access mode: model round-trips and tool calls in the trajectory. For the
    /// static one-shot path these are `1` and `0`. Token fields above are *sums over the trajectory*.
    pub n_turns: u64,
    pub n_tool_calls: u64,
}

impl ResultRow {
    pub fn to_json(&self) -> String {
        object(&[
            ("model", Val::from(self.model.as_str())),
            ("think", Val::from(self.think)),
            ("effort", Val::from(self.effort.as_str())),
            ("encoding", Val::from(self.encoding.as_str())),
            ("board", Val::from(self.board.as_str())),
            ("item_id", Val::from(self.item_id.as_str())),
            ("category", Val::from(self.category.as_str())),
            ("tier", Val::from(self.tier.as_str())),
            ("status", Val::from(self.status.as_str())),
            ("correct", Val::from(self.status == Status::Correct)),
            ("expected", Val::from(self.expected.as_str())),
            ("got", Val::from(self.got.as_str())),
            ("prompt_tokens", Val::from(self.prompt_tokens)),
            ("completion_tokens", Val::from(self.completion_tokens)),
            ("cached_tokens", Val::from(self.cached_tokens)),
            ("latency_ms", Val::from(self.latency_ms)),
            ("n_turns", Val::from(self.n_turns)),
            ("n_tool_calls", Val::from(self.n_tool_calls)),
        ])
    }
}

/// Build the (encoding × item) prompt plan.
struct Plan<'a> {
    entries: Vec<PlanEntry<'a>>,
}

struct PlanEntry<'a> {
    encoding: String,
    item: &'a EvalItem,
    prompt: Prompt,
    canonical: String,
}

fn build_plan<'a>(board: &Board, encoders: &[Box<dyn Encoder>], items: &'a [EvalItem]) -> Plan<'a> {
    // The combat rules block is board-level (identical for every T2/T3 item); build it once and
    // hand it to those items only. T0/T1 keep `None`, so their board block stays byte-identical.
    let rules = rules_block(board);
    let mut entries = Vec::new();
    for enc in encoders {
        let block = enc.render(board);
        for item in items {
            let rb = match item.tier {
                Tier::T2 | Tier::T3 => Some(rules.as_str()),
                Tier::T0 | Tier::T1 => None,
            };
            let prompt = assemble(&block, item, enc.as_ref(), board, rb);
            entries.push(PlanEntry {
                encoding: enc.name().to_string(),
                item,
                canonical: item.answer.canonical(),
                prompt,
            });
        }
    }
    Plan { entries }
}

/// Run the encoders × items matrix against an arbitrary model (e.g. a live provider).
///
/// `concurrency` > 1 runs calls in parallel — a large wall-clock win for cloud providers, which
/// serve many requests at once (a serial sweep leaves that on the table). It is **cache-aware**:
/// per encoding it sends the first call serially to populate the board-prefix cache, then fans the
/// rest out, so the 98% cache-hit rate survives (blasting all calls at once would make every one a
/// cache *miss*). Only helps providers that parallelise — LM Studio serves serially, so keep
/// `concurrency = 1` for local models. The model must be `Sync` to share across threads.
///
/// `tracer` (optional) captures the full prompt + reply of every trial to disk as a side-channel;
/// pass `None` to disable. It never affects scoring, token accounting, or determinism.
///
/// `sink` (optional) is the incremental JSONL result sink: when set, each scored row is appended and
/// flushed as it completes (crash-safe), and any trial already in the file (a `--resume`) is skipped
/// before the model is called. Pass `None` to keep every trial and return rows without writing.
pub fn run(
    board: &Board,
    encoding_names: &[String],
    items: &[EvalItem],
    model: &(dyn Model + Sync),
    concurrency: usize,
    tracer: Option<&Tracer>,
    sink: Option<&RunSink>,
) -> Vec<ResultRow> {
    let encoders: Vec<Box<dyn Encoder>> = encoding_names
        .iter()
        .filter_map(|n| encoder_by_name(n))
        .collect();
    let mut plan = build_plan(board, &encoders, items);
    filter_done(&mut plan, sink);
    if concurrency > 1 {
        run_with_model_concurrent(&plan, model, concurrency, tracer, sink)
    } else {
        run_with_model(&plan, model, tracer, sink)
    }
}

/// Drop plan entries whose 6-key is already present in the resume sink (a no-op without a sink).
fn filter_done(plan: &mut Plan, sink: Option<&RunSink>) {
    if let Some(s) = sink {
        plan.entries
            .retain(|e| !s.is_done(&e.encoding, &e.item.id, &e.item.board_source));
    }
}

/// Run the offline Oracle eval over the encoders × items matrix. Returns the scored rows. `sink` is
/// threaded through exactly as in [`run`] (skip already-done trials, append+flush the rest).
pub fn run_oracle(
    board: &Board,
    encoding_names: &[String],
    items: &[EvalItem],
    sink: Option<&RunSink>,
) -> Vec<ResultRow> {
    let encoders: Vec<Box<dyn Encoder>> = encoding_names
        .iter()
        .filter_map(|n| encoder_by_name(n))
        .collect();
    let mut plan = build_plan(board, &encoders, items);

    // Build the prompt→canonical map from the FULL plan (before skip-filtering) so the Oracle can
    // answer every entry it is asked; then drop the already-done entries from what actually runs.
    let by_prompt: HashMap<String, String> = plan
        .entries
        .iter()
        .map(|e| (e.prompt.full(), e.canonical.clone()))
        .collect();
    let model = OracleModel::new(by_prompt);
    filter_done(&mut plan, sink);

    // The Oracle is a deterministic offline stand-in, not real LLM I/O — never traced.
    run_with_model(&plan, &model, None, sink)
}

/// Score one plan entry: pose its prompt to the model and grade the reply into a result row.
/// When `tracer` is set, the single request/reply is also persisted as a static trace.
fn score_entry(model: &dyn Model, e: &PlanEntry, tracer: Option<&Tracer>) -> ResultRow {
    // B4: a provider that returns nothing (empty / 0-token completion) is a *failed call*, not an
    // answered-wrong. Retry once; if it is still empty, record `error` (excluded from the scored
    // denominator) rather than folding a non-answer into accuracy as `invalid`.
    let (reply, empty) = answer_with_retry(model, &e.prompt.cacheable_prefix, &e.prompt.tail);
    let sc = if empty {
        Score {
            status: Status::Error,
            extracted: String::new(),
        }
    } else {
        score(&e.item.answer, &reply.text)
    };
    if let Some(t) = tracer {
        let category = e.item.category.to_string();
        let mut tb = t.begin(&TraceMeta {
            mode: "static",
            model: model.name(),
            encoding: &e.encoding,
            board: &e.item.board_source,
            item_id: &e.item.id,
            category: &category,
            tier: e.item.tier.as_str(),
        });
        tb.static_turn(&e.prompt.cacheable_prefix, &e.prompt.tail, &reply);
        tb.finish(
            sc.status.as_str(),
            &e.item.answer.canonical(),
            &sc.extracted,
        );
    }
    ResultRow {
        model: model.name().to_string(),
        // Stamped by the sink from the run identity; default here (leaf doesn't know the flags).
        think: false,
        effort: String::new(),
        encoding: e.encoding.clone(),
        board: e.item.board_source.clone(),
        item_id: e.item.id.clone(),
        category: e.item.category.to_string(),
        tier: e.item.tier.as_str().to_string(),
        status: sc.status,
        expected: e.item.answer.canonical(),
        got: sc.extracted,
        prompt_tokens: reply.usage.prompt_tokens,
        completion_tokens: reply.usage.completion_tokens,
        cached_tokens: reply.usage.cached_tokens,
        latency_ms: reply.latency_ms,
        n_turns: 1,
        n_tool_calls: 0,
    }
}

/// Whether a completion is empty (a failed/empty provider body). We key on the *text* rather than
/// the token count: a real answer with usage the provider forgot to report (completion_tokens == 0)
/// is a legitimate scored reply, whereas an empty body is the failed-call signal we want to retry.
fn is_empty_completion(reply: &Reply) -> bool {
    reply.text.trim().is_empty()
}

/// Fetch one completion, retrying **once** if the first came back empty (B4). Returns the reply and
/// whether it is still empty after the retry (→ `error`, not a scored `invalid`). Model-agnostic, so
/// it hardens the live `OpenAiCompatible` provider (which yields an empty `Reply` on a failed HTTP
/// call) without any provider-specific code.
fn answer_with_retry(model: &dyn Model, cacheable_prefix: &str, tail: &str) -> (Reply, bool) {
    let mut reply = model.answer_cached(cacheable_prefix, tail);
    if is_empty_completion(&reply) {
        reply = model.answer_cached(cacheable_prefix, tail);
    }
    let empty = is_empty_completion(&reply);
    (reply, empty)
}

/// Execute a prebuilt plan against any model, serially. Used by the offline Oracle path (keeping
/// its self-consistency invariant deterministic) and whenever `concurrency <= 1`. Each row is
/// appended+flushed through `sink` as it completes.
fn run_with_model(
    plan: &Plan,
    model: &dyn Model,
    tracer: Option<&Tracer>,
    sink: Option<&RunSink>,
) -> Vec<ResultRow> {
    plan.entries
        .iter()
        .map(|e| {
            let mut row = score_entry(model, e, tracer);
            if let Some(s) = sink {
                s.record(&mut row);
            }
            row
        })
        .collect()
}

/// Concurrent, cache-aware execution (see `run`). Processes one encoding at a time; within each,
/// the first call is serial (warms the shared board-prefix cache) and the rest run on a bounded
/// pool of `concurrency` worker threads that pull from an atomic index. Output order matches the
/// serial path (results are re-sorted to plan order within each group).
fn run_with_model_concurrent(
    plan: &Plan,
    model: &(dyn Model + Sync),
    concurrency: usize,
    tracer: Option<&Tracer>,
    sink: Option<&RunSink>,
) -> Vec<ResultRow> {
    // Group entries by encoding, preserving first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&PlanEntry>> = HashMap::new();
    for e in &plan.entries {
        if !groups.contains_key(&e.encoding) {
            order.push(e.encoding.clone());
        }
        groups.entry(e.encoding.clone()).or_default().push(e);
    }

    let total = plan.entries.len();
    let progress = AtomicUsize::new(0);
    let mut rows = Vec::with_capacity(total);
    for enc in &order {
        let entries = &groups[enc];
        eprintln!(
            "  [{enc}] {} trials — warming board-prefix cache, then running at concurrency {concurrency} ...",
            entries.len()
        );
        rows.extend(run_group_concurrent(
            entries,
            model,
            concurrency,
            &progress,
            total,
            tracer,
            sink,
        ));
    }
    rows
}

/// Run one encoding's entries: entry 0 serial (cache warm-up), the rest across a bounded thread
/// pool. Returns rows in plan order for this group. Each row is appended+flushed through `sink` as
/// it completes (the sink's writer is `Mutex`-guarded, so concurrent records are safe).
fn run_group_concurrent(
    entries: &[&PlanEntry],
    model: &(dyn Model + Sync),
    concurrency: usize,
    progress: &AtomicUsize,
    total: usize,
    tracer: Option<&Tracer>,
    sink: Option<&RunSink>,
) -> Vec<ResultRow> {
    let score_and_record = |e: &PlanEntry| {
        let mut row = score_entry(model, e, tracer);
        if let Some(s) = sink {
            s.record(&mut row);
        }
        row
    };
    let Some((first, rest)) = entries.split_first() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(entries.len());
    // Warm the board-prefix cache with the first call before any fan-out.
    out.push(score_and_record(first));
    tick(progress, total);

    if rest.is_empty() {
        return out;
    }

    let next = AtomicUsize::new(0);
    let workers = concurrency.min(rest.len()).max(1);
    let mut collected: Vec<(usize, ResultRow)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    let mut local = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= rest.len() {
                            break;
                        }
                        let row = score_and_record(rest[i]);
                        tick(progress, total);
                        local.push((i, row));
                    }
                    local
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    collected.sort_by_key(|(i, _)| *i);
    out.extend(collected.into_iter().map(|(_, r)| r));
    out
}

/// Bump the global completion counter and log occasional progress so long runs show life.
fn tick(progress: &AtomicUsize, total: usize) {
    let done = progress.fetch_add(1, Ordering::Relaxed) + 1;
    if done.is_multiple_of(25) || done == total {
        eprintln!("    ... {done}/{total} trials done");
    }
}

/// Board-block token size per encoding (a preview of the cost axis, independent of questions).
pub fn board_block_tokens(board: &Board, encoding_names: &[String]) -> BTreeMap<String, u64> {
    let encoders: Vec<Box<dyn Encoder>> = encoding_names
        .iter()
        .filter_map(|n| encoder_by_name(n))
        .collect();
    encoders
        .iter()
        .map(|e| {
            (
                e.name().to_string(),
                crate::model::estimate_tokens(&e.render(board)),
            )
        })
        .collect()
}

// --- summary --------------------------------------------------------------

/// Accuracy and mean prompt cost per (encoding, category), plus per-encoding totals.
pub fn summarize(rows: &[ResultRow]) -> String {
    // (encoding, category) -> (correct, total, token_sum)
    let mut cells: BTreeMap<(String, String), (u64, u64, u64)> = BTreeMap::new();
    let mut enc_totals: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
    let mut invalids: BTreeMap<String, u64> = BTreeMap::new();
    let mut errors: BTreeMap<String, u64> = BTreeMap::new();
    for r in rows {
        // B4: `error` (failed/empty call) is NOT a scored trial — tally it separately and keep it
        // out of the accuracy denominator so a run can't be silently deflated by dropped calls.
        if r.status == Status::Error {
            *errors.entry(r.encoding.clone()).or_default() += 1;
            continue;
        }
        let cell = cells
            .entry((r.encoding.clone(), r.category.clone()))
            .or_default();
        cell.0 += (r.status == Status::Correct) as u64;
        cell.1 += 1;
        cell.2 += r.prompt_tokens;
        let tot = enc_totals.entry(r.encoding.clone()).or_default();
        tot.0 += (r.status == Status::Correct) as u64;
        tot.1 += 1;
        tot.2 += r.prompt_tokens;
        if r.status == Status::Invalid {
            *invalids.entry(r.encoding.clone()).or_default() += 1;
        }
    }

    let mut out = String::new();
    out.push_str("Accuracy by encoding × category (correct/total):\n");
    let categories: std::collections::BTreeSet<String> =
        cells.keys().map(|(_, c)| c.clone()).collect();
    let encodings: std::collections::BTreeSet<String> =
        cells.keys().map(|(e, _)| e.clone()).collect();

    out.push_str(&format!("  {:<14}", "category"));
    for e in &encodings {
        out.push_str(&format!("{:>16}", e));
    }
    out.push('\n');
    for c in &categories {
        out.push_str(&format!("  {c:<14}"));
        for e in &encodings {
            if let Some((ok, tot, _)) = cells.get(&(e.clone(), c.clone())) {
                out.push_str(&format!("{:>16}", format!("{ok}/{tot}")));
            } else {
                out.push_str(&format!("{:>16}", "-"));
            }
        }
        out.push('\n');
    }

    out.push_str("\nPer-encoding totals:\n");
    for (e, (ok, tot, toks)) in &enc_totals {
        let acc = if *tot > 0 {
            *ok as f64 / *tot as f64
        } else {
            0.0
        };
        let mean_tok = toks.checked_div(*tot).unwrap_or(0);
        let inv = invalids.get(e).copied().unwrap_or(0);
        let err = errors.get(e).copied().unwrap_or(0);
        out.push_str(&format!(
            "  {e:<10} accuracy {ok}/{tot} = {acc:.3}   mean prompt ~{mean_tok} tok   invalid {inv}   error {err}\n"
        ));
    }
    out
}

// --------------------------------------------------------------------------
// interactive board-access — the multi-turn tool loop (see
// interactive-board-access-design.md). The model gets a cheap system prefix + tools and queries
// the board coarse-to-fine; we score the final answer and sum tokens over the whole trajectory.
// --------------------------------------------------------------------------

/// Loop knobs. `max_turns` is the real economic governor (kept at 16); `token_budget` is a
/// runaway/OOM backstop that force-answers only a genuinely pathological trajectory. Since C1 the
/// budget is charged on NON-cached tokens (see the loop), so a big cached prefix no longer trips it.
///
/// The three `*` arm modifiers are composable, default to the current (no-op) behavior, and are
/// applied as a TRANSFORM over the surface's `tools()`/`overview()` inside [`run_interactive`] — the
/// surface impls never see them. They exist to express a 4-arm A/B on the `interactive-maxops`
/// surface (control desc / redescribe / drop scan_grid / +roster) in ONE sweep, differing only by
/// explicit flags. See `civ-cli` `--legacy-rs-desc` / `--withhold` / `--frontload-roster`.
#[derive(Clone, Debug)]
pub struct InteractiveConfig {
    pub max_turns: usize,
    pub token_budget: u64,
    /// When true, restore the pre-redescribe `region_summary` description
    /// ([`crate::encoders::REGION_SUMMARY_DESC_LEGACY`]) — the A/B control arm.
    pub legacy_rs_desc: bool,
    /// Tool names to remove from the interactive menu for this run (e.g. `scan_grid`).
    pub withhold: Vec<String>,
    /// When true, prepend the rendered `list_units` + `list_cities` directory (the occupant roster)
    /// to the surface overview, so the model sees the roster without spending calls on it. NOTE: the
    /// `interactive-maxops`/`-enum` surfaces now front-load the roster in their OWN overview as the
    /// standard, so this flag is a no-op there (it still applies to the plain interactive surfaces).
    pub frontload_roster: bool,
}

impl Default for InteractiveConfig {
    fn default() -> Self {
        // `max_turns` is the governor; `token_budget` is a high runaway/OOM backstop only. With C1's
        // cache-deducted accounting, a large cached board prefix no longer trips this, so both
        // front-loaded and interactive surfaces get their full 16-turn allowance.
        InteractiveConfig {
            max_turns: 16,
            token_budget: 2_000_000,
            legacy_rs_desc: false,
            withhold: Vec::new(),
            frontload_roster: false,
        }
    }
}

/// The strategy-free mechanics prompt (interactive-board-access-design.md §6). Teaches HOW to use
/// the tools + the answer format; never WHAT to look for (that would be teaching-to-the-test). The
/// tool-teaching head comes from the surface itself (`system_preamble`) so each board-access mode
/// (perception verbs, or the operator calculator) describes its own verbs; the runner only appends
/// the board text (`overview`) and any rules block.
fn interactive_system_prompt(system_base: &str, rules: Option<&str>, max_turns: usize) -> String {
    // Budget-awareness header (prepended so it leads the system message). Parameterized by the actual
    // `max_turns`, which is constant across a run, so the cacheable prefix stays stable. Each tool
    // result is tagged `[turn n/N]` in the loop so the model can pace itself. Priority order is
    // deliberate: getting FORCED into a rushed answer (budget exhaustion re-prompts for an answer with
    // no tools) is the worst outcome, then accuracy, then efficiency — see
    // analysis/findings-scanregion-board2-replication.md (frontier kinds go budget-marginal at scale
    // and end up answering under the gun if they don't self-manage).
    let mut s = format!(
        "Query budget: you have a HARD LIMIT of {max_turns} tool-use turns for this question. If you \
         run out, you'll be asked to answer with whatever you have — so a rushed, forced answer is \
         the real risk, not a missing one. Each tool result is tagged `[turn n/{max_turns}]` so you \
         can track how many remain. Manage the budget in this priority order:\n\
         1. ABOVE ALL, don't get forced into a rushed answer — pace yourself and keep at least one \
         turn in reserve so you give your `Answer:` line deliberately, not under the gun.\n\
         2. Then, be correct.\n\
         3. Then, use as few turns as possible.\n\n"
    );
    s.push_str(system_base);
    if let Some(r) = rules {
        s.push_str("\n\n");
        s.push_str(r);
    }
    s
}

/// Assemble the per-run interactive system base: the surface's tool-teaching `system_preamble` +
/// the board `overview`, then scrub every `withhold` tool out of that combined prose. Built ONCE per
/// run (like `transform_tools`/`overview`) so every question sees the identical arm; the per-question
/// rules block is appended afterwards by [`interactive_system_prompt`].
fn interactive_system_base(preamble: &str, overview: &str, withhold: &[String]) -> String {
    scrub_withheld_prose(&format!("{preamble}{overview}"), withhold)
}

/// Sentence/clause separators the prose scrub treats as boundaries.
const PROSE_SEPS: [&str; 3] = ["; ", ". ", "\n"];

/// Remove every textual mention of the `withhold` tools from the assembled interactive system prompt
/// (preamble + overview). The withheld tools are already dropped from the JSON tool menu by
/// [`transform_tools`]; leaving them *described* in the prose tells the model to call a verb it
/// cannot, which it then tries and fails at — thrashing that confounds the withhold ablation. For
/// each withheld tool the scrub: (a) drops its own `- <name>(...)` bullet line (with the trailing
/// newline); (b) collapses slash-enumerations that list it (`scan/<name>/get_tile` → `scan/get_tile`);
/// and (c) deletes any remaining running-prose clause that still mentions it, tidying the surrounding
/// sentence separators so the result stays grammatical (no `scan//get_tile`, no dangling `use ;`).
/// Written generically over the withhold set; it only has to be correct for the tools we actually
/// withhold (`scan_grid` on `interactive-maxops`). A no-op when `withhold` is empty.
fn scrub_withheld_prose(prompt: &str, withhold: &[String]) -> String {
    let mut s = prompt.to_string();
    for w in withhold {
        // (a) Drop the tool's own bullet line, including its trailing newline.
        let bullet = format!("- {w}(");
        while let Some(pos) = s.find(&bullet) {
            let end = s[pos..].find('\n').map_or(s.len(), |i| pos + i + 1);
            s.replace_range(pos..end, "");
        }
        // (b) Collapse slash-separated enumerations that list the tool (spaced or tight).
        for pat in [
            format!(" / {w}"),
            format!("/{w}"),
            format!("{w} / "),
            format!("{w}/"),
        ] {
            s = s.replace(&pat, "");
        }
        // (c) Delete any remaining clause that still mentions the tool.
        while let Some(pos) = s.find(w.as_str()) {
            remove_clause_at(&mut s, pos);
        }
    }
    s
}

/// Delete the running-prose clause containing byte offset `pos`, bounded by [`PROSE_SEPS`], along
/// with its trailing separator (so no dangling `; `). If the clause started a sentence, re-capitalize
/// whatever now follows so the prose stays grammatical.
fn remove_clause_at(s: &mut String, pos: usize) {
    // Clause start = just after the nearest separator ending at/before `pos` (else string start).
    // `start_is_sentence` is true when that separator ends a sentence (`. `/newline), not a `; `.
    let (start, start_is_sentence) = PROSE_SEPS
        .iter()
        .filter_map(|sep| s[..pos].rfind(sep).map(|i| (i + sep.len(), *sep != "; ")))
        .max_by_key(|(i, _)| *i)
        .unwrap_or((0, true));
    // Clause end = start of the nearest separator at/after `pos` (else string end).
    let (end, end_sep_len) = PROSE_SEPS
        .iter()
        .filter_map(|sep| s[pos..].find(sep).map(|i| (pos + i, sep.len())))
        .min_by_key(|(i, _)| *i)
        .unwrap_or((s.len(), 0));
    s.replace_range(start..end + end_sep_len, "");
    if start_is_sentence {
        if let Some(c) = s[start..].chars().next() {
            if c.is_ascii_lowercase() {
                s.replace_range(start..start + c.len_utf8(), &c.to_ascii_uppercase().to_string());
            }
        }
    }
}

/// Per-run interactive context shared across every question (built once so the system-prefix cache
/// warms): the board overview text and the combat rules block.
struct InteractiveEnv<'a> {
    /// The assembled system base (preamble + overview, with withheld tools scrubbed from the prose).
    system_base: &'a str,
    rules: &'a str,
    /// The tool menu after applying the arm modifiers (withhold / legacy-rs-desc). Built once so
    /// every question in the run sees the identical (transformed) tool list.
    tools: &'a [ToolDef],
}

/// Render the occupant directory the way the `list_units` / `list_cities` fetch verbs do, delimited
/// under headers, for the `--frontload-roster` arm. Reuses the surface's own `execute` so the text
/// is byte-identical to what the model would get by calling the tools itself.
fn render_frontloaded_roster(board: &Board, surface: &dyn QueryableSurface) -> String {
    let call = |name: &str| ToolCall {
        id: String::new(),
        name: name.to_string(),
        args_json: "{}".to_string(),
    };
    let units = surface.execute(board, &call("list_units"));
    let cities = surface.execute(board, &call("list_cities"));
    format!(
        "KNOWN UNITS (every visible unit — the list_units directory, provided so you need not call it):\n\
         {units}\n\n\
         KNOWN CITIES (every visible city — the list_cities directory, provided so you need not call it):\n\
         {cities}\n\n"
    )
}

/// Apply the composable arm modifiers to a surface's tool menu: drop any `withhold` names, and (if
/// `legacy_rs_desc`) restore the pre-redescribe `region_summary` description. Built once per run.
fn transform_tools(mut tools: Vec<ToolDef>, cfg: &InteractiveConfig) -> Vec<ToolDef> {
    if !cfg.withhold.is_empty() {
        tools.retain(|t| !cfg.withhold.iter().any(|w| w == t.name));
    }
    if cfg.legacy_rs_desc {
        if let Some(t) = tools.iter_mut().find(|t| t.name == "region_summary") {
            t.description = REGION_SUMMARY_DESC_LEGACY.to_string();
        }
    }
    tools
}

/// Drive one question through the tool loop and score the final answer.
fn run_one_interactive(
    board: &Board,
    surface: &dyn QueryableSurface,
    model: &dyn ChatModel,
    item: &EvalItem,
    env: &InteractiveEnv,
    cfg: &InteractiveConfig,
    tracer: Option<&Tracer>,
) -> ResultRow {
    let tools = env.tools;
    let rb = matches!(item.tier, Tier::T2 | Tier::T3).then_some(env.rules);
    let system = interactive_system_prompt(env.system_base, rb, cfg.max_turns);
    let question = item.question.render(&RawEncoder, board);

    let mut messages = vec![Message::system(system), Message::user(question)];
    let mut total = Usage::default();
    let mut turns: u64 = 0;
    let mut tool_calls: u64 = 0;
    let mut latency_ms: u64 = 0;

    // Capture the whole trajectory (every request + reply) as a side-channel when tracing is on.
    let category = item.category.to_string();
    let mut trace = tracer.map(|t| {
        t.begin(&TraceMeta {
            mode: "interactive",
            model: model.name(),
            encoding: surface.name(),
            board: &item.board_source,
            item_id: &item.id,
            category: &category,
            tier: item.tier.as_str(),
        })
    });

    let final_text = loop {
        // C1: count only NON-cached prompt tokens. A front-loaded surface (e.g. raw-maxops, ~55k
        // board) re-sends its full cached system+board prefix every turn; charging the whole
        // `prompt_tokens` each time tripped the budget in ~3 turns and force-answered it while its
        // turn allowance sat untouched. Deducting `cached_tokens` makes accounting symmetric, so
        // `max_turns` is the real governor and `token_budget` is only a runaway/OOM backstop.
        // (OracleModel reports 0 cached, so offline behavior is unchanged.)
        let spent =
            total.prompt_tokens.saturating_sub(total.cached_tokens) + total.completion_tokens;
        if turns as usize >= cfg.max_turns || spent >= cfg.token_budget {
            // Budget/turn cap hit: force a final answer with no tools available.
            messages.push(Message::user(
                "You have reached your query budget. Answer now with what you know, as `Answer: <X>`.",
            ));
            let reply = model.chat(&messages, &[]);
            if let Some(tb) = trace.as_mut() {
                tb.chat_turn(&messages, &[], &reply);
            }
            accumulate(&mut total, &reply.usage);
            latency_ms += reply.latency_ms;
            turns += 1;
            break reply.text.unwrap_or_default();
        }

        let reply = model.chat(&messages, tools);
        if let Some(tb) = trace.as_mut() {
            tb.chat_turn(&messages, tools, &reply);
        }
        accumulate(&mut total, &reply.usage);
        latency_ms += reply.latency_ms;
        turns += 1;

        if !reply.tool_calls.is_empty() {
            messages.push(Message::assistant_calls(reply.tool_calls.clone()));
            // Tag every result with the running turn count so the model can pace itself toward the
            // `max_turns` cap (a missing answer is the worst outcome — see the budget header). `turns`
            // was just incremented for this reply, so it is the count of turns used so far.
            let stamp = format!(" [turn {}/{}]", turns, cfg.max_turns);
            for call in &reply.tool_calls {
                // C2: only execute a tool that is actually on this arm's advertised menu. Withheld or
                // removed tools (scan_grid, get_tile, …) are scrubbed from the menu text but a model
                // can still emit them by name; without this guard only a strict provider would block
                // them, silently smuggling a dropped tool back into a withhold A/B or a cross-model
                // run. Off-menu calls get an error result (not a real answer) but still consume the
                // turn, so the budget/pacing accounting is unchanged.
                let result = if tools.iter().any(|t| t.name == call.name) {
                    surface.execute(board, call)
                } else {
                    format!("error: tool '{}' is not available on this surface", call.name)
                };
                messages.push(Message::tool_result(&call.id, format!("{result}{stamp}")));
                tool_calls += 1;
            }
            continue;
        }
        break reply.text.unwrap_or_default();
    };

    // B4: an empty final answer is a failed call, not an answered-wrong → `error` (excluded from the
    // scored denominator). We only classify here, not retry the whole trajectory, to keep the fix
    // localized; the static path retries once.
    let sc = if final_text.trim().is_empty() {
        Score {
            status: Status::Error,
            extracted: String::new(),
        }
    } else {
        score(&item.answer, &final_text)
    };
    if let Some(tb) = trace {
        tb.finish(sc.status.as_str(), &item.answer.canonical(), &sc.extracted);
    }
    ResultRow {
        model: model.name().to_string(),
        // Stamped by the sink from the run identity; default here (leaf doesn't know the flags).
        think: false,
        effort: String::new(),
        encoding: surface.name().to_string(),
        board: item.board_source.clone(),
        item_id: item.id.clone(),
        category: item.category.to_string(),
        tier: item.tier.as_str().to_string(),
        status: sc.status,
        expected: item.answer.canonical(),
        got: sc.extracted,
        prompt_tokens: total.prompt_tokens,
        completion_tokens: total.completion_tokens,
        cached_tokens: total.cached_tokens,
        latency_ms,
        n_turns: turns,
        n_tool_calls: tool_calls,
    }
}

fn accumulate(total: &mut Usage, u: &Usage) {
    total.prompt_tokens += u.prompt_tokens;
    total.completion_tokens += u.completion_tokens;
    total.cached_tokens += u.cached_tokens;
}

/// Run every item through the interactive tool loop. `concurrency` fans questions out across
/// threads (cloud only); the first question runs serially to warm the system-prefix cache, then
/// the rest run on a bounded pool — the same cache-aware pattern as the static concurrent path.
#[allow(clippy::too_many_arguments)]
pub fn run_interactive(
    board: &Board,
    surface: &(dyn QueryableSurface + Sync),
    model: &(dyn ChatModel + Sync),
    items: &[EvalItem],
    cfg: InteractiveConfig,
    concurrency: usize,
    tracer: Option<&Tracer>,
    sink: Option<&RunSink>,
) -> Vec<ResultRow> {
    // Build the (possibly transformed) tool menu + overview ONCE, so every question in the run sees
    // the identical arm. `withhold`/`legacy_rs_desc` transform the menu; `frontload_roster` prepends
    // the occupant directory to the overview. The surface impls are untouched by the modifiers.
    let tools = transform_tools(surface.tools(), &cfg);
    let base_overview = surface.overview(board);
    // `interactive-maxops` (+ `-enum`) front-load the occupant roster in their OWN overview now, so
    // the flag is redundant there — never double-prepend it. It still works for the plain
    // `interactive`/`interactive-ops` surfaces, whose overview carries no roster.
    let overview = if cfg.frontload_roster && !surface.name().starts_with("interactive-maxops") {
        format!("{}{base_overview}", render_frontloaded_roster(board, surface))
    } else {
        base_overview
    };
    // Assemble the system base (preamble + overview) once and scrub any withheld tool from its prose,
    // so a `--withhold` arm leaves no textual trace of a tool it also dropped from the menu.
    let system_base = interactive_system_base(&surface.system_preamble(), &overview, &cfg.withhold);
    let rules = rules_block(board);
    let env = InteractiveEnv {
        system_base: &system_base,
        rules: &rules,
        tools: &tools,
    };
    // Skip trials already in the resume file, then wrap the leaf so each row is appended+flushed as
    // it completes.
    let run_items: Vec<&EvalItem> = items
        .iter()
        .filter(|it| match sink {
            Some(s) => !s.is_done(surface.name(), &it.id, &it.board_source),
            None => true,
        })
        .collect();
    let one = |item: &EvalItem| -> ResultRow {
        let mut row = run_one_interactive(board, surface, model, item, &env, &cfg, tracer);
        if let Some(s) = sink {
            s.record(&mut row);
        }
        row
    };

    if run_items.is_empty() {
        return Vec::new();
    }
    if concurrency <= 1 || run_items.len() <= 1 {
        return run_items.iter().copied().map(one).collect();
    }

    // Warm the system-prefix cache with the first question serially, then fan out the rest.
    eprintln!(
        "  [interactive] {} questions — warming system-prefix cache, then running at concurrency {concurrency} ...",
        run_items.len()
    );
    let (first, rest) = run_items.split_first().expect("len > 1");
    let mut out = vec![one(first)];
    let next = AtomicUsize::new(0);
    let workers = concurrency.min(rest.len()).max(1);
    let mut collected: Vec<(usize, ResultRow)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    let mut local = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= rest.len() {
                            break;
                        }
                        local.push((i, one(rest[i])));
                    }
                    local
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    collected.sort_by_key(|(i, _)| *i);
    out.extend(collected.into_iter().map(|(_, r)| r));
    out
}

#[cfg(test)]
mod tests {
    // `Reply`, `Usage`, `Model`, `Status`, `ResultRow`, `AtomicUsize`/`Ordering`,
    // `answer_with_retry`, and `summarize` all come in via the parent's imports.
    use super::*;

    /// Always returns an empty completion (a failed/empty provider body).
    struct EmptyModel;
    impl Model for EmptyModel {
        fn name(&self) -> &str {
            "empty"
        }
        fn answer(&self, _prompt: &str) -> Reply {
            Reply {
                text: String::new(),
                reasoning: None,
                usage: Usage::default(),
                latency_ms: 0,
            }
        }
    }

    /// Empty on the first call, then a real answer — exercises the single retry.
    struct FlakyModel {
        calls: AtomicUsize,
    }
    impl Model for FlakyModel {
        fn name(&self) -> &str {
            "flaky"
        }
        fn answer(&self, _prompt: &str) -> Reply {
            let n = self.calls.fetch_add(1, Ordering::Relaxed);
            let text = if n == 0 {
                String::new()
            } else {
                "Answer: 5".to_string()
            };
            Reply {
                text,
                reasoning: None,
                usage: Usage::default(),
                latency_ms: 0,
            }
        }
    }

    #[test]
    fn empty_completion_after_retry_is_error_not_scored() {
        // B4: a persistently-empty completion is flagged empty (→ `Status::Error`), so the caller
        // records it as a failed call excluded from the denominator, NOT a scored `invalid`.
        let (_reply, empty) = answer_with_retry(&EmptyModel, "prefix", "tail");
        assert!(
            empty,
            "an empty completion after the retry must be flagged as an error"
        );
    }

    #[test]
    fn empty_completion_is_retried_once_and_recovers() {
        // B4: the first (empty) call is retried once; the second returns a real answer, so it is a
        // scored trial — and exactly two calls were made.
        let m = FlakyModel {
            calls: AtomicUsize::new(0),
        };
        let (reply, empty) = answer_with_retry(&m, "prefix", "tail");
        assert!(!empty, "a non-empty retry must be treated as a real answer");
        assert_eq!(reply.text, "Answer: 5");
        assert_eq!(
            m.calls.load(Ordering::Relaxed),
            2,
            "exactly one retry (2 calls total)"
        );
    }

    #[test]
    fn error_rows_are_excluded_from_the_accuracy_denominator() {
        // B4: `error` rows must not deflate accuracy — the denominator counts only scored trials.
        let mk = |status: Status| ResultRow {
            model: "m".into(),
            think: false,
            effort: String::new(),
            encoding: "raw".into(),
            board: "b".into(),
            item_id: "i".into(),
            category: "terrain".into(),
            tier: "T0".into(),
            status,
            expected: "x".into(),
            got: "x".into(),
            prompt_tokens: 0,
            completion_tokens: 0,
            cached_tokens: 0,
            latency_ms: 0,
            n_turns: 1,
            n_tool_calls: 0,
        };
        let rows = vec![mk(Status::Correct), mk(Status::Wrong), mk(Status::Error)];
        let summary = summarize(&rows);
        // 1 correct out of 2 SCORED (the error is not in the denominator), and it is tallied.
        assert!(summary.contains("accuracy 1/2 = 0.500"), "got: {summary}");
        assert!(
            summary.contains("error 1"),
            "error must be surfaced, got: {summary}"
        );
    }

    /// A tiny all-Grassland board just big enough for `overview` to render four quadrants.
    fn scrub_test_board() -> Board {
        use std::collections::{BTreeSet, HashMap};
        let mut tiles = Vec::new();
        for y in 0..4i32 {
            let mut row = Vec::new();
            for x in 0..4i32 {
                row.push(civ_core::Tile {
                    x,
                    y,
                    terrain: "Grassland".to_string(),
                    extras: BTreeSet::new(),
                    owner: None,
                });
            }
            tiles.push(row);
        }
        Board {
            width: 4,
            height: 4,
            tiles,
            cities: vec![],
            units: vec![],
            players: vec![civ_core::Player {
                id: 0,
                name: "A".to_string(),
                nation: String::new(),
                is_alive: true,
            }],
            known: HashMap::new(),
            visibility: None,
            ruleset: None,
            turn: None,
            source: "scrub-test".to_string(),
        }
    }

    #[test]
    fn withhold_scrubs_tool_from_interactive_system_prompt() {
        // The confound fix: `--withhold scan_grid` must remove the tool from the PROSE (preamble +
        // overview), not just the JSON tool menu — else the model is told to call a verb it cannot.
        // Tested on the plain `interactive` surface, which still exposes scan_grid/get_tile in its
        // prose (interactive-maxops has since consolidated its terrain perception to scan_region only,
        // so scan_grid/get_tile are no longer in its prose to scrub).
        use crate::encoders::Interactive;
        let board = scrub_test_board();
        let surface = Interactive;
        // Assemble exactly the way `run_interactive` does: scrub(preamble + overview).
        let assemble = |withhold: &[String]| {
            interactive_system_base(&surface.system_preamble(), &surface.overview(&board), withhold)
        };

        // Control arm (withhold = []): the prompt is unchanged — both tools fully present.
        let full = assemble(&[]);
        assert_eq!(
            full,
            format!("{}{}", surface.system_preamble(), surface.overview(&board)),
            "an empty withhold set must leave the assembled prompt byte-for-byte unchanged"
        );
        assert!(full.contains("scan_grid"), "control keeps scan_grid");
        assert!(full.contains("get_tile"), "control keeps get_tile");

        // Withhold arm: ZERO mentions of the withheld tool anywhere in the assembled prompt.
        let scrubbed = assemble(&["scan_grid".to_string()]);
        assert!(
            !scrubbed.contains("scan_grid"),
            "withheld tool must not appear anywhere in the prompt:\n{scrubbed}"
        );
        // The scrub is not over-broad: a non-withheld perception tool survives.
        assert!(
            scrubbed.contains("get_tile"),
            "a non-withheld perception tool must survive the scrub"
        );
        // The leftover prose stays clean (no collapsed-slash or dangling-clause artifacts).
        assert!(!scrubbed.contains("//"), "no doubled slash: {scrubbed}");
        assert!(!scrubbed.contains("use ;"), "no dangling clause: {scrubbed}");
        assert!(
            !scrubbed.contains(" ;"),
            "no orphaned separator: {scrubbed}"
        );
    }
}
