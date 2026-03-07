#!/bin/sh
set -eu

cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
nix develop --command prettier --check .
nix flake check --no-build
