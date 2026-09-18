# Multi-board corpus — self-play generation tooling

Generate the CivSpatial robustness corpus (**≥3 early / ≥3 mid / ≥3 late boards**,
varied in size / density / contest, **all `classic` ruleset**) by headless Freeciv
AI-vs-AI self-play. Because the maps and games are **seeded**, the corpus is
reproducible from the tiny `.serv` scripts — so we commit the scripts + `sweep.json`,
not the multi-MB `.sav` blobs (which are gitignored).

## Why self-play (not community saves)

Our combat/valuation constants are hard-coded from the `classic` ruleset. A
differently-ruled save **silently mismatches** the solvers (see the ruleset-mismatch
hazard in `CONTINUITY-2026-08-04.md`). Self-play with `rulesetdir classic` makes the
ruleset **guaranteed by construction**, and the manifest step re-verifies it per board.

## The three steps

### 1. Generate the `.serv` scripts (no Freeciv needed)

```sh
python scripts/gen-corpus-scripts.py
# -> data/corpus/scripts/*.serv  +  data/corpus/scripts/sweep.json
```

Default sweep = **sizes {small 64×40, medium 80×50, large 96×60} × aifill {3,6,9} ×
seeds {42,1337}** = 18 autogames. With `saveturns 20` and `endturn 220`, each game
autosaves a snapshot every 20 turns → up to 11 boards/game, **3 early / 4 mid / 4 late
per game**. Overridable:

```sh
python scripts/gen-corpus-scripts.py --sizes small,large --aifill 3,9 --seeds 42 \
    --endturn 160 --saveturns 20 --timeout 3 --out-dir data/corpus/scripts
```

Each script sets: `rulesetdir classic`, deterministic `mapsize XYSIZE`+`xsize`/`ysize`,
`mapseed`/`gameseed`, `aifill N`, **`minplayers 0`** (so the all-AI game starts with zero
humans), `saveturns`/`endturn`, `autosaves TURN|GAMEOVER`, `victories ""` (disable
spacerace/domination early-exit), a **positive** `timeout`, `compresstype "PLAIN"`
(plaintext saves our parser reads directly), a `savename` that encodes the sweep cell,
and `start`.

### 2. Run the autogames headless (needs Freeciv)

```sh
bash scripts/run-corpus.sh                 # runs every data/corpus/scripts/*.serv
FREECIV_SERVER="/c/Program Files/Freeciv/freeciv-server.exe" bash scripts/run-corpus.sh
ONLY=corpus_small_a3_s42 bash scripts/run-corpus.sh    # just one cell
```

Per script it runs `freeciv-server -e --Announce none -p <port> -s <savedir> -r <script>`
(`-e` = exit on endturn; no `-q`/quitidle — an all-AI game has zero client connections, so
quitidle would kill it mid-run). Autosaves land in `data/corpus/saves/` (gitignored), named
`<savename>-T<turn>-Y-<year>-auto.sav`. A full sweep to endturn 220 is **many minutes** —
launch in the background. The driver auto-locates the server on PATH or under the versioned
`C:\Program Files\Freeciv*\` install dir (override with `FREECIV_SERVER`), and points
`FREECIV_DATA_PATH` at the binary's sibling `data/` so it finds the rulesets (override with
`FREECIV_DATA_PATH`).

### 3. Build the manifest (reuses OUR parser)

```sh
python scripts/build-manifest.py --saves-dir data/corpus/saves
# -> data/corpus/manifest.json  +  data/corpus/manifest.md
```

For every save it records `{turn, w, h, n_units, n_cities, n_players, density,
ruleset, band, seed, aifill, size_name}`. Board dims / unit / city / player counts
come from `civ dump-board` (**our Rust decoder**, so the manifest reflects exactly
what the eval sees); `turn` + `ruleset` are read from the save text; `seed`/`aifill`/
`size` are joined from `sweep.json` by savename prefix. It **flags any board whose
ruleset ≠ `classic`** (exclude those) and any save our parser chokes on, and tallies
the early/mid/late yield against the ≥3/≥3/≥3 target.

Bands: **early** turn ≤ 60, **mid** 61–140, **late** > 140.

### 4. Fog perspective + yield-driven board selection

The experiment is run **fogged**, so each board needs a **fog perspective player**, and
each question kind only produces instances on boards whose state actually *supports* it
(the generators drop non-decisive draws — see the board-support reality in
`CONTINUITY-2026-08-05.md`). Two scripts turn "216 raw saves" into a defensible 15-board
corpus, both keyed on the fogged condition the run will use.

**`corpus-yield.py`** — per board, picks the fog perspective and measures per-kind yield:

```sh
# single board / a chosen list / the whole pool
python scripts/corpus-yield.py --board data/corpus/saves/foo.sav
python scripts/corpus-yield.py --boards-file scripts/some-list.txt --out data/corpus/yield
python scripts/corpus-yield.py --saves-dir data/corpus/saves --out data/corpus/yield-fullpool
# -> <out>.md (perspective + kind×board matrix) and, for a sweep, <out>.json
#    (per-board records + the global per-kind SUPPORT CEILING = #boards supporting each kind)
```

- **Fog perspective heuristic = the largest sustained civ.** Among players that are (a)
  not barbarian/pirate/animal (by nation) and (b) alive (≥1 city **and** ≥1 unit), pick
  the one maximizing `(n_cities, n_units, -id)`. Cities are the durable footprint; the max
  is substantial by construction (excludes "tiny"); and since these games kept 5–13 civs to
  T221 there is no runaway hegemon, so the pick is a *major power with meaningful fog*, not
  an omniscient one. Winners with `< 3` cities are flagged, not auto-dropped. Per-player
  counts + leader→id map come from `civ dump-board`'s header + DETAIL `owner` lines (player
  id == the order of the `players=[…]` header; verified id 0 = first tuple).
- **Yield** runs `gen-questions --board B --fog <id> --per-kind 12` — the exact fogged
  condition (board masked to that player + player-relative kinds pinned to it, §9 coherence)
  — and tallies the `[kind] kept N` lines into a matrix.

**`select-corpus.py`** — greedily picks N boards from the sweep JSON to **maximize per-kind
support**, weighting rare (frontier) kinds highest, instead of guessing by band/size:

```sh
python scripts/select-corpus.py --yield-json data/corpus/yield-fullpool.json --n 15 \
    --out data/corpus/selection-optimized
# -> selection-optimized.txt (savename list for --boards-file) + .md (support report)
```

Objective `Σ_k W_k·min(support_k, CAP)` with `W_k = 1/ceiling_k` (rarer kinds dominate) and
`CAP = 8` (diminishing returns). **Hard** one-board-per-game (snapshots of the same game are
correlated → 15 boards = 15 distinct games). **Soft** band floor `≥3 early / mid / late` so
the sparse-early perception regime survives even though the objective ignores bands.

**Why yield-driven, not band-picked (the finding).** Support for the frontier kinds is
board-*idiosyncratic*, not a function of band/size/density — so hand-picking by band left
`settle-site` at **4/15** and `forward-posting` at **5/15**. But those kinds are supported on
**60** and **74** of 216 boards respectively (they were *missed*, not rare). Optimizing
selection lifts every discriminating kind to **10–15/15** (settle-site 4→10, forward-posting
5→12) while holding 15 distinct games and a balanced 5/5/5 size mix. The committed corpus is
`data/corpus/selection-optimized.txt`. **The `yield-*` / `manifest*` artifacts are gitignored
(regenerable); the selection list is committed as the pre-registered corpus.**

## Reproducing the corpus from scratch

`gen-corpus-scripts.py` → `run-corpus.sh` → `build-manifest.py` → `corpus-yield.py`
(`--saves-dir` sweep) → `select-corpus.py`. Same seeds + same Freeciv version ⇒ byte-identical
saves ⇒ deterministic manifest, yield, and selection. Commit the scripts + `sweep.json` +
`selection-optimized.txt`; regenerate the saves/manifest/yield anytime.

## Server commands — VERIFIED against Freeciv 3.2.5

All commands below were confirmed live against the installed **freeciv-server 3.2.5**
(a small `aifill 3`, endturn-40 smoke autogame ran clean and our parser round-tripped
all three fresh saves with `ruleset=classic`). Two corrections came out of that smoke run:

- **`set compress 0` is INVALID** (range is 1..9) — removed. `compresstype "PLAIN"`
  alone already means "No compression" (verified), so saves come out plaintext `.sav`.
- **`set minplayers 0` is REQUIRED** — otherwise `start` refuses headless with
  *"Not enough human players ('minplayers' server setting has value 1); game will not start."*

Confirmed-good on 3.2.5: `rulesetdir classic`, `set mapsize XYSIZE`+`xsize`/`ysize`,
`set mapseed`/`gameseed`, `set aifill`, `set minplayers 0`, `set saveturns`/`endturn`,
`set autosaves "TURN|GAMEOVER"`, `set victories ""`, `set timeout 3`,
`set compresstype "PLAIN"`, `set savename`. If you run a *different* Freeciv version,
re-verify in-server (`help set`, `show`, `freeciv-server --help`).

The manifest builder also decompresses `.sav.gz` / `.sav.xz` as a fallback if a build
ignores PLAIN (`.zst` is warned/skipped — install `zstd` or force PLAIN).

## Known caveats

- **Early game-over.** With more AIs a game can end (last civ standing) before
  endturn, losing that config's late snapshots. `victories ""` removes spacerace/
  domination exits but not elimination; the sweep's breadth still yields ≥3 late
  boards across configs. The manifest tells you what you actually got.
- **Autosave filename format** varies across Freeciv versions; we don't depend on it
  — saves are joined to their cell by `savename` **prefix**, and `turn` is read from
  the save's own `turn=` field.
