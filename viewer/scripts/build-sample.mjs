// Merge one or more `civ viewer-export` JSON files into src/sample-data.js, which sets
// window.__CIVSPATIAL_SAMPLE__ so the viewer shows something out-of-the-box (no server, no fetch).
//
// Usage:  node scripts/build-sample.mjs [export1.json export2.json ...]
// Default inputs: samples/static-sample.json + samples/interactive-sample.json (if present).

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const args = process.argv.slice(2);
const inputs = args.length
  ? args
  : [join(root, "samples", "static-sample.json"), join(root, "samples", "interactive-sample.json")]
      .filter(existsSync);

if (!inputs.length) {
  console.error("no input exports found; pass paths or generate samples/ first");
  process.exit(1);
}

const merged = { boards: {}, questions: [], meta: { model: "", sources: [] } };
for (const path of inputs) {
  const d = JSON.parse(readFileSync(path, "utf8"));
  Object.assign(merged.boards, d.boards);
  for (const q of d.questions) merged.questions.push(q);
  if (!merged.meta.model && d.meta?.model) merged.meta.model = d.meta.model;
  merged.meta.sources.push(path);
}
merged.meta.n_questions = merged.questions.length;
merged.meta.n_boards = Object.keys(merged.boards).length;

const out = join(root, "src", "sample-data.js");
writeFileSync(out, "window.__CIVSPATIAL_SAMPLE__ = " + JSON.stringify(merged) + ";\n");
console.log(
  `wrote ${out} — ${merged.questions.length} questions, ${merged.meta.n_boards} board(s), from ${inputs.length} export(s)`
);
