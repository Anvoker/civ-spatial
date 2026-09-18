# Findings — Multi-board corpus for the robustness experiment

**Recommendation in one line:** Build the corpus by **scripted local Freeciv AI self-play** (guarantees `classic` by construction, fully controllable/reproducible, portfolio-worthy), and add a small `rulesetdir` gate to the parser as defense-in-depth. Downloading public saves is a dead end for our hard constraint.

## 1. How to detect/verify a save's ruleset is `classic`

The authoritative field exists and is trivial. Both our saves are **plain text** (start `[scenario]`, not gzip magic `1f 8b`), and carry in their **`[savefile]`** section:

```
rulesetdir="classic"
```

A civ2civ3 save reads `rulesetdir="civ2civ3"`. **This is the single field to gate on.** Supporting provenance fields (verified in both saves): `game_version=3029002` (=3.2.90) in `[scenario]`; `revision`/`orig_version="3.2.90.2-dev"` and save-format `version=70` in `[savefile]`; `turn=51` in `[game]` (for early/mid/late characterization); `xsize`/`ysize` in `[settings]` (78×52 for T50, 84×56 for T677 — also derivable from the map grid).

**Where our parser reads it / what's needed.** `civ-core/src/parser.rs` **already loads `[savefile]`** (`let savefile = sections.get("savefile")…`, line 55) and has the exact helper — `get_kv(savefile, "rulesetdir")` (same `get_kv` used for `extras_vector`). **We do not currently read it**; `Board` (`board.rs:91`) has no `ruleset` (or `turn`) field. Concrete change (~15 lines):
1. `board.rs`: add `pub ruleset: Option<String>` (and optionally `pub turn: Option<i32>`).
2. `parser.rs`: `let ruleset = get_kv(savefile, "rulesetdir")…`; read `turn` from `sections.get("game")`.
3. **Gate:** in `run`/`gen-questions` preflight (or a new `verify-ruleset` subcommand), hard-**error** (not warn) if `ruleset != Some("classic")`. Optional `--allow-ruleset` escape hatch, default closed. Also assert `game_version` is 3.2.x (a future rebalanced `classic` would pass the name gate but drift our hard-coded constants — low near-term risk).

Until that lands, gate out-of-band at harvest with `grep '^rulesetdir=' board.sav` — sufficient to build the corpus now; the in-parser gate is the durable fix.

## 2. Strategy head-to-head

**Strategy A — local AI self-play (server autogame).** Freeciv is **not installed locally** (no `freeciv-server`/`civserver` on PATH, no install dir), so step 0 is a one-time Windows install from freeciv.org (a 3.2.x build matches our saves' format). Mechanism: `freeciv-server -r <script.serv> -e` reads a console-command script and exits on game end (`-e/--exit-on-end`). An all-AI game plays itself. `rulesetdir classic` in the script **guarantees the gate by construction** — no detection needed. Example script:

```
rulesetdir classic
hard
set aifill 7           # fill slots with AI; vary 3..10 for density
set size 4             # or xsize/ysize; vary for size diversity
set mapseed 12345
set gameseed 12345
set endturn 220        # hard stop
set saveturns 20       # autosave every 20 turns -> early/mid/late ladder from ONE game
set savename "classic_s12345_a7"
set timeout 1          # >0 so an all-AI game advances without a client
start
```

`mapseed`+`gameseed` make runs deterministic → commit the tiny `.serv` scripts, regenerate the `.sav` blobs on demand. One game yields snapshots at every 20th turn (early ~T25-40, mid ~T90-120, late ~T180-220); sweeping seed/size/aifill gives the size/density/contest spread. Parse-compat is high (same 3.2.x server family that made our two working saves). Caveat: `timeout -1` ("AI rampant, as fast as possible") is **debug-build only**; on a release build use a small positive `timeout` (220 turns at `timeout 1` ≈ minutes). Effort: ~half-day, mostly one-time. Local only — no third-party cloud.

**Strategy B — download public saves.** Availability for our constraint is **poor**: LongTurn (`longturn/games`, the largest public trove) runs **civ2civ3 and custom rulesets** — disqualified, and its rules aren't what our solvers model. Freeciv distribution files are scenarios (`is_scenario=TRUE`, no gameplay state) targeting civ2civ3. Forum/hall-of-fame/Fandom saves are sparse, often 2.x-era formats that may not match our 3.2 parser, mixed/unknown rulesets, unclear licensing. **There is no curated public archive of `classic` mid/late savegames at scale.** Realistic yield of genuinely-classic, format-compatible, license-clear boards ≈ zero — and most public saves are the exact civ2civ3 silent-mismatch hazard we're gating against.

**Scorecard:** A wins on ruleset guarantee (by construction vs. ~all-fail), effort (half-day one-time vs. high per-board), control/diversity (full vs. none), reproducibility (commit `.serv` vs. opaque blobs), parse-compat (3.2.x family vs. risky 2.x), licensing (our own data vs. ad-hoc), and portfolio value (reproducible benchmark pipeline vs. low). B's only nominal edge — "boards already exist" — is negated because they're the wrong ruleset.

## 3. Recommended path to the 3+3+3 corpus
1. **Land the `rulesetdir` gate** (small parser change, §1) — defense-in-depth for every future harvest; closes the documented hazard. Add `Board.ruleset`+`Board.turn`, hard-error on non-`classic`, assert 3.2.x.
2. **Install Freeciv 3.2.x for Windows**; confirm a throwaway `-r … -e` autogame produces autosaves our parser round-trips.
3. **Parameterized `.serv` generator + driver** sweeping ~3 sizes × {aifill 3,6,9} × 2-3 seeds, `set saveturns 20`, `set endturn ~220`; keep early/mid/late snapshots. A handful of games gives >9 boards; select 3/3/3 spanning size×density×contest (measure contest via tile owner-count + rival unit/city adjacency from existing `Board` fields).
4. **Commit the scripts + a manifest** (`{turn, w×h, #units, #cities, #players, density, ruleset}`), not the blobs (`.sav` are gitignored today) — regenerable from seeds.
5. **Wire into the run matrix** (the still-open "finalize kind roster / re-pre-register" task). Reach kinds want early, less-railed boards → early self-play snapshots are ideal.

## 4. Blockers / open questions for the user
1. **OK to install Freeciv 3.2.x locally?** (one-time, tens of MB, local only) — the only real prerequisite for A.
2. **Commit policy:** tiny reproducible `.serv` scripts + manifest (preferred) vs. also stashing generated `.sav` blobs?
3. **Snapshot thresholds** for early/mid/late — proposed ~T25-40 / ~T90-120 / ~T180-220 on `classic`; adjust? (affects rail density → reach-kind difficulty).
4. **Corpus size:** 3/3/3=9, or extra per band for seed-pooling (noise floor ~12-18%/item)?
5. **Sequencing vs. FOG three-state pending work** — same experiment the fog change re-opens; build corpus in parallel, final run waits on fog + budget top-up.
6. **Parser hardening scope:** just the `rulesetdir` gate now, or also `game_version` range assert + `.sav.gz` gunzip support (public/older saves are often gzip'd; our two are plain)?

## Relevant files
- `civ-core/src/parser.rs` (line 55 loads `[savefile]`; add `rulesetdir`/`turn` reads)
- `civ-core/src/board.rs` (line 91 `Board` struct — add `ruleset`/`turn`)
- `civ-eval/src/rules.rs` (lines 9-16 document the hard-coded `classic` constants)
- `data/saves/myagent_T50.sav` / `testcontroller_T677.sav` (both `rulesetdir="classic"`)
