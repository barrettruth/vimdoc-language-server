default:
    @just --list

format: rust-format markdown-format site-format
    @:

lint: rust-lint site-check flake-check
    @:

test:
    cargo test --all

build:
    cargo build --release

rust-format:
    cargo fmt --all -- --check

rust-lint:
    cargo clippy --all-targets -- -D warnings

markdown-format:
    prettier --check .

site-install:
    cd site && pnpm install --frozen-lockfile

site-check: site-install
    cd site && pnpm check

site-format: site-install
    cd site && pnpm format:check

site-build: site-install
    cd site && pnpm build

flake-check:
    nix flake check --no-build

build-target target:
    cargo build --release --target {{target}}

ci: format lint test
    @:
