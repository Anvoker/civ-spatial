# CivSpatial task runner. Install `just`: https://github.com/casey/just
# List recipes with `just` or `just --list`.

_default:
    @just --list

# Build the whole workspace (offline; no network features).
build:
    cargo build --workspace

# Run all tests.
test:
    cargo test --workspace

# Lint (warnings are errors).
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format check.
fmt:
    cargo fmt --all -- --check

# The harness self-consistency gate: the Oracle model must score 100% — on both boards, and both
# omniscient AND three-state fogged (from a live-civ perspective: T50 player 1, T677 player 0).
verify-oracle:
    cargo run -q -p civ-cli -- verify-oracle --board data/saves/myagent_T50.sav
    cargo run -q -p civ-cli -- verify-oracle --board data/saves/myagent_T50.sav --fog 1
    cargo run -q -p civ-cli -- verify-oracle --board data/saves/testcontroller_T677.sav
    cargo run -q -p civ-cli -- verify-oracle --board data/saves/testcontroller_T677.sav --fog 0

# Dump a parsed board as an ASCII overview (sanity check the parser).
dump board="data/saves/myagent_T50.sav":
    cargo run -q -p civ-cli -- dump-board --board {{board}}

# Generate the question set for a board and print a summary.
questions board="data/saves/myagent_T50.sav" seed="1":
    cargo run -q -p civ-cli -- gen-questions --board {{board}} --seed {{seed}}

# Run the offline oracle eval across the v1 encoder trio; writes results.jsonl.
eval board="data/saves/myagent_T50.sav" seed="1":
    cargo run -q -p civ-cli -- run --board {{board}} --seed {{seed}} \
        --encoding raw --encoding ascii --encoding adjacency \
        --model oracle --out results.jsonl

# Everything CI would check.
check: test lint verify-oracle

# Stand up the local self-hosted Langfuse stack for viewing OTel GenAI traces.
langfuse-up:
    bash observability/langfuse-up.sh

# Tear down the Langfuse stack (append args="--volumes" for a full data wipe).
langfuse-down args="":
    bash observability/langfuse-down.sh {{args}}
