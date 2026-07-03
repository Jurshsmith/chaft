#!/usr/bin/env sh
set -eu

cargo_args="$*"

cargo fmt --all --check
cargo check --workspace --all-targets $cargo_args
cargo clippy --workspace --all-targets $cargo_args -- -D warnings
cargo test --workspace --all-targets $cargo_args
