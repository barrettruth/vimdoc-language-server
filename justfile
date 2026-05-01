default:
    @just --list

format: rust-format site-format
    @:

lint: rust-lint site-check flake-check
    cargo run --quiet --example generate-man -- man
    git diff --exit-code -- man
    test -z "$(git ls-files --others --exclude-standard -- man)" || (git ls-files --others --exclude-standard -- man && exit 1)

test:
    cargo test --all

build:
    cargo build --release

rust-format:
    cargo fmt --all -- --check

rust-lint:
    cargo clippy --all-targets -- -D warnings

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

release version *args:
    nix develop .#ci --command ./scripts/release.sh {{version}} {{args}}
