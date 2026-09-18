// Build the `civ-wasm` crate to wasm32, then base64-inline the .wasm into src/wasm-data.js
// (window.__CIVSPATIAL_WASM__) so the offline viewer can instantiate it from file:// with NO fetch
// and NO network — mirroring how build-tileset.mjs / build-sample.mjs inline their assets.
//
// The wasm exposes civ_eval::run_tool (the eval's REAL tool dispatch) so the viewer's tool playground
// runs tools with byte-for-byte parity to the model, instead of re-implementing any tool in JS.
//
// Usage:  node scripts/build-wasm.mjs
//   Requires: rustup target add wasm32-unknown-unknown  (already present in this repo's toolchain).
//   Optional: wasm-opt (binaryen) on PATH -> an extra -Oz size pass; skipped cleanly when absent.

import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const viewerRoot = join(here, "..");
const repoRoot = join(viewerRoot, "..");

const PROFILE = "wasm-release";
const TARGET = "wasm32-unknown-unknown";

// 1. Compile the wasm crate.
console.log(`building civ-wasm (${TARGET}, ${PROFILE})...`);
const build = spawnSync(
  "cargo",
  ["build", "-p", "civ-wasm", "--profile", PROFILE, "--target", TARGET],
  { cwd: repoRoot, stdio: "inherit", shell: process.platform === "win32" }
);
if (build.status !== 0) {
  console.error("cargo build failed");
  process.exit(build.status ?? 1);
}

let wasmPath = join(repoRoot, "target", TARGET, PROFILE, "civ_wasm.wasm");
if (!existsSync(wasmPath)) {
  console.error(`expected wasm at ${wasmPath} but it is missing`);
  process.exit(1);
}

// 2. Optional wasm-opt -Oz pass (binaryen), when the tool is on PATH.
const hasWasmOpt = spawnSync("wasm-opt", ["--version"], {
  stdio: "ignore",
  shell: process.platform === "win32",
}).status === 0;
if (hasWasmOpt) {
  const tmp = mkdtempSync(join(tmpdir(), "civwasm-"));
  const optPath = join(tmp, "civ_wasm.opt.wasm");
  const opt = spawnSync("wasm-opt", ["-Oz", wasmPath, "-o", optPath], {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (opt.status === 0 && existsSync(optPath)) {
    wasmPath = optPath;
    console.log("applied wasm-opt -Oz");
  } else {
    console.error("wasm-opt failed; using the unoptimized wasm");
  }
  // (tmp dir is cleaned after we read the file, below)
  process.once("exit", () => {
    try {
      rmSync(tmp, { recursive: true, force: true });
    } catch {
      /* best-effort cleanup */
    }
  });
} else {
  console.log("wasm-opt not on PATH — skipping the extra size pass (optional).");
}

// 3. Base64-inline into src/wasm-data.js.
const bytes = readFileSync(wasmPath);
const b64 = bytes.toString("base64");
const dest = join(viewerRoot, "src", "wasm-data.js");
writeFileSync(dest, `window.__CIVSPATIAL_WASM__ = "${b64}";\n`);
console.log(
  `wrote ${dest} — ${bytes.length.toLocaleString()} wasm bytes -> ${b64.length.toLocaleString()} base64 chars`
);
