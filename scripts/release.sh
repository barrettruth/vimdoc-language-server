#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release.sh <version> [--dry-run]

Prepares and ships a numbered release from a clean, up-to-date main branch.

Examples:
  just release 0.2.2
  just release 0.2.3 --dry-run
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

run() {
  echo "+ $*"
  "$@"
}

version="${1:-}"
mode="${2:-}"

if [ -z "$version" ] || [ "$version" = "-h" ] || [ "$version" = "--help" ]; then
  usage
  exit 0
fi

dry_run=false
case "$mode" in
  "")
    ;;
  "--dry-run")
    dry_run=true
    ;;
  *)
    die "unknown option: $mode"
    ;;
esac

version="${version#v}"
tag="v${version}"

if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  die "version must look like 1.2.3, got: $version"
fi

start_head="$(git rev-parse HEAD)"

cleanup_dry_run() {
  if [ "$dry_run" = true ]; then
    echo "+ cleanup dry-run changes"
    git reset --hard "$start_head" >/dev/null
    git tag -d "$tag" >/dev/null 2>&1 || true
  fi
}
trap cleanup_dry_run EXIT

[ -f Cargo.toml ] || die "run from the repository root"

branch="$(git branch --show-current)"
[ "$branch" = "main" ] || die "release must run from main, currently on $branch"

[ -z "$(git status --porcelain)" ] || die "working tree must be clean"

run git fetch origin main

local_head="$(git rev-parse HEAD)"
remote_head="$(git rev-parse origin/main)"
[ "$local_head" = "$remote_head" ] || die "main must match origin/main"

if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  die "local tag already exists: $tag"
fi

if git ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1; then
  die "remote tag already exists: $tag"
fi

if command -v tea >/dev/null 2>&1; then
  if ! tea actions secrets list --remote origin --output simple 2>/dev/null \
    | awk '{print $1}' \
    | grep -qx 'CARGO_REGISTRY_TOKEN'; then
    die "Forgejo action secret CARGO_REGISTRY_TOKEN is not configured"
  fi
else
  die "tea is required to verify Forgejo action secrets"
fi

crate_name="$(sed -n 's/^name = "\(.*\)"/\1/p' Cargo.toml | head -1)"
current_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[ -n "$crate_name" ] || die "could not read crate name from Cargo.toml"
[ -n "$current_version" ] || die "could not read current version from Cargo.toml"

if [ "$version" = "$current_version" ]; then
  die "version is already $version"
fi

lowest="$(printf '%s\n%s\n' "$current_version" "$version" | sort -V | head -1)"
if [ "$lowest" != "$current_version" ]; then
  die "target version $version must be greater than current version $current_version"
fi

published_version="$(
  cargo search "$crate_name" --limit 1 2>/dev/null \
    | sed -n "s/^${crate_name} = \"\\([^\"]*\\)\".*/\\1/p"
)"

if [ "$published_version" = "$version" ]; then
  die "$crate_name $version is already published on crates.io"
fi

echo "Preparing $crate_name $tag"

run cargo set-version "$version"
run sed -i '0,/version = "[^"]*";/s//version = "'"$version"'";/' flake.nix
run cargo run --quiet --example generate-man -- man

run git add Cargo.toml Cargo.lock flake.nix man

run just ci
run cargo publish --dry-run --locked --allow-dirty

run git diff --cached --stat

if [ "$dry_run" = true ]; then
  echo "Dry run complete for $tag; no commit, tag, or push was kept."
  exit 0
fi

run git commit -m "chore: release $tag"
run git tag -a "$tag" -m "$tag"
run git push --atomic origin HEAD:main "refs/tags/${tag}"

cat <<EOF

Pushed $tag.
Forgejo will publish crates.io and replace the release from the tag workflow.
EOF
