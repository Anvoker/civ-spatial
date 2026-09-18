// Parse the GPL Freeciv `trident` OVERHEAD tileset (.spec files) into OUR OWN sprite index and
// inline the spritesheets as data: URIs, emitting src/tileset-data.js (window.__CIVSPATIAL_TILESET__).
// This is a from-scratch parser of the public `.spec` grammar — it does NOT copy freeciv-web's
// AGPL tileset_config_*.js. Assets + license/credits live under assets/tilesets/trident/.
//
// Usage:  node scripts/build-tileset.mjs
// If the assets are absent (not fetched in this environment) it exits 0 without writing, so the
// viewer simply falls back to flat-color terrain.
//
// The .spec grammar we parse: one or more `[grid_*]` blocks, each with x_top_left / y_top_left /
// dx / dy, then `tiles = { "row","column","tag" ... }`. A data row is `<row>, <col>, "tag"[,"alias"…]`
// and any following lines that begin with a quote are additional alias tags for that SAME cell.
// Each tag resolves to the pixel rect { x: x_top_left+col*dx, y: y_top_left+row*dy, w:dx, h:dy }.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const tdir = join(root, "assets", "tilesets", "trident");

// Which .spec + .png pairs to index, keyed by the sheet name we reference from main.ts.
const SHEETS = [
  { key: "tiles", spec: "tiles.spec", png: "tiles.png" },
  { key: "cities", spec: "cities.spec", png: "cities.png" },
  { key: "units", spec: "units.spec", png: "units.png" },
];

function num(block, name) {
  const m = block.match(new RegExp(`^\\s*${name}\\s*=\\s*(\\d+)`, "m"));
  return m ? parseInt(m[1], 10) : null;
}

// Parse one .spec's grid blocks into { tag: {x,y,w,h} } (pixel rects on that sheet).
function parseSpec(text) {
  const sprites = {};
  // Isolate each grid block: from a `[grid_...]` header to the next `[` section or EOF.
  const gridRe = /\[grid_[^\]]*\]([\s\S]*?)(?=\n\[|$)/g;
  let g;
  while ((g = gridRe.exec(text))) {
    const block = g[1];
    const x0 = num(block, "x_top_left");
    const y0 = num(block, "y_top_left");
    const dx = num(block, "dx");
    const dy = num(block, "dy");
    if (x0 == null || y0 == null || !dx || !dy) continue;
    // The `tiles = { ... }` list within this block.
    const tm = block.match(/tiles\s*=\s*\{([\s\S]*?)\n\}/);
    if (!tm) continue;
    let cur = null; // { row, col }
    for (let raw of tm[1].split("\n")) {
      const line = raw.replace(/;.*$/, "").trim(); // strip trailing comment
      if (!line || line.startsWith('"row"')) continue;
      const head = line.match(/^(\d+)\s*,\s*(\d+)\s*,(.*)$/);
      let tagPart;
      if (head) {
        cur = { row: parseInt(head[1], 10), col: parseInt(head[2], 10) };
        tagPart = head[3];
      } else if (cur && line.startsWith('"')) {
        tagPart = line; // continuation: more alias tags for the current cell
      } else {
        continue;
      }
      const rect = { x: x0 + cur.col * dx, y: y0 + cur.row * dy, w: dx, h: dy };
      for (const t of tagPart.match(/"([^"]+)"/g) || []) {
        sprites[t.slice(1, -1)] = rect;
      }
    }
  }
  return sprites;
}

const haveAll = SHEETS.every((s) => existsSync(join(tdir, s.spec)) && existsSync(join(tdir, s.png)));
if (!haveAll) {
  console.error(
    `trident assets not present under ${tdir} — skipping tileset build (viewer falls back to flat color).`
  );
  process.exit(0);
}

const out = { sheets: {}, sprites: {} };
for (const s of SHEETS) {
  const specText = readFileSync(join(tdir, s.spec), "utf8");
  const rects = parseSpec(specText);
  for (const [tag, rect] of Object.entries(rects)) out.sprites[tag] = { sheet: s.key, ...rect };
  const b64 = readFileSync(join(tdir, s.png)).toString("base64");
  out.sheets[s.key] = `data:image/png;base64,${b64}`;
}

const dest = join(root, "src", "tileset-data.js");
writeFileSync(dest, "window.__CIVSPATIAL_TILESET__ = " + JSON.stringify(out) + ";\n");
console.log(
  `wrote ${dest} — ${Object.keys(out.sprites).length} sprites across ${Object.keys(out.sheets).length} sheet(s)`
);
