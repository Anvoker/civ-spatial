//! `civ` — command-line entry point for the CivSpatial eval.
//!
//! Subcommands:
//!   dump-board   --board P                     parse a save; print stats + ASCII overview
//!   gen-questions --board P [--seed N] [--per-kind K]
//!   verify-oracle --board P [--seed N] [--per-kind K] [--encoding E ...]
//!   run          --board P [--seed N] [--per-kind K] [--encoding E ...] [--model M] [--out F]

mod viewer_export;

use std::collections::HashMap;
use std::process::ExitCode;

use civ_core::parse_save;
use civ_eval::encoders::{encoder_by_name, AsciiEncoder, Encoder};
use civ_eval::{
    board_block_tokens, generate_questions_fogged, run_oracle, summarize, Difficulty, EvalItem,
    ResultRow, Status, DEFAULT_ENCODERS, QUERYABLE_SURFACES, V1_ENCODERS,
};

#[cfg(feature = "remote")]
use civ_eval::remote::{Effort, OpenAiCompatible};
#[cfg(feature = "remote")]
use civ_eval::{RunManifest, Tracer};
#[cfg(feature = "remote")]
use std::path::{Path, PathBuf};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let flags = Flags::parse(&args[1..]);

    let result = match cmd.as_str() {
        "dump-board" => cmd_dump(&flags),
        "gen-questions" => cmd_gen(&flags),
        "verify-oracle" => return cmd_verify(&flags),
        "ping-model" => cmd_ping(&flags),
        "run" => cmd_run(&flags),
        "rescore" => cmd_rescore(&flags),
        "viewer-export" => cmd_viewer_export(&flags),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
civ — CivSpatial eval CLI

USAGE:
  civ dump-board    --board <save>
  civ gen-questions --board <save> [--seed N] [--per-kind K] [--kinds a,b,c]
  civ verify-oracle --board <save> [--seed N] [--per-kind K] [--kinds a,b,c] [--encoding E ...]
  civ ping-model    --base-url <url> --model <id> [--api-key K] [--provider P]   (needs `--features remote`)
  civ run           --board <save> [--seed N] [--per-kind K] [--kinds a,b,c] [--encoding E ...] [--model M] [--out F]
  civ rescore       --traces <dir>                 re-extract+re-score recorded completions offline (no model calls)
                    [--base-url <url>] [--api-key K] [--think on|off] [--provider P] [--cache on]
                    [--reasoning-effort low|medium|high] [--difficulty easy|hard] [--concurrency N]
                    [--trace-dir <path>] [--no-trace] [--resume | --fresh]
                    [--legacy-rs-desc] [--withhold <tool> ...] [--frontload-roster]  (interactive arm modifiers)
  civ viewer-export --trace-dir <path> [--out F] [--saves-dir D] [--no-fog]
                    Turn a completed run's trace dir into one self-contained JSON for the replay
                    viewer (see viewer/). Read-only, offline; default --out viewer-export.json,
                    default --saves-dir data/saves. --no-fog ignores any #fog(pN) provenance so the
                    primary board renders at full visibility (fog-independent sample).

Encodings: raw, ascii, adjacency, egocentric, hierarchical (static), + interactive / raw-ops /
           interactive-ops / raw-maxops / interactive-maxops / raw-maxops-enum /
           interactive-maxops-enum / raw-calc / raw-calc-enum (queryable tool loops).
           `run` defaults to raw+ascii+hierarchical.
           Adjacency/egocentric are opt-in. The tool-loop surfaces need a live model
           (--features remote): `interactive` = fetch verbs; `raw-ops` = full raw board + the
           PRIMITIVE calculator (distance/travel_turns/count, NO fetch
           verbs); `interactive-ops` = fetch verbs PLUS those primitives; `raw-maxops` = full raw
           board + the MAXIMAL measurement calculator (the primitives plus att_eff/def_eff/
           city_defense/garrison_defense/threat/site_axes/reach_turns, NO fetch verbs — the decision
           corpus arm); `interactive-maxops` = fetch verbs PLUS the maximal calculator, with the
           occupant ROSTER front-loaded as standard and scan_region (per-tile terrain, fog-aware) as
           the SOLE terrain-perception verb — region_summary, list_cities/list_units AND
           scan/get_tile/scan_grid are dropped from the menu (scan/get_tile used wastefully;
           scan_grid's hidden-force edge was a scan_region fog-fidelity gap, now fixed);
           `raw-maxops-enum` = full raw board + the maximal calculator with count
           retired for list_tiles PLUS the enumeration/index group (list_tiles/
           list_owned_cities/list_owned_units, NO fetch verbs — the primary enumeration arm that
           tests whether positional enumeration breaks the LOCATING ceiling); `interactive-maxops-enum`
           = the same interactive-maxops fetch surface PLUS that enum calculator, EXCEPT
           list_owned_cities/list_owned_units are dropped there (redundant with the front-loaded
           roster). `raw-calc` = full raw board + ONLY the
           spatial-logistics calculator (distance/travel_turns/count/
           reach_turns — raw-ops plus reach_turns, NO combat/valuation ops, NO fetch verbs — the
           clean dispersed-kind arm); `raw-calc-enum` = that spatial calculator with count_* retired
           for list_tiles PLUS the enumeration group (distance/travel_turns/reach_turns/list_tiles/
           list_owned_cities/list_owned_units). Tune with --max-turns N (default 16) and
           --token-budget N (default 200000). verify-oracle covers adjacency.
--kinds a,b,c  restrict to these question categories (default: all). Categories: terrain, adjacency,
               direction, distance, nearest, region-count, reachability, unit-strength, city-defense, best-site,
               nearest-owned, settle-site, reachable-nearest, t3-retreat, t3-threat, cf-vacate,
               adv-assault-target, triage-reinforce, compare-two-attacks, constraint-site, forward-posting.
               The minimal profile uses --kinds terrain,region-count,nearest (the discriminating set).
--difficulty easy|hard  question-parameter tier (default hard: larger radii/paths/rays).
Model: oracle (default). A live OpenAI-compatible model id + --base-url needs `--features remote`.
--base-url accepts http:// (local: LM Studio/Ollama) or https:// (cloud: OpenRouter/OpenAI).
--provider P  pins one OpenRouter upstream provider (no fallback) to keep a comparison controlled.
--think on|off  portable reasoning toggle. On OpenRouter it emits reasoning:{enabled:bool} (honored
                across DeepSeek/Gemini/Claude); on local hosts it emits reasoning_effort:\"none\" on off.
--reasoning-effort low|medium|high  graded reasoning override (OpenRouter reasoning:{effort:...}).
--concurrency N  parallel in-flight calls (default 1). Cloud only — big wall-clock win; the board
                 cache is warmed per encoding first so hits survive. Keep 1 for local LM Studio.
--crop x0,y0,w,h  restricts to a re-based sub-board (shrinks denser encodings to fit small contexts).
--fog <player-id>  three-state fog-of-war from that player's perspective: unexplored -> Unknown;
                   explored-but-unwatched -> terrain remembered, live enemy units hidden, enemy
                   cities last-known (tagged \"(fogged)\"); in-sight -> full truth. Perspective must
                   be a living, real civilization (not dead/barbarian) unless --allow-any-fog-player.
--allow-ruleset  bypass the hard rulesetdir==\"classic\" preflight (constants are classic-specific).
--allow-any-fog-player  bypass the --fog dead/non-civ perspective guardrail (default closed).
--trace-dir <path>  where to persist full LLM I/O trajectories (one JSON per question). Default:
                    traces/<out-stem>-<run-id>/. Live models only; the Oracle path is never traced.
--no-trace  disable trajectory tracing for this run.
Resume/overwrite of --out (results are written+flushed one JSONL line per trial as they complete, so
a crashed run is never lost). A trial is identified by the 6-tuple (encoding, item_id, board, model,
think, effort); a \"match\" means --out already holds a row with the same 6-key as a trial this run
would produce. Resume is NOT the default:
  --resume  skip trials already in --out and run only the missing ones, appending them. ERRORS (and
            does nothing) if NO trial matches — a changed model/think/effort or the wrong file. A
            partial match (some rows present, some missing) is the normal resume case, not an error.
  --fresh   truncate --out and rerun every trial.
  neither   ordinary first run (fresh write). If --out ALREADY has matching results, ERRORS and does
            nothing, asking you to pass --resume (continue) or --fresh (overwrite).
Interactive arm modifiers (composable; only affect the tool-loop encodings). They transform the tool
menu / overview at run time so a 4-arm A/B on interactive-maxops runs in one sweep:
--legacy-rs-desc   restore the pre-redescribe (\"Qualitative…\") region_summary description (control arm).
--withhold <tool>  remove a tool from the interactive menu; repeatable (e.g. --withhold scan_grid).
--frontload-roster prepend the list_units/list_cities directory to the overview (roster given for free).
                   No-op on interactive-maxops/-enum, which front-load the roster as standard.";

// --- tiny flag parser -----------------------------------------------------
struct Flags {
    single: HashMap<String, String>,
    encodings: Vec<String>,
    /// Repeatable `--withhold <tool>` — tool names to drop from the interactive menu (see the
    /// interactive arm modifiers in [`civ_eval::InteractiveConfig`]). Only consumed by the
    /// remote-gated `interactive_config`, so it is dead in the offline-only build.
    #[cfg_attr(not(feature = "remote"), allow(dead_code))]
    withhold: Vec<String>,
}

impl Flags {
    fn parse(args: &[String]) -> Self {
        let mut single = HashMap::new();
        let mut encodings = Vec::new();
        let mut withhold = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let a = &args[i];
            if let Some(key) = a.strip_prefix("--") {
                // Valueless boolean flags; don't swallow the following token.
                if matches!(
                    key,
                    "no-trace"
                        | "no-fog"
                        | "allow-ruleset"
                        | "allow-any-fog-player"
                        | "legacy-rs-desc"
                        | "frontload-roster"
                        | "resume"
                        | "fresh"
                ) {
                    single.insert(key.to_string(), "1".to_string());
                    i += 1;
                    continue;
                }
                let val = args.get(i + 1).cloned().unwrap_or_default();
                if key == "encoding" {
                    encodings.push(val);
                } else if key == "withhold" {
                    withhold.push(val);
                } else {
                    single.insert(key.to_string(), val);
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        Flags {
            single,
            encodings,
            withhold,
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.single.get(key).map(String::as_str)
    }

    fn board(&self) -> Result<String, String> {
        self.get("board")
            .map(str::to_string)
            .ok_or_else(|| "missing --board <save>".to_string())
    }

    fn seed(&self) -> u64 {
        self.get("seed").and_then(|s| s.parse().ok()).unwrap_or(1)
    }

    fn per_kind(&self) -> usize {
        self.get("per-kind")
            .and_then(|s| s.parse().ok())
            .unwrap_or(12)
    }

    /// `--kinds a,b,c` — restrict the generated set to these question categories (comma-separated).
    /// Absent = all kinds. The minimal-profile runs use this to skip the saturated categories and
    /// only pay tokens for the discriminating ones (terrain, region-count, nearest). Names must
    /// match [`civ_eval::Question::category`]; validated by [`apply_kinds_filter`].
    fn kinds(&self) -> Option<Vec<String>> {
        self.get("kinds").map(|s| {
            s.split(',')
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect()
        })
    }

    /// `--concurrency N` — parallel in-flight calls (cloud only; keep 1 for local LM Studio).
    /// Only reachable from the `remote` run path, so it carries the same gate to stay
    /// dead-code-clean in the default offline build.
    #[cfg(feature = "remote")]
    fn concurrency(&self) -> usize {
        self.get("concurrency")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
            .max(1)
    }

    /// Interactive tool-loop knobs (`--max-turns`, `--token-budget`); defaults from
    /// [`civ_eval::InteractiveConfig::default`]. Only used by the `--encoding interactive` path.
    #[cfg(feature = "remote")]
    fn interactive_config(&self) -> civ_eval::InteractiveConfig {
        let mut cfg = civ_eval::InteractiveConfig::default();
        if let Some(n) = self.get("max-turns").and_then(|s| s.parse().ok()) {
            cfg.max_turns = n;
        }
        if let Some(n) = self.get("token-budget").and_then(|s| s.parse().ok()) {
            cfg.token_budget = n;
        }
        // Composable interactive arm modifiers (default = current behavior). These express the 4-arm
        // A/B on interactive-maxops: A=--legacy-rs-desc, B=(none), C=--withhold scan_grid,
        // D=--withhold scan_grid --frontload-roster.
        cfg.legacy_rs_desc = self.get("legacy-rs-desc").is_some();
        cfg.frontload_roster = self.get("frontload-roster").is_some();
        cfg.withhold = self.withhold.clone();
        cfg
    }

    /// `--reasoning-effort low|medium|high` — graded reasoning override (overrides `--think`).
    /// On OpenRouter this emits `reasoning:{effort:...}`; elsewhere `reasoning_effort:...`. Only
    /// reachable from the `remote` run path, so it carries the same gate to stay dead-code-clean
    /// in the default offline build.
    #[cfg(feature = "remote")]
    fn reasoning_effort(&self) -> Result<Option<Effort>, String> {
        match self.get("reasoning-effort") {
            None => Ok(None),
            Some(name) => Effort::from_name(name).map(Some).ok_or_else(|| {
                format!("unknown --reasoning-effort {name:?} (have: low, medium, high)")
            }),
        }
    }

    /// `--difficulty easy|hard` (default hard — the regime that clears the small-board ceiling).
    fn difficulty(&self) -> Result<Difficulty, String> {
        match self.get("difficulty") {
            None => Ok(Difficulty::hard()),
            Some(name) => Difficulty::from_name(name)
                .ok_or_else(|| format!("unknown --difficulty {name:?} (have: easy, hard)")),
        }
    }

    /// Explicit `--encoding E ...` if given, else the caller-supplied `default` set. `run` passes
    /// [`DEFAULT_ENCODERS`] (raw+ascii — adjacency is opt-in); `verify-oracle` passes [`V1_ENCODERS`]
    /// so the offline gate keeps covering adjacency's contract even though it no longer runs by default.
    fn encoding_names(&self, default: &[&str]) -> Vec<String> {
        if self.encodings.is_empty() {
            default.iter().map(|s| s.to_string()).collect()
        } else {
            self.encodings.clone()
        }
    }

    /// Parse `--crop x0,y0,w,h` into four integers, if present.
    fn crop(&self) -> Result<Option<(i32, i32, i32, i32)>, String> {
        let Some(spec) = self.get("crop") else {
            return Ok(None);
        };
        let nums: Vec<i32> = spec
            .split(',')
            .map(|s| s.trim().parse::<i32>())
            .collect::<Result<_, _>>()
            .map_err(|_| format!("bad --crop {spec:?}; expected x0,y0,w,h"))?;
        match nums.as_slice() {
            [x0, y0, w, h] => Ok(Some((*x0, *y0, *w, *h))),
            _ => Err(format!("--crop needs 4 numbers x0,y0,w,h (got {spec:?})")),
        }
    }

    /// Parse `--fog <player-id>` — mask the board to that player's known (explored) tiles (fog of
    /// war). Absent = the full omniscient map.
    fn fog(&self) -> Result<Option<i32>, String> {
        match self.get("fog") {
            None => Ok(None),
            Some(s) => s
                .trim()
                .parse::<i32>()
                .map(Some)
                .map_err(|_| format!("bad --fog {s:?}; expected a player id like 0")),
        }
    }

    /// `--allow-ruleset` — bypass the hard `rulesetdir == "classic"` preflight (our solver constants
    /// are `classic`-specific). Default closed.
    fn allow_ruleset(&self) -> bool {
        self.get("allow-ruleset").is_some()
    }

    /// `--allow-any-fog-player` — bypass the fog-perspective guardrail that rejects a dead or
    /// non-civilization (barbarian/pirate/animal) `--fog` target. Default closed.
    fn allow_any_fog_player(&self) -> bool {
        self.get("allow-any-fog-player").is_some()
    }

    /// Resume/overwrite policy for `--out` from `--resume` / `--fresh` (mutually exclusive). Absent
    /// = the ordinary first-run policy ([`civ_eval::ResumeMode::Default`], which refuses to clobber a
    /// file that already has matching results).
    fn resume_mode(&self) -> Result<civ_eval::ResumeMode, String> {
        match (self.get("resume").is_some(), self.get("fresh").is_some()) {
            (true, true) => Err("--resume and --fresh are mutually exclusive".to_string()),
            (true, false) => Ok(civ_eval::ResumeMode::Resume),
            (false, true) => Ok(civ_eval::ResumeMode::Fresh),
            (false, false) => Ok(civ_eval::ResumeMode::Default),
        }
    }

    /// The reasoning toggle in force (`--think`, default on). Part of the 6-tuple resume key.
    fn think(&self) -> bool {
        self.get("think") != Some("off")
    }

    /// The graded reasoning effort (`--reasoning-effort`) as a canonical string, or `"none"`. Part
    /// of the 6-tuple resume key; the CLI keeps it as a plain string so the offline build (no
    /// `Effort` type) shares the same key logic.
    fn effort_key(&self) -> String {
        self.get("reasoning-effort").unwrap_or("none").to_string()
    }
}

/// Known question categories — must stay in sync with [`civ_eval::Question::category`]. Used to
/// validate `--kinds` so a typo fails loudly instead of silently generating an empty question set.
const ALL_CATEGORIES: &[&str] = &[
    "terrain",
    "adjacency",
    "direction",
    "distance",
    "nearest",
    "region-count",
    "reachability",
    "unit-strength",
    "city-defense",
    "best-site",
    "nearest-owned",
    "settle-site",
    "reachable-nearest",
    "t3-retreat",
    "t3-threat",
    "forward-posting",
    "cf-vacate",
    "adv-assault-target",
    "triage-reinforce",
    "compare-two-attacks",
    "constraint-site",
    "fogged-assault",
    "surprise-strike",
    "hidden-force",
];

/// Kinds retired from the DEFAULT `run` set (2026-08-13): the three reasoning-frontier hidden-info
/// kinds were audited out as information-bound (luck), not reasoning-bound — see
/// `analysis/kind-roster-preregistration.md` (amendment) + `analysis/reasoning-frontier-vs-luck.md`.
/// They stay fully in code (still generated by `all_kinds`, still Oracle-verified by `verify-oracle`,
/// still runnable via an explicit `--kinds`) — they are only dropped from a `run` that specifies no
/// `--kinds`, so the default experiment matches the re-scoped 3-arm roster.
const RETIRED_DEFAULT_KINDS: &[&str] = &["fogged-assault", "surprise-strike", "hidden-force"];

/// Apply an optional `--kinds` filter to a freshly-generated item set, validating the names first.
/// A `--kinds` entry that matches no known category is a hard error (a typo would otherwise produce
/// a silently-empty run). No `--kinds` flag = pass every item through unchanged.
fn apply_kinds_filter(items: Vec<EvalItem>, flags: &Flags) -> Result<Vec<EvalItem>, String> {
    let Some(want) = flags.kinds() else {
        return Ok(items);
    };
    if let Some(bad) = want.iter().find(|k| !ALL_CATEGORIES.contains(&k.as_str())) {
        return Err(format!(
            "unknown --kinds entry {bad:?} (have: {})",
            ALL_CATEGORIES.join(", ")
        ));
    }
    Ok(items
        .into_iter()
        .filter(|it| want.iter().any(|k| k == it.category))
        .collect())
}

/// Parse the `--board` save and apply an optional `--crop`.
fn load_board(flags: &Flags) -> Result<civ_core::Board, String> {
    Ok(load_board_pair(flags)?.0)
}

/// Load the board AND, on a fogged run, the UNMASKED board behind it. Returns `(rendered, unmasked)`
/// where `rendered` is what the model sees — masked to the fog perspective under `--fog`, else the
/// real board — and `unmasked` is `Some(true board)` only when fogged. The hidden-force family
/// (P1/P3/P7f) scores ground truth on `unmasked` while the model sees only `rendered`
/// (`reasoning-frontier-questions-v2.md` §v2.3); every other command uses just `rendered`.
fn load_board_pair(flags: &Flags) -> Result<(civ_core::Board, Option<civ_core::Board>), String> {
    let mut board = parse_save(flags.board()?).map_err(|e| e.to_string())?;

    // Ruleset preflight: our solver constants (combat, movement, vision) are `classic`-specific, so
    // a differently-ruled save would silently mismatch. Hard-error unless explicitly overridden.
    if !flags.allow_ruleset() && board.ruleset.as_deref() != Some("classic") {
        return Err(format!(
            "board ruleset is {:?}, but this harness's constants target \"classic\". \
             Pass --allow-ruleset to override (results may be wrong for a non-classic save).",
            board.ruleset
        ));
    }

    if let Some((x0, y0, w, h)) = flags.crop()? {
        board = board.crop(x0, y0, w, h)?;
    }
    if let Some(id) = flags.fog()? {
        // Fog-perspective guardrail: "self" must be a living, real civilization — computing sight
        // around a dead slot or a barbarian/pirate/animal player yields a meaningless view.
        if !flags.allow_any_fog_player() {
            let p = board
                .players
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("--fog {id}: no player with that id on this board"))?;
            if !p.is_valid_fog_perspective() {
                let why = if !p.is_alive {
                    "dead (is_alive=FALSE)"
                } else {
                    "a barbarian/pirate/animal (non-civilization) player"
                };
                return Err(format!(
                    "--fog {id} (\"{}\", nation {:?}) is {why}; the fog perspective must be a \
                     living, real civilization. Pass --allow-any-fog-player to override.",
                    p.name, p.nation
                ));
            }
        }
        let masked = board.mask_to_known(id)?;
        // Keep the pre-mask board as the unmasked ground-truth for hidden-force kinds.
        return Ok((masked, Some(board)));
    }
    Ok((board, None))
}

/// Resolve the fog perspective player's owner NAME for question generation. When `--fog P` is set,
/// every player-relative question kind must generate for exactly that player, so its decision ("you
/// are player X") matches whose sight the board is masked to (`fog-three-state-design.md` §9). The
/// question `player` field is the owner NAME, while `--fog` is a player ID, so we resolve id → name
/// via `board.players`. Absent `--fog` returns `None` (the unfogged path: random owner, unchanged).
/// The id was already validated to exist by `load_board`'s guardrail, but we re-check to fail
/// cleanly if reached independently.
fn fog_perspective_name(board: &civ_core::Board, flags: &Flags) -> Result<Option<String>, String> {
    match flags.fog()? {
        None => Ok(None),
        Some(id) => board
            .players
            .iter()
            .find(|p| p.id == id)
            .map(|p| Some(p.name.clone()))
            .ok_or_else(|| format!("--fog {id}: no player with that id on this board")),
    }
}

// --- commands -------------------------------------------------------------
fn cmd_dump(flags: &Flags) -> Result<(), String> {
    let board = load_board(flags)?;
    println!(
        "{}  {}x{}  players={:?}",
        board.source,
        board.width,
        board.height,
        board
            .players
            .iter()
            .map(|p| (&p.name, &p.nation))
            .collect::<Vec<_>>()
    );
    println!("cities={} units={}", board.cities.len(), board.units.len());

    let mut terr: HashMap<&str, u32> = HashMap::new();
    for t in board.iter_tiles() {
        *terr.entry(t.terrain.as_str()).or_default() += 1;
    }
    let mut terr: Vec<_> = terr.into_iter().collect();
    terr.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("terrain: {terr:?}");
    println!();
    print!("{}", AsciiEncoder.render(&board));
    Ok(())
}

fn cmd_gen(flags: &Flags) -> Result<(), String> {
    let (board, unmasked) = load_board_pair(flags)?;
    let perspective = fog_perspective_name(&board, flags)?;
    let items = generate_questions_fogged(
        &board,
        unmasked.as_ref(),
        flags.seed(),
        flags.per_kind(),
        flags.difficulty()?,
        perspective.as_deref(),
    );
    let items = apply_kinds_filter(items, flags)?;

    let mut by_cat: HashMap<&str, u32> = HashMap::new();
    for it in &items {
        *by_cat.entry(it.category).or_default() += 1;
    }
    let mut by_cat: Vec<_> = by_cat.into_iter().collect();
    by_cat.sort();
    println!(
        "generated {} questions (seed {}):",
        items.len(),
        flags.seed()
    );
    for (c, n) in by_cat {
        println!("  {c:<14} {n}");
    }

    // A few samples rendered under the raw encoder, with their ground-truth answers.
    let enc = encoder_by_name("raw").unwrap();
    println!("\nsamples:");
    for it in items.iter().take(8) {
        println!(
            "  [{}] {}\n      -> {}",
            it.category,
            it.question.render(enc.as_ref(), &board),
            it.answer.canonical()
        );
    }
    Ok(())
}

fn cmd_verify(flags: &Flags) -> ExitCode {
    let run = || -> Result<bool, String> {
        let (board, unmasked) = load_board_pair(flags)?;
        let perspective = fog_perspective_name(&board, flags)?;
        let items = generate_questions_fogged(
            &board,
            unmasked.as_ref(),
            flags.seed(),
            flags.per_kind(),
            flags.difficulty()?,
            perspective.as_deref(),
        );
        let items = apply_kinds_filter(items, flags)?;
        if items.is_empty() {
            return Err("no questions generated".into());
        }
        let encodings = flags.encoding_names(&V1_ENCODERS);
        let rows = run_oracle(&board, &encodings, &items, None);
        let total = rows.len();
        let bad: Vec<_> = rows
            .iter()
            .filter(|r| r.status != Status::Correct)
            .collect();
        if bad.is_empty() {
            println!(
                "verify-oracle: PASS — {} trials, {} questions × {} encodings, all correct.",
                total,
                items.len(),
                encodings.len()
            );
            Ok(true)
        } else {
            eprintln!(
                "verify-oracle: FAIL — {}/{} trials not correct:",
                bad.len(),
                total
            );
            for r in bad.iter().take(20) {
                eprintln!(
                    "  {} [{}] expected={:?} got={:?} status={}",
                    r.encoding,
                    r.item_id,
                    r.expected,
                    r.got,
                    r.status.as_str()
                );
            }
            Ok(false)
        }
    };
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The identity a run records as each row's `model` field (= `Model::name()`), used to build the
/// resume key so a live `--resume` matches its own written rows. Oracle names itself "oracle"; a live
/// model uses `model_label(id, think)` — the bare `--model` flag omits the `[think|nothink]` suffix
/// the row actually stores, which broke live resume + the refuse-clobber guard.
fn resume_model_identity(model_flag: &str, think: bool) -> String {
    if model_flag == "oracle" {
        return "oracle".to_string();
    }
    #[cfg(feature = "remote")]
    {
        civ_eval::remote::model_label(model_flag, think)
    }
    #[cfg(not(feature = "remote"))]
    {
        let _ = think; // no live model without the remote feature; the run errors later anyway
        model_flag.to_string()
    }
}

/// `rescore` — RE-SCORE already-recorded model completions through the CURRENT (fixed) extractor +
/// scorer, with NO model/API calls. Given a trace directory containing a `run.json` manifest and one
/// `<encoding>__<item_id>.json` trajectory per question, it:
///   1. reloads the exact board (applying the manifest's crop/fog), regenerates the fully-typed
///      question set from `(seed, per_kind, difficulty, kinds)` — so it recovers each item's real
///      `Answer` (option / acceptable sets), which the trace does NOT store; and
///   2. feeds each trace's FINAL assistant completion (`turns[-1].response.text`) back through the
///      real [`civ_eval::score`], emitting one TSV row of `old` vs `new` status per question.
///
/// The extraction/scoring logic is 100% the shipping Rust path (never reimplemented), so the numbers
/// match what a fresh run would now produce. Aggregation (per-kind / per-arm flip counts) is done by
/// the caller over the TSV. Output columns (tab-separated, one header line prefixed `#`):
///   run_id, encoding, board, category, item_id, old_status, old_correct, old_got, expected,
///   new_status, new_correct, new_got
fn cmd_rescore(flags: &Flags) -> Result<(), String> {
    use civ_eval::json::Json;

    let dir = flags
        .get("traces")
        .ok_or_else(|| "missing --traces <trace-dir>".to_string())?;
    let dir = std::path::Path::new(dir);
    let manifest_txt = std::fs::read_to_string(dir.join("run.json"))
        .map_err(|e| format!("reading {}/run.json: {e}", dir.display()))?;
    let m = Json::parse(&manifest_txt).map_err(|e| format!("parsing run.json: {e}"))?;

    let run_id = m.get("run_id").and_then(Json::as_str).unwrap_or("").to_string();
    let seed = m.get("seed").and_then(Json::as_u64).ok_or("run.json: seed")?;
    let per_kind = m.get("per_kind").and_then(Json::as_u64).ok_or("run.json: per_kind")? as usize;
    let diff_name = m.get("difficulty").and_then(Json::as_str).unwrap_or("hard");
    let difficulty = Difficulty::from_name(diff_name)
        .ok_or_else(|| format!("run.json: unknown difficulty {diff_name:?}"))?;
    let board_path = m
        .get("board_path")
        .and_then(Json::as_str)
        .ok_or("run.json: board_path")?;
    let kinds: Option<Vec<String>> = m.get("kinds").and_then(Json::as_array).map(|a| {
        a.iter()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect()
    });
    let fog_player = m.get("fog_player").and_then(Json::as_u64).map(|v| v as i32);
    let crop = m.get("crop").filter(|c| !c.is_null()).map(|c| {
        let g = |k: &str| c.get(k).and_then(Json::as_u64).unwrap_or(0) as i32;
        (g("x0"), g("y0"), g("w"), g("h"))
    });

    // Rebuild the exact board the run saw (mirrors `load_board_pair`, driven by the manifest rather
    // than flags; the ruleset preflight is skipped since the run already passed it).
    let mut board = parse_save(board_path).map_err(|e| e.to_string())?;
    if let Some((x0, y0, w, h)) = crop {
        board = board.crop(x0, y0, w, h)?;
    }
    let (board, unmasked, perspective) = match fog_player {
        Some(id) => {
            let name = board
                .players
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone());
            let unmasked = board.clone();
            let masked = board.mask_to_known(id)?;
            (masked, Some(unmasked), name)
        }
        None => (board, None, None),
    };

    let items = generate_questions_fogged(
        &board,
        unmasked.as_ref(),
        seed,
        per_kind,
        difficulty,
        perspective.as_deref(),
    );
    let items: Vec<EvalItem> = match &kinds {
        Some(want) => items
            .into_iter()
            .filter(|it| want.iter().any(|k| k == it.category))
            .collect(),
        None => items,
    };
    let by_id: HashMap<&str, &EvalItem> = items.iter().map(|it| (it.id.as_str(), it)).collect();

    // Reads the final assistant completion from a trace: the LAST turn's `response.text` (null → "").
    let final_text = |trace: &Json| -> String {
        trace
            .get("turns")
            .and_then(Json::as_array)
            .and_then(|t| t.last())
            .and_then(|turn| turn.get("response"))
            .and_then(|r| r.get("text"))
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string()
    };
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");

    // `faithful` = the regenerated item's canonical answer equals the recorded `expected` (so the
    // Answer is byte-identical to what the run scored). ONLY faithful rows attribute a status change
    // to the extractor fix; a non-faithful row means the ground truth itself drifted since the run
    // (e.g. the 521c0a1 reachability-oracle correction, or item-set drift) and is out of scope here.
    println!(
        "#run_id\tencoding\tboard\tcategory\titem_id\tfaithful\told_status\told_correct\told_got\texpected\tcanon\tnew_status\tnew_correct\tnew_got"
    );

    let mut n_files = 0usize;
    let mut n_unmatched = 0usize;
    let mut n_canon_mismatch = 0usize;
    let mut n_changed = 0usize;
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !fname.ends_with(".json") || fname == "run.json" {
            continue;
        }
        let txt = std::fs::read_to_string(&path).map_err(|e| format!("reading {fname}: {e}"))?;
        let trace = match Json::parse(&txt) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("warning: skipping {fname}: parse error: {e}");
                continue;
            }
        };
        n_files += 1;
        let item_id = trace.get("item_id").and_then(Json::as_str).unwrap_or("");
        let encoding = trace.get("encoding").and_then(Json::as_str).unwrap_or("");
        let category = trace.get("category").and_then(Json::as_str).unwrap_or("");
        let board_src = trace.get("board").and_then(Json::as_str).unwrap_or("");
        let result = trace.get("result");
        let old_status = result
            .and_then(|r| r.get("status"))
            .and_then(Json::as_str)
            .unwrap_or("");
        let old_got = result
            .and_then(|r| r.get("got"))
            .and_then(Json::as_str)
            .unwrap_or("");
        let expected = result
            .and_then(|r| r.get("expected"))
            .and_then(Json::as_str)
            .unwrap_or("");

        let Some(item) = by_id.get(item_id) else {
            n_unmatched += 1;
            eprintln!("warning: item_id {item_id:?} ({category}) not in regenerated set");
            println!(
                "{run_id}\t{encoding}\t{board_src}\t{category}\t{item_id}\t0\t{old_status}\t{}\t{}\t{}\t\tUNMATCHED\t\t",
                (old_status == "correct") as u8,
                clean(old_got),
                clean(expected),
            );
            continue;
        };

        // Regeneration sanity: the item's canonical answer must match the recorded `expected`.
        let canonical = item.answer.canonical();
        if canonical != expected {
            n_canon_mismatch += 1;
            eprintln!(
                "warning: canonical mismatch for {item_id}: regenerated {canonical:?} vs trace expected {expected:?}"
            );
        }

        let faithful = (canonical == expected) as u8;
        let text = final_text(&trace);
        // Mirror the runner's B4 guard: an empty final completion is a failed CALL (`error`, excluded
        // from the scored denominator), NOT an answered-wrong `invalid`. `score()` alone can't tell
        // these apart, so the empty-check must live here exactly as it does in the runner.
        let (new_status, new_got) = if text.trim().is_empty() {
            (Status::Error.as_str(), String::new())
        } else {
            let sc = civ_eval::score(&item.answer, &text);
            (sc.status.as_str(), sc.extracted)
        };
        if faithful == 1 && new_status != old_status {
            n_changed += 1;
        }
        println!(
            "{run_id}\t{encoding}\t{board_src}\t{category}\t{item_id}\t{faithful}\t{old_status}\t{}\t{}\t{}\t{}\t{new_status}\t{}\t{}",
            (old_status == "correct") as u8,
            clean(old_got),
            clean(expected),
            clean(&canonical),
            (new_status == "correct") as u8,
            clean(&new_got),
        );
    }

    eprintln!(
        "[rescore] {}: {n_files} traces, regenerated {} items ({} unmatched, {} canonical-mismatch), {n_changed} status changes",
        dir.display(),
        items.len(),
        n_unmatched,
        n_canon_mismatch,
    );
    Ok(())
}

fn cmd_run(flags: &Flags) -> Result<(), String> {
    let model = flags.get("model").unwrap_or("oracle").to_string();
    let (board, unmasked) = load_board_pair(flags)?;
    let encodings = flags.encoding_names(&DEFAULT_ENCODERS);
    for e in &encodings {
        // The queryable board-access surfaces (interactive, raw-ops, interactive-ops) are tool
        // loops, not static Encoders — handled separately in obtain_rows; every other name must be a
        // registered static encoder.
        if !QUERYABLE_SURFACES.contains(&e.as_str()) && encoder_by_name(e).is_none() {
            return Err(format!(
                "unknown encoding {e:?} (have: raw, ascii, adjacency, egocentric, hierarchical, interactive, raw-ops, interactive-ops, raw-maxops, interactive-maxops, raw-maxops-enum, interactive-maxops-enum, raw-calc, raw-calc-enum)"
            ));
        }
    }
    let perspective = fog_perspective_name(&board, flags)?;
    let items = generate_questions_fogged(
        &board,
        unmasked.as_ref(),
        flags.seed(),
        flags.per_kind(),
        flags.difficulty()?,
        perspective.as_deref(),
    );
    let explicit_kinds = flags.kinds().is_some();
    let items = apply_kinds_filter(items, flags)?;
    // With no explicit `--kinds`, drop the audited-out fog trio from the default run set (they remain
    // generatable on demand via `--kinds fogged-assault,...`). An explicit `--kinds` wins verbatim, so
    // the ceiling-theorem exhibit is still reproducible when asked for by name.
    let items: Vec<EvalItem> = if explicit_kinds {
        items
    } else {
        let before = items.len();
        let kept: Vec<EvalItem> = items
            .into_iter()
            .filter(|it| !RETIRED_DEFAULT_KINDS.contains(&it.category))
            .collect();
        if kept.len() != before {
            eprintln!(
                "[run] dropped {} audited-out fog-kind item(s) from the default set ({}); pass --kinds to include them",
                before - kept.len(),
                RETIRED_DEFAULT_KINDS.join(", ")
            );
        }
        kept
    };
    if items.is_empty() {
        return Err("no questions generated".into());
    }
    // Open the incremental JSONL sink: it applies the --resume/--fresh truth table (erroring before
    // any model call for the no-match / refuse-clobber cases) and then writes+flushes one line per
    // trial as it completes. The 6-tuple resume key is (encoding, item_id, board, model, think,
    // effort); think/effort are stamped onto every row by the sink.
    let out_path = flags.get("out").unwrap_or("results.jsonl").to_string();
    // The resume key's model component must equal what each row PERSISTS (= Model::name()), which for
    // a live model carries a `[think|nothink]` suffix the bare `--model` flag lacks. Keying on the
    // flag made a live `--resume` match nothing (and the refuse-clobber guard silently truncate +
    // re-spend); key on the real recorded identity instead.
    let model_name = resume_model_identity(&model, flags.think());
    let sink = civ_eval::RunSink::open(
        &out_path,
        flags.resume_mode()?,
        &encodings,
        &items,
        &model_name,
        flags.think(),
        &flags.effort_key(),
    )?;
    let rows = obtain_rows(flags, &model, &board, &encodings, &items, &sink)?;
    let skipped = sink.skipped();

    println!(
        "ran {} new trials ({} questions × {} encodings; {} resumed/skipped) with model '{}' on {}",
        rows.len(),
        items.len(),
        encodings.len(),
        skipped,
        model,
        board.source
    );
    println!("wrote {out_path} (append+flush per trial)\n");
    // Board-block size is a static-encoder concept; the queryable surfaces have no single block.
    let static_encs: Vec<String> = encodings
        .iter()
        .filter(|e| !QUERYABLE_SURFACES.contains(&e.as_str()))
        .cloned()
        .collect();
    println!("Board-block size by encoding (the cost axis, questions aside):");
    for (e, t) in board_block_tokens(&board, &static_encs) {
        println!("  {e:<10} ~{t} tokens");
    }
    println!();
    print!("{}", summarize(&rows));
    Ok(())
}

// --- model selection (oracle offline, or a live provider behind the `remote` feature) -----
#[cfg(feature = "remote")]
fn obtain_rows(
    flags: &Flags,
    model: &str,
    board: &civ_core::Board,
    encodings: &[String],
    items: &[civ_eval::EvalItem],
    sink: &civ_eval::RunSink,
) -> Result<Vec<ResultRow>, String> {
    let queryable_encs: Vec<String> = encodings
        .iter()
        .filter(|e| QUERYABLE_SURFACES.contains(&e.as_str()))
        .cloned()
        .collect();
    if model == "oracle" {
        if !queryable_encs.is_empty() {
            return Err(format!(
                "the queryable surface(s) {queryable_encs:?} need a live model (not the Oracle)"
            ));
        }
        return Ok(run_oracle(board, encodings, items, Some(sink)));
    }
    let base = flags
        .get("base-url")
        .ok_or("a live model needs --base-url (e.g. http://localhost:1234/v1)")?;
    let key = flags.get("api-key").map(str::to_string);
    let think = flags.get("think") != Some("off");
    let provider = flags.get("provider").map(str::to_string);
    let cache = flags.get("cache") == Some("on");
    let effort = flags.reasoning_effort()?;
    let m = OpenAiCompatible::from_base_url(base, model, key, think)?
        .with_provider(provider)
        .with_cache(cache)
        .with_reasoning_effort(effort);
    let concurrency = flags.concurrency();
    // Trajectory tracing is on by default for the live run path (disable with --no-trace).
    let tracer = build_tracer(flags);

    // Persist-everything: write the per-run manifest ONCE into the trace dir, so a consumer can
    // regenerate the fully-typed questions from `run.json` + the `.sav` and match them to traces by
    // `item_id`. Never records the API key / base URL. Skipped when tracing is off.
    if let Some(t) = tracer.as_ref() {
        RunManifest {
            run_id: run_id_from_dir(t.dir()),
            seed: flags.seed(),
            per_kind: flags.per_kind() as u64,
            difficulty: flags.get("difficulty").unwrap_or("hard").to_string(),
            kinds: flags.kinds(),
            board_path: flags.board()?,
            board_source: board.source.clone(),
            crop: flags.crop()?,
            fog_player: flags.fog()?,
            encodings: encodings.to_vec(),
            model: model.to_string(),
            think,
            concurrency: concurrency as u64,
        }
        .write_to(t.dir());
    }

    // Static encoders go through the one-shot matrix; each queryable surface (interactive,
    // raw-ops, interactive-ops) is driven by the multi-turn tool loop. Both use the same live
    // model; rows are concatenated.
    let static_encs: Vec<String> = encodings
        .iter()
        .filter(|e| !QUERYABLE_SURFACES.contains(&e.as_str()))
        .cloned()
        .collect();
    let mut rows = Vec::new();
    if !static_encs.is_empty() {
        eprintln!(
            "running {} static trials against '{model}' (think={}, concurrency={concurrency}) at {base} ...",
            items.len() * static_encs.len(),
            if think { "on" } else { "off" }
        );
        rows.extend(civ_eval::run(
            board,
            &static_encs,
            items,
            &m,
            concurrency,
            tracer.as_ref(),
            Some(sink),
        ));
    }
    for enc in &queryable_encs {
        let surface = civ_eval::queryable_surface(enc)
            .ok_or_else(|| format!("unknown queryable surface {enc:?}"))?;
        let cfg = flags.interactive_config();
        eprintln!(
            "running {} {enc} trials against '{model}' (max_turns={}, token_budget={}, concurrency={concurrency}) at {base} ...",
            items.len(),
            cfg.max_turns,
            cfg.token_budget
        );
        rows.extend(civ_eval::run_interactive(
            board,
            &*surface,
            &m,
            items,
            cfg,
            concurrency,
            tracer.as_ref(),
            Some(sink),
        ));
    }
    Ok(rows)
}

/// Build the per-run trajectory tracer for the live `run` path. On by default; `--no-trace`
/// disables it, `--trace-dir <path>` overrides the location. The default location is a per-run
/// subdir of `traces/` derived from the `--out` basename plus a wall-clock run id, so repeated runs
/// don't clobber each other. A directory-creation failure disables tracing (warns) rather than
/// aborting the run.
#[cfg(feature = "remote")]
fn build_tracer(flags: &Flags) -> Option<Tracer> {
    if flags.get("no-trace").is_some() {
        return None;
    }
    let dir = match flags.get("trace-dir") {
        Some(d) => PathBuf::from(d),
        None => default_trace_dir(flags),
    };
    let tracer = Tracer::new(dir);
    if let Some(t) = &tracer {
        eprintln!(
            "tracing full LLM I/O to {} (one JSON per question; --no-trace to disable)",
            t.dir().display()
        );
    }
    tracer
}

/// Default trace directory: `traces/<out-stem>-<run-id>`, where run-id is milliseconds since the
/// Unix epoch. Keeps each run's traces grouped and non-colliding.
#[cfg(feature = "remote")]
fn default_trace_dir(flags: &Flags) -> PathBuf {
    let out = flags.get("out").unwrap_or("results.jsonl");
    let stem = Path::new(out)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("run");
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    PathBuf::from("traces").join(format!("{stem}-{run_id}"))
}

/// Recover the run-id from a trace directory. For the default `traces/<stem>-<run-id>` layout the
/// run-id is the trailing all-digits segment; for a custom `--trace-dir` the whole final component
/// is used as the id. Empty/degenerate paths fall back to `"run"`.
#[cfg(feature = "remote")]
fn run_id_from_dir(dir: &Path) -> String {
    let name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("run");
    if let Some((_, tail)) = name.rsplit_once('-') {
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
            return tail.to_string();
        }
    }
    if name.is_empty() {
        "run".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(not(feature = "remote"))]
fn obtain_rows(
    _flags: &Flags,
    model: &str,
    board: &civ_core::Board,
    encodings: &[String],
    items: &[civ_eval::EvalItem],
    sink: &civ_eval::RunSink,
) -> Result<Vec<ResultRow>, String> {
    if let Some(q) = encodings
        .iter()
        .find(|e| QUERYABLE_SURFACES.contains(&e.as_str()))
    {
        return Err(format!(
            "the {q} surface needs a live provider; rebuild with `--features remote`"
        ));
    }
    if model == "oracle" {
        Ok(run_oracle(board, encodings, items, Some(sink)))
    } else {
        Err(format!(
            "model {model:?} needs a live provider; rebuild with `--features remote`"
        ))
    }
}

/// `viewer-export` — turn a completed run's trace dir into one self-contained JSON for the offline
/// replay viewer. Additive and read-only; see `viewer_export.rs`.
fn cmd_viewer_export(flags: &Flags) -> Result<(), String> {
    let trace_dir = flags
        .get("trace-dir")
        .ok_or("missing --trace-dir <path> (a run's traces/<stem>-<runid>/ directory)")?;
    let out = flags.get("out").unwrap_or("viewer-export.json");
    let saves_dir = flags.get("saves-dir").unwrap_or("data/saves");
    // `--no-fog` strips any `#fog(pN)` provenance so the primary board renders at full visibility
    // (used to build a fog-independent bundled sample; no perspective / no `unfogged` companion).
    let no_fog = flags.get("no-fog").is_some();
    viewer_export::run(trace_dir, out, saves_dir, no_fog)
}

#[cfg(feature = "remote")]
fn cmd_ping(flags: &Flags) -> Result<(), String> {
    use civ_eval::Model;
    let base = flags.get("base-url").ok_or("missing --base-url")?;
    let model = flags.get("model").ok_or("missing --model <id>")?;
    let key = flags.get("api-key").map(str::to_string);
    let think = flags.get("think") != Some("off");
    let provider = flags.get("provider").map(str::to_string);
    let m = OpenAiCompatible::from_base_url(base, model, key, think)?.with_provider(provider);
    let r = m.answer("Reply with exactly the single word: pong");
    println!("reply: {:?}", r.text);
    println!(
        "usage: prompt={} completion={} tokens   latency={} ms",
        r.usage.prompt_tokens, r.usage.completion_tokens, r.latency_ms
    );
    if r.text.trim().is_empty() {
        return Err("empty reply — check the base URL / that the model is loaded".into());
    }
    Ok(())
}

#[cfg(not(feature = "remote"))]
fn cmd_ping(_flags: &Flags) -> Result<(), String> {
    Err("ping-model needs a live provider; rebuild with `--features remote`".into())
}
