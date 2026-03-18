#!/bin/sh
set -eu

cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
nix develop --command prettier --check .
nix develop --command sh -c 'cd site && pnpm install --frozen-lockfile && pnpm check'
nix develop --command sh -c 'cd site && pnpm format:check'
nix flake check --no-build
