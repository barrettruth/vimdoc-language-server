# vimfmt roadmap

## Phase 1 — Parser

Hand-rolled line-scanner parser in pure Rust. No external dependencies.

- [ ] Define AST node types (headings, blocks, lines, codeblocks, list items, inline atoms)
- [ ] Line-level parser: classify each line by its structural role
- [ ] Inline parser: scan `*tag*`, `|link|`, `'option'`, `` `codespan` ``, `{arg}`, urls
- [ ] Codeblock detection (`>` / `<` delimiters, verbatim passthrough)
- [ ] Blank-line and section-separator handling
- [ ] Parser test suite with fixture inputs

## Phase 2 — Formatter core

Width-aware printer that consumes the AST and emits formatted vimdoc.

- [ ] Prose line reflowing (word-wrap at configurable width, default 78)
- [ ] Tag right-alignment (column arithmetic on heading lines)
- [ ] Section separator generation (fixed-width `=` / `-` lines)
- [ ] Codeblock passthrough (no reformatting of content)
- [ ] List item formatting
- [ ] Column heading formatting (` ~` lines)
- [ ] Blank-line preservation
- [ ] Idempotency guarantee: `format(format(x)) == format(x)`

## Phase 3 — CLI

- [ ] File arguments: `vimfmt <file>...` (format in place)
- [ ] Stdin/stdout: `vimfmt -` reads stdin, writes stdout
- [ ] `--check` mode (exit 2 if files would change)
- [ ] `--diff` mode (print unified diff, don't write)
- [ ] `--stdin-filepath <path>` for editor integration
- [ ] `--line-width N` override
- [ ] `--color=auto|always|never`, `NO_COLOR` / `FORCE_COLOR` support
- [ ] `--quiet` flag
- [ ] `--version`
- [ ] Exit codes: 0 = success, 1 = error, 2 = check failed

## Phase 4 — Config

- [ ] `.vimfmt.toml` config file
- [ ] Config discovery: walk parent dirs, fall back to `~/.config/vimfmt/config.toml`
- [ ] `--config <path>` override
- [ ] `--print-config` for debugging
- [ ] Options: `line_width`, `indent_style`

## Phase 5 — Testing and stability

- [ ] Fixture-based test suite (`tests/fixtures/<name>.input.txt` / `<name>.output.txt`)
- [ ] Idempotency CI check (format twice, diff)
- [ ] Real-world corpus testing (format neovim's runtime help files, diff)
- [ ] Edge case coverage: empty files, modelines, urls, nested inline atoms

## Phase 6 — Distribution

- [ ] `cargo-dist` setup (GitHub Actions release workflow)
- [ ] Pre-built binaries: linux x86_64/aarch64, macos x86_64/aarch64, windows x86_64
- [ ] Publish to crates.io
- [ ] Nix flake with overlay
- [ ] Upstream to nixpkgs
- [ ] Homebrew tap (via cargo-dist)

## Phase 7 — Editor integration

- [ ] PR to conform.nvim (formatter entry)
- [ ] PR to none-ls (formatter source)
- [ ] PR to guard (file watcher integration)

## Phase 8 — Post-1.0

- [ ] Versioning stability policy (patch = no output changes, minor = output may change)
- [ ] Style edition concept if formatting rules evolve
- [ ] Range formatting (for LSP-style partial formatting)
- [ ] CHANGELOG discipline
