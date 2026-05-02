#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release.sh <version> [--dry-run|--tag]

Prepares a numbered release pull request from a clean, up-to-date main branch.

After the release PR is merged, run the script again with --tag to tag the
merged main commit and trigger the Forgejo release workflow.

Examples:
  just release 0.2.2
  just release 0.2.3 --dry-run
  just release 0.2.3 --tag
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
tag_only=false
case "$mode" in
  "")
    ;;
  "--dry-run")
    dry_run=true
    ;;
  "--tag")
    tag_only=true
    ;;
  *)
    die "unknown option: $mode"
    ;;
esac

version="${version#v}"
tag="v${version}"
release_branch="release/${tag}"

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

crate_name="$(sed -n 's/^name = "\(.*\)"/\1/p' Cargo.toml | head -1)"
current_version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[ -n "$crate_name" ] || die "could not read crate name from Cargo.toml"
[ -n "$current_version" ] || die "could not read current version from Cargo.toml"

if [ "$tag_only" = true ]; then
  [ "$version" = "$current_version" ] ||
    die "Cargo.toml is $current_version; merge the $tag release PR before tagging"

  remote_tag_target="$(git ls-remote --tags origin "refs/tags/${tag}^{}" | awk '{print $1}')"
  remote_tag_object="$(git ls-remote --tags origin "refs/tags/${tag}" | awk '{print $1}')"
  if [ -n "$remote_tag_object" ]; then
    if [ "$remote_tag_target" = "$local_head" ] ||
      { [ -z "$remote_tag_target" ] && [ "$remote_tag_object" = "$local_head" ]; }; then
      echo "$tag already exists on origin and points at HEAD."
      exit 0
    fi
    die "remote tag already exists and does not point at HEAD: $tag"
  fi

  if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    local_tag_target="$(git rev-parse "${tag}^{}")"
    [ "$local_tag_target" = "$local_head" ] ||
      die "local tag $tag points at $local_tag_target, not HEAD $local_head"
  else
    run git tag -a "$tag" -m "$tag"
  fi

  run git push origin "refs/tags/${tag}"

  cat <<EOF

Pushed $tag.
Forgejo will publish crates.io and replace the release from the tag workflow.
EOF
  exit 0
fi

if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
  die "local tag already exists: $tag"
fi

if git ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1; then
  die "remote tag already exists: $tag"
fi

if git show-ref --verify --quiet "refs/heads/${release_branch}"; then
  die "local release branch already exists: $release_branch"
fi

if git ls-remote --exit-code --heads origin "$release_branch" >/dev/null 2>&1; then
  die "remote release branch already exists: $release_branch"
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

if [ "$dry_run" = false ]; then
  run git switch -c "$release_branch"
fi

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
run git push -u origin "$release_branch"

pr_body="$(cat <<EOF
## Problem

The next patch release needs the crate, Nix package metadata, and generated manpages bumped from $current_version to $version.

## Solution

Run the release preparation script for $version, including repo CI and \`cargo publish --dry-run --locked --allow-dirty\`, and route the release commit through Forgejo branch protection.
EOF
)"

run tea pulls create \
  --remote origin \
  --head "$release_branch" \
  --base main \
  --title "chore: release $tag" \
  --description "$pr_body"

cat <<EOF

Opened a release PR for $tag.
After that PR is merged, tag the merged main commit with:

  just release $version --tag
EOF
