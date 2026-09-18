# Running the eval through subscriptions (Claude Max + ChatGPT/Codex)

Point the harness at your **Claude Max** and **ChatGPT/Codex** subscriptions instead of a
pay-per-token API, using a local OpenAI-compatible proxy. No Rust changes: `civ run` already speaks
OpenAI-compatible to any `--base-url` (that's how the LM Studio and OpenRouter paths work), so a
subscription backend is just a different URL + key.

**Bridge:** [CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) — one process wraps *both*
subscriptions (via the Claude Code and Codex OAuth flows) and re-exposes them at
`http://localhost:8317/v1/chat/completions`. Guides: <https://help.router-for.me/>.

```
  civ run --base-url http://localhost:8317/v1   ┌─────────────┐  OAuth   ┌──────────────┐
  --model claude-code|gpt-5 --api-key … ───────▶│ CLIProxyAPI │─────────▶│ Claude Max   │
     (identical to the OpenRouter path)          │   :8317     │          │ ChatGPT/Codex│
                                                 └─────────────┘          └──────────────┘
```

> **Current scope: Claude Max only.** OpenAI/Codex isn't set up yet — skip `--codex-login`. To add it
> later: run `--codex-login` and append the model, e.g. `SUB_MODELS="claude-haiku-4-5-20251001 gpt-5-mini"`.

## For agents (start here)

You (an AI agent working in this repo) can *run* experiments on this path but **cannot set it up** —
the proxy holds an OAuth login the user must do interactively in a browser with their own account.

1. **Check the proxy is up before anything else** (agents can't start it if it isn't):
   ```bash
   curl -s -m5 http://localhost:8317/v1/models -H "Authorization: Bearer civspatial-local"
   ```
   Empty/refused → the proxy isn't running. **Ask the user** to run the three commands in *One-time
   setup* below; do not try to `--claude-login` yourself. A JSON model list → you're good.

2. **Run experiments** with the ready runner (it preflights the proxy and prints the model list):
   ```bash
   bash scripts/run-subs-arm.sh                 # cheap Claude-only smoke (1 board, per-kind 1)
   SMOKE=0 PK=4 bash scripts/run-subs-arm.sh    # fuller exploratory arm (see billing rule below)
   ```
   Or a raw call — the only difference from the OpenRouter path is three flags:
   `--base-url http://localhost:8317/v1  --api-key civspatial-local  --model <id-from-/v1/models>`
   (build first: `cargo build -p civ-cli --features remote`).

**Non-negotiable rules:**
- **`--cache on` — caching now WORKS on this path (fixed `c0b8cd6`, 2026-08-24).** Earlier the
  maxops/interactive arms 400'd because the harness placed `cache_control` inside
  `tool_result.content`; native Anthropic requires it on the `tool_result` block itself, and the
  remote encoder now emits it there off-OpenRouter. Verified ~99% cached on warm calls for BOTH raw
  and maxops. Flat-rate billing means caching saves no *dollars*, but it conserves the *weekly cap*
  (~7× fewer tokens on the maxops arms), which is the real constraint here — so **use `--cache on`.**
  (`run-subs-arm.sh` still defaults `CACHE=off` for now; pass `CACHE=on`.)
- **Exploratory, NOT a preregistered arm.** Claude Code injects a hidden system prompt you can't
  control. Never merge subscription results into the OpenRouter clean-run files
  (`results-3arm-*.jsonl`); the runner isolates them as `results-subs-arm-*.jsonl`.
- **Billing = the user's Max subscription usage limits** (5-hour + weekly), NOT pay-per-token. At the
  limit it STOPS (no auto-charge, unless the user turned on the overflow toggle). But a large sweep
  can exhaust the *weekly* cap and lock the user out of their own Claude Code. **Check in with the
  user before any run bigger than the smoke** (their standing preference), and keep runs modest.
- **Cheapest model:** `claude-haiku-4-5-20251001` (verified deterministic at temp 0; `--think off`
  works). `--model claude-code` is an alias that routes to the subscription default.

## One-time setup

1. **Install the proxy (Windows).** Download the `windows-amd64` archive from the
   [releases page](https://github.com/router-for-me/CLIProxyAPI/releases) and extract it — you get
   **`cli-proxy-api.exe`**. (Prefer a GUI? [EasyCLIProxyAPI](https://github.com/router-for-me/EasyCLI)
   is a desktop front-end that does the same OAuth logins.)

   **Where to run it from:** the working directory just needs to be the repo root
   (`C:\Projects\CivSpatial`) so the relative `--config scripts/cliproxy-config.yaml` resolves; the
   `.exe` itself can live anywhere. Simplest: drop `cli-proxy-api.exe` in the repo root. The three
   commands below assume PowerShell at the repo root — adjust `.\cli-proxy-api.exe` to wherever you
   put it (or pass an absolute `--config` path and cwd stops mattering).

2. **Log in to Claude Max** (the ONE step only you can do — it opens a browser OAuth flow against
   *your* account; the token is cached under `~/.cli-proxy-api/`):

   ```powershell
   .\cli-proxy-api.exe --config scripts/cliproxy-config.yaml --claude-login
   ```

3. **Start the proxy** (leave it running in its own terminal):

   ```powershell
   .\cli-proxy-api.exe --config scripts/cliproxy-config.yaml
   ```

   Config lives in [`scripts/cliproxy-config.yaml`](cliproxy-config.yaml): localhost-only, port 8317,
   downstream key `civspatial-local`. That key gates *localhost access to the proxy* — it is **not** a
   subscription secret (the OAuth tokens are). Override it in `.env` with `CLIPROXY_API_KEY=...` if you
   like; the scripts read it.

4. **List the exact model ids** the proxy exposes (routing aliases `claude-code` / `gpt-5` work
   immediately, but for clean result labels use the real ids):

   ```bash
   curl -s http://localhost:8317/v1/models -H "Authorization: Bearer civspatial-local"
   ```

## Run it

```bash
bash scripts/subs-ping.sh          # fidelity smoke-test (DO THIS FIRST — see caveats below)
bash scripts/run-subs-arm.sh       # cheap smoke run: cheapest model per provider, 1 board, per-kind 1
SMOKE=0 PK=8 bash scripts/run-subs-arm.sh   # scale up to a fuller exploratory arm
```

`run-subs-arm.sh` mirrors `run-3arm-2026-08-18.sh` (per-invocation result files, `--resume`, tracing)
but bills against the subscriptions via the proxy. It preflights the proxy and lists the exact model
ids it exposes, then defaults to Anthropic's most recent Haiku + OpenAI's small tier.

Point any real run at the proxy by swapping three flags on the usual `civ run` line:

```
--base-url http://localhost:8317/v1  --api-key civspatial-local  --model <id-from-/v1/models>
```

Everything else (encoders, `--cache on`, `--concurrency`, the JSONL sink, `--resume`, and the full
LLM-I/O tracer under `traces/`) behaves identically to the OpenRouter arms.

## Caveats — why this is EXPLORATORY, not a preregistered arm

The 3-arm prereg (`analysis/run-preregistration-3arm-2026-08-17.md`) pins **temperature 0** and
**`--think off`** for a clean, comparable measurement. Subscription backends break those controls in
ways an API doesn't:

- **Hidden system prompt.** The Codex/Claude Code backends inject their own coding-agent system
  prompt between you and the model. That's a different condition from the raw OpenRouter call — it
  can help or hurt, and you can't see it.
- **`--think off` may be inert or rejected.** These are reasoning models. The harness emits
  `reasoning_effort:"none"` on `--think off`; the backend may ignore it (reasoning stays on) or 400
  the request (→ empty replies, logged as a warn, recorded as blank). `subs-ping.sh` checks this per
  model; if think-off is empty/unstable, that model **cannot match the prereg's think-off arm**. Use
  `THINK=on` for subscription runs (the harness then sends no reasoning field — the safe default).
- **Temperature may be ignored**, so runs may not be deterministic even at 0. `subs-ping.sh` probes
  determinism with repeated identical prompts.
- **No provider/quantization pin.** You lose the `--provider` control the OpenRouter arms rely on.
- **`--cache on` now works (fixed `c0b8cd6`, 2026-08-24).** Previously the tool-loop encodings 400'd
  because the harness placed `cache_control` inside `tool_result.content`; the native Anthropic API
  needs it on the `tool_result` block, which the remote encoder now does off-OpenRouter. Verified
  ~99% cached on warm calls (raw AND maxops). Flat-rate billing means no dollar saving, but caching
  cuts weekly-cap consumption ~7× on the maxops arms — use it.

**Validated 2026-08-20 (Claude Max, `claude-haiku-4-5-20251001`):**
`--think off` returns non-empty, and both the static (`raw`) and interactive (`raw-maxops`) arms run
0-error. **Caching validated 2026-08-24 (`c0b8cd6`):** ~99% cached on warm raw and maxops calls
through the proxy — run with `CACHE=on`. Full I/O + `run.json` persisted under `traces/` as usual.
**NOT deterministic at temp 0** — a full Haiku 3-arm run (2026-08-24) found re-run pairs only ~64–73%
identical (answer+status), so the early "deterministic" read does not hold at scale. Another reason
this path is exploratory-only. **Windows `--resume` caveat:** non-ASCII chars in an `item_id` (e.g.
the late board's fog player "François Mitterrand", the `ç`) break the resume key-match → items re-run
instead of skipped and the raw JSONL double-counts; dedup by `(encoding, item_id)` before
`summarize.py`.
- **ToS / stability.** The Codex path is semi-blessed (OpenAI publicly endorsed the pattern); the
  Claude-via-proxy path routes as Claude Code and Anthropic has blocked some out-of-Claude-Code
  subscription use before. Auth can go stale; re-run the login. Don't build the published numbers on
  a reverse-engineered bridge.

**Bottom line:** great for cheap scouting and for adding frontier OpenAI/Anthropic *subjects* you
don't currently run — but keep the clean, citable arms on OpenRouter unless `subs-ping.sh` shows a
given model is deterministic and honors the think toggle. See the fidelity-merge bar in the project
memory before promoting any subscription result into the main comparison.
