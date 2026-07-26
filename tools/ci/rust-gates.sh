#!/usr/bin/env sh
set -eu

cargo_args="$*"

cargo fmt --all --check
cargo check --workspace --all-targets $cargo_args
cargo clippy --workspace --all-targets $cargo_args -- -D warnings
cargo bench -p chaft-benchmarks --bench hot_paths --no-run $cargo_args
cargo test --workspace --all-targets --exclude chaft-benchmarks $cargo_args
