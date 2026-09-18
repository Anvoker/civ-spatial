# CivSpatial Replay Viewer

A static, blog-embeddable web viewer for the CivSpatial LLM-spatial-reasoning eval. It replays the
questions a model went through — the board, the question's **referents** (highlighted) and
**candidate set** (shaded), the **model's answer vs. the correct answer** with a pass/fail chip, and,
for interactive encodings, a **turn-by-turn stepper** that lights up the tiles the model fetched as it
explored. Fully offline: it makes **no LLM calls** and needs **no server** — the compiled output is a
plain `index.html` + one `dist/viewer.js`.

It is our own HTML5-canvas renderer (Option B). Terrain, cities and units are blitted from the GPL
Freeciv **`trident`** overhead tileset (parsed by our own `.spec` parser into `src/tileset-data.js`);
flat color remains a graceful fallback when a sprite or the tileset bundle is missing. The board pane
**zooms** (mouse wheel, toward cursor) and **pans** (click-drag), with a "Fit" reset; the hover tile
readout works under any zoom/pan.

It also has a board-scoped **Tools playground**: pick any of the eval's 29 model-facing tools, fill
its parameters (click the map to fill a coord pair or a unit id), and see the tool's **exact output
string** run against **whatever board is displayed** — respecting the **Fog** toggle. This runs the
**real Rust dispatch** (`civ_eval::run_tool`) compiled to WebAssembly, so outputs are byte-for-byte
what a model would see; no tool is re-implemented in JS. See **Tool playground** below.

## Layout

```
viewer/
  index.html            # the page (theme-aware CSS, panel layout, board controls)
  src/main.ts           # the whole app (vanilla TS, no framework, no runtime deps)
  src/sample-data.js    # generated: window.__CIVSPATIAL_SAMPLE__ (zero-config demo data)
  src/tileset-data.js   # generated: window.__CIVSPATIAL_TILESET__ (trident sprite index + inlined PNGs)
  dist/viewer.js        # generated: tsc output referenced by index.html
  src/wasm-data.js      # generated: window.__CIVSPATIAL_WASM__ (base64-inlined civ-wasm .wasm)
  dist/viewer.js        # generated: tsc output referenced by index.html
  scripts/build-sample.mjs    # merge export JSON(s) -> src/sample-data.js
  scripts/build-tileset.mjs   # parse trident .spec + inline PNGs -> src/tileset-data.js
  scripts/build-wasm.mjs      # build civ-wasm (wasm32) + base64-inline -> src/wasm-data.js
  assets/tilesets/trident/    # GPL trident assets (.spec + .png) + COPYING + CREDITS.md
  samples/*.json        # example exports (load via the file picker)
  tsconfig.json package.json
```

## Build & run

```bash
cd viewer
npm install          # one dev dependency: typescript
npm run build        # tsc -> dist/viewer.js  (npm run check for typecheck-only)

# Optional regen of the generated JS bundles (already committed):
npm run build:sample     # merge samples/*.json  -> src/sample-data.js
npm run build:tileset    # parse assets/tilesets/trident/*.spec + inline PNGs -> src/tileset-data.js
npm run build:wasm       # build civ-wasm (wasm32) + base64-inline -> src/wasm-data.js
```

Building the tool-playground wasm needs the Rust `wasm32-unknown-unknown` target (`rustup target add
wasm32-unknown-unknown`) and nothing else — **no** `wasm-pack` / `wasm-bindgen-cli`. `civ-wasm` is a
plain dependency-free `cdylib` with a hand-rolled C ABI (`alloc`/`dealloc`/`run_tool`), built with the
isolated `wasm-release` cargo profile; `build:wasm` compiles it, runs an optional `wasm-opt -Oz` pass
when [binaryen](https://github.com/WebAssembly/binaryen) is on `PATH`, and base64-inlines the `.wasm`
into `src/wasm-data.js`. It is **inlined** (not fetched) precisely so the page can `WebAssembly
.instantiate` it from `file://` with no network and no CORS — see *CSP / offline* below.

Board controls: **scroll** to zoom (toward the cursor), **click-drag** to pan, **Fit** to reset the
view, **+ / −** buttons to zoom about center, and **Sprites: on/off** to toggle the trident art vs.
flat color. Hover any tile for its terrain / extras / owner / city / unit readout.

Then open `index.html` in a browser (double-click / `file://` works — it is fully static). The bundled
sample loads automatically. Load a different run three ways:

- **File picker** (top-right button) or **drag-and-drop** a `viewer-export.json` onto the page.
- **`?data=<url>`** query param when the page is hosted (fetches that JSON).
- Regenerate the embedded sample: `npm run build:sample -- path/to/export.json [more.json ...]`.

To watch a model *explore* (the interactive stepper), load `samples/interactive-sample.json` via the
picker and use the **Turn ◀ / ▶** controls (or Shift+Arrow keys).

## Producing an export

The viewer loads the JSON emitted by the additive Rust subcommand (`civ-cli`):

```bash
# from the repo root, against any completed run's trace dir under traces/
cargo run -p civ-cli -- viewer-export \
    --trace-dir traces/<stem>-<runid> \
    --out viewer-export.json \
    --saves-dir data/saves        # optional; defaults to data/saves
```

The two files in `samples/` were produced this way (a static `raw` run and an interactive tool-loop
run). Regenerate the bundled sample from one with `npm run build:sample`.

## JSON contract

One self-contained object per run:

```jsonc
{
  "boards": {
    "<board_ref>": {                       // e.g. "testcontroller_T677.sav#fog(p4)"
      "source": "<board_ref>",
      "width": 84, "height": 56,
      "tiles":  [ { "x", "y", "terrain", "extras": ["River","Road",...], "owner": "<name>"|null } ],
      "cities": [ { "x", "y", "id", "name", "owner", "size", "improvements": ["City Walls",...] } ],
      "units":  [ { "x", "y", "id", "kind", "owner", "veteran", "hp" } ]
    }
  },
  "questions": [
    {
      "item_id", "category", "tier", "encoding", "board_ref",
      "question_text",
      "referents":  [ { "kind": "tile|city|unit", "x", "y", "id"?, "label"? } ],
      "candidates": [ { "kind", "x", "y", "id"?, "label"? } ],   // for choice kinds
      "options":    [ "<option label>", ... ],                   // full choice set, when known
      "radius":     2,                                            // region-count / best-site work radius (optional)
      "model_answer",                                            // the PARSED answer (result.got); "" when scoring couldn't parse it
      "answer_text",                                             // the model's RAW completion text (shown even when model_answer is "")
      "correct_answer", "status",                                // status: correct | wrong | invalid | ...
      "reasoning",                                               // model reasoning, when the trace carried it (optional)
      "latency_ms", "prompt_tokens", "completion_tokens", "cached_tokens",  // summed over the trace's turns
      "n_turns", "n_tool_calls",                                 // cached_tokens is the cache-hit SUBSET of prompt (not additive); no total_tokens
      "tool_call_counts": { "get_tile": 4, "att_eff": 2 },       // tool name -> count over ALL turns (every tool-using encoding)
      "trace_file",                                              // source trace filename — the unique key for the copy-identifier
      "trajectory": [                                            // interactive encodings only
        {
          "tool_calls": [ { "name", "args", "region": {"x0","y0","x1","y1"}|null } ],
          "fetched_regions": [ {"x0","y0","x1","y1"} ],          // tiles this turn lit up on the board
          "text": "<assistant text>",
          "reasoning": "<...>"                                    // only if the trace carried it
        }
      ]
    }
  ],
  "meta": { "trace_dir", "model", "n_questions", "n_boards" }
}
```

`tiles` is the full row-major grid (every in-bounds tile). `referents`/`candidates` are resolved to
board coordinates. Interactive `fetched_regions` are parsed from the model's `scan` / `scan_grid` /
`get_tile` / `region_summary` call arguments. A city's **`improvements`** (City Walls / Coastal Defense
/ Great Wall / Palace / …) are load-bearing for the tool playground's wall/coastal/Great-Wall/Palace
math (`city_defense` / `garrison_defense` / `city_fall_prob`); older exports omit the field and it is
treated as "no improvements". When a board carries an `unfogged` companion, its cities also carry
`improvements`.

## Tool playground

The **Tools** panel (right column, below the Answer panel — it stays put across questions because it
targets the *board*, not the question) lets you invoke any of the eval's model-facing tools yourself:

1. Pick a tool from the grouped dropdown (each option shows its one-line description).
2. Fill parameters. **Click the map** to fill the focused field: a coordinate pair (`x/y`, `x0/y0`,
   `x1/y1`, `cx/cy`, `tx/ty`) from the clicked tile, a **unit id** from a unit on that tile (a chooser
   appears when several are stacked), or a `terrain` / `resource` / `player` field from the tile's own
   facts. `player`-type fields default to the board's `perspective` ("you").
3. **Run** — the current view board is serialized and handed to `wasm.run_tool(boardJson, name, args)`;
   the returned string is shown verbatim (errors in red). A capped scrollback keeps prior calls.

The **Fog** toggle re-targets the tools automatically: with fog **on** they run against the masked
view the model saw (hidden enemy units dropped, unexplored tiles are `Unknown`); with fog **off** they
run against the full ground truth. Parity is exact because the same `civ_eval::run_tool` dispatch that
answers a model's tool call answers yours.

**Parity note.** The export board is *already* the exact view to run against, so the rebuilt board is
treated as fully-visible truth of that view (`Board::from_viewer_json` sets `visibility: None`). The
per-tile Fogged-vs-Visible *annotation* a live fogged run carries is not reconstructable from the
export (only the masked tiles are), so `get_tile` on a fogged export won't print a "(fogged)" marker —
the tiles and hidden-unit drops are faithful, the visibility *labelling* is the one documented gap.

## CSP / offline

The viewer is a strictly self-contained `file://` app: no CDN, no fetch, no network. The wasm is
**base64-inlined** into `src/wasm-data.js` and instantiated via `WebAssembly.instantiate(bytes)` from
those inlined bytes — deliberately, because a `file://` page cannot `fetch()` a sibling `.wasm`
(browsers treat it as a cross-origin/opaque request and block it, and there is no `WebAssembly
.instantiateStreaming` MIME type for `file://`). Inlining sidesteps both. The whole tool path is
dependency-free (pure-Rust `civ-core`/`civ-eval` → `cdylib` → base64), so a strict CSP that forbids
external hosts does not affect it. If `src/wasm-data.js` is absent (wasm not built), the Tools panel
disables its Run button and shows "run `npm run build:wasm`"; the rest of the viewer is unaffected.

## Notes / follow-ups

- **Trident sprites.** Terrain/city/unit art comes from the GPL Freeciv `trident` *overhead* tileset
  under `assets/tilesets/trident/` (see `CREDITS.md` + `COPYING`). `scripts/build-tileset.mjs` parses
  the `.spec` files with our own parser (not freeciv-web's AGPL config) into `src/tileset-data.js`.
  Our terrain names already match Freeciv's (`Glacier` maps to freeciv's `arctic`); water terrains have
  no single-cell sprite and stay flat blue. Flat color (`TERRAIN_COLORS` in `src/main.ts`) is always
  the fallback when a sprite or the bundle is absent — the viewer works with or without the assets.
- **Model answer.** The Answer panel shows the model's parsed answer and its raw completion text side
  by side with the correct answer, the pass/fail chip, the run's model name, and reasoning if present.
  The raw `answer_text` is what surfaces when scoring couldn't parse a value (status `invalid`).
- **Question filters + tool-call analytics.** The left column filters questions by category, encoding,
  and result (correct/wrong/invalid); below the list, a **Tool-call analytics** histogram sums each
  tool's call count over the *currently-filtered* set (most-called first) with the filtered question
  count and grand total, updating live. It covers every tool-using encoding (e.g. raw-maxops's
  `count_terrain`, interactive-maxops's `get_tile`/`scan_grid`), sourced from each question's
  `tool_call_counts`.
- **Per-question metrics.** The Answer panel shows summed latency (ms→s), prompt/completion tokens
  (total = prompt + completion), `cached` tokens (the cache-hit subset of prompt, shown separately —
  never added into a total), turn count, and tool-call count.
- **Full question id + copy.** The center header shows the full `item_id` (the sidebar clips it); a
  Copy button copies a unique identifier — `enc=… | board=… | item=… | trace=<trace_dir>/<file>` — the
  trace filename guaranteeing uniqueness even if the (encoding, board, item) triple ever repeats. Copy
  uses the async Clipboard API with a `file://`-safe `execCommand` fallback.
- The viewer is theme-aware (light/dark, plus a manual toggle) and responsive for screenshots.
