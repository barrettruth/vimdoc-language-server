# Goals

Living document. Ecosystem-level goals for the vimdoc toolchain effort.

---

## 1. Build deep format expertise through the parser

We cannot write a spec, go upstream, or ship correct tooling until we understand
the format at a level nobody else has reached. That means working through the
hard problems firsthand.

Concrete work:

- Run parser + formatter on `$VIMRUNTIME/doc/*.txt` (~160 files, ~2.5M chars)
- Catalog every case our parser mishandles
- Compare our structural analysis against tree-sitter-vimdoc's AST
- Study `gen_help_html.lua` node mapping thoroughly — it contains 4+ years of
  hard-won decisions about h4 detection, list nesting, noise filtering, layout
  modes, tab-alignment-after-conceal, and cross-file tag resolution (documented
  in `notes/vimdoc-to-markdown.md`)
- Study `$VIMRUNTIME/syntax/help.vim` regex patterns as ground truth
- Study `nanotee/vimdoc-notes` for undocumented constructs

This is not optional prep. This IS the spec work. The spec emerges from building
and breaking the tooling, not from reading docs and writing prose.

---

## 2. Canonicalize the vimdoc grammar

Once we have real expertise from goal 1, codify it. The canonical reference
resolves every ambiguity that tree-sitter-vimdoc's issue tracker exposes.

Prescriptive where ambiguity causes real bugs. Descriptive everywhere else.
Lives in this repo (`spec/` or similar) and serves tool authors.

Key decisions (drawn from tree-sitter-vimdoc issues and gen_help_html.lua):

- Codeblock-in-listitem: blank line required before `>`
- Implicit codeblock stop: explicit `<` required in canonical form
- `>` must be preceded by space, not tab
- Column heading: space required before `~`
- h3: UPPERCASE words, defined minimum
- h4: tag-only lines, indentation heuristic (gen_help_html uses >8 spaces)
- Paragraph boundaries in preformatted layout
- Table structure (tab-aligned with `~` header)
- List nesting depth via indentation
- Section hierarchy (h1 > h2 > h3 > h4)

The spec evolves alongside the parser and formatter. If the tooling can't
enforce a rule, the rule doesn't belong in the spec yet.

---

## 3. Upstream contributions (tree-sitter-vimdoc)

Fix bugs where the fix is clean and uncontroversial:

| Issue                        | Fix                      |
| ---------------------------- | ------------------------ |
| #98 h3 single-char word      | Regex tweak              |
| #94 `foo~` column heading    | Remove space requirement |
| #111 keycode at line start   | Token priority           |
| #58 keycode separated by `/` | Don't break on `/`       |

For structural issues (#95, #20, #21, #118, #110, #132, #113): comment on the
issues explaining how the LSP and spec address them. Don't try to force
solutions into a framework that can't express them.

Timing: after we have working software and earned credibility. We need goals 1-2
substantially complete before these PRs carry weight.

---

## 4. Ship vimdoc-language-server

The LSP implements the canonical grammar. Formatter, diagnostics, and navigation
are all grounded in spec decisions from goals 1-2. The current tier 1 code is
scaffolding — the parser needs to get much richer before we can ship correct
diagnostics or a formatter that doesn't break `options.txt`.

### Phase 1: MVP release

- Cargo.toml metadata, LICENSE, README
- Nix flake `packages.default`
- `cargo publish` 0.1.0 + GitHub release tag
- `minimal_init.lua` example
- nvim-lspconfig PR (draft)

### Phase 2: Core LSP features

- Completion (`|` triggers tag completion)
- Hover (tag definition context)
- References (all taglinks to a tag)
- Rename + prepareRename
- Document highlight, document link
- Folding ranges (sections, code blocks, lists)
- Integration test harness

### Phase 3: Spec enforcement + cross-file intelligence

- Diagnostic rules (unclosed delimiters, missing `<` terminator,
  codeblock-in-listitem without blank line, etc.)
- Workspace-wide tag index
- Cross-file definition, references, completion
- Broken-link diagnostics, cross-file duplicate tag detection

### Phase 4: Advanced

- Semantic tokens (keycodes, optionlinks, arguments, URLs)
- Code actions (quick-fix for diagnostics)
- Selection ranges, format-on-type

---

## 5. Lint Neovim's own docs

Run vimdoc-language-server on `runtime/doc/*.txt`. Report real issues.

- Proves the tool works at scale on the most important vimdoc corpus
- Finds real bugs (broken links, unclosed delimiters, ambiguous constructs)
- Builds credibility for the spec and the LSP
- Feeds goals 6 and 6b directly

Prerequisite: diagnostics must be mature and low-false-positive. A noisy linter
that flags 500 things in `options.txt` gets ignored. A precise one that catches
12 real broken links gets adopted.

---

## 6. Contribute validation rules to Neovim's lintdoc (Lua, upstream)

Neovim already has a doc linting pipeline: `make lintdoc` → `lintdoc.lua` →
`gen_help_html.run_validate()`. It runs in CI on every PR. It checks parse
errors (tree-sitter ERROR nodes), broken taglinks, bad URLs, and misspellings.
It is 100% Lua, walks tree-sitter-vimdoc, and lives in-tree.

justinmk's comment (discussion #38173) — "Unlikely to take on a rust dependency
instead of that" — is about Neovim's internal build dependencies, not about
external tools users install separately. Neovim doesn't bundle marksman (F#) or
ltex-ls (Kotlin) either; external LSP servers are installed via nvim-lspconfig,
Mason, or package managers. His concern is adding Rust to Neovim's own build
chain, not what users choose to install.

This means two separate deliverables regardless of language:

- **The LSP binary** (Rust, external, installed by users, speaks LSP protocol)
- **Upstream validation rules** (Lua, lives in Neovim's repo, runs in CI)

Same rules, two implementations. The LSP stays Rust. The validation rules we
contribute upstream are Lua additions to `visit_validate()`.

What their linter currently does NOT check:

- Unclosed delimiters (`` * | ` ``)
- Codeblock without explicit `<` terminator
- Codeblock inside listitem without blank line separator
- `>` preceded by tab instead of space
- Column heading `~` without preceding space
- Missing or malformed modeline
- Separator line length inconsistency
- Heading without preceding separator
- Duplicate tags within a single file (they only check cross-file via tagmap)

The workflow: develop rules in Rust for our LSP (goals 1-4), validate them
against the full runtime corpus (goal 5), then express the proven rules as Lua
additions to `visit_validate()` and PR them to Neovim. Same rules, two
implementations — Rust for the interactive LSP, Lua for upstream CI.

This is distinct from shipping our binary into CI. This is contributing
knowledge upstream in Neovim's own language and framework. The value is not the
code — it's the rules themselves, battle-tested against 160 files.

---

## 7. Upstream contribution (Neovim `:help help-writing`)

Expand the help-writing section to document everything it currently omits:
lists, h3/h4, tables, URLs, modeline, blank line semantics, indentation,
encoding, line width.

This is late in the list for a reason. We earn the right to write this by:

- Building a parser that handles every edge case (goal 1)
- Codifying what we learned (goal 2)
- Demonstrating expertise through upstream PRs (goals 3, 6)
- Shipping working software (goal 4)
- Finding real issues in Neovim's own docs (goal 5)
- Contributing proven validation rules in Lua (goal 6)

"We built the only vimdoc language server, ran it on all 160 runtime docs, and
found these undocumented conventions" carries weight. "We read the issues and
think you should document X" does not.

The `:help help-writing` PR is the upstream-friendly subset of goal 2. The
canonical spec goes deeper — edge cases, interactions, canonical forms too
detailed for a help file.

---

## 8. Ecosystem integrations

| Integration    | When                  | Notes                                      |
| -------------- | --------------------- | ------------------------------------------ |
| nvim-lspconfig | With goal 4 phase 1   | Draft PR, finalize after 0.1.0             |
| Mason.nvim     | With goal 4 phase 1-2 | After crates.io publish                    |
| Nixpkgs        | With goal 4 phase 2   | After flake package output                 |
| AUR            | With goal 4 phase 2   | After binary releases stabilize            |
| Neovim CI      | With goal 6           | Lua validation rules in `visit_validate()` |

---

## 9. Vimdoc-to-Markdown converter (speculative)

Neovim #329 (open 12 years) and #36249 discuss vimdoc→markdown migration.
`gen_help_html.lua` proves vimdoc→output-format works. Research is in
`notes/vimdoc-to-markdown.md`.

This is speculative for real reasons:

- Neovim has discussed this for 12 years without committing to a migration.
  Building a converter for a migration that may never happen is a gamble.
- `gen_help_html.lua` punts on the hardest problem: 90%+ of files use "old
  layout" (preformatted). It wraps them in `white-space: pre` CSS. Markdown has
  no equivalent — paragraph un-wrapping in preformatted vimdoc is unsolved and
  may require heuristics that produce wrong output.
- Massive scope expansion that doesn't serve the LSP's core mission.
- Adoption path is unclear — Neovim may want an in-tree Lua script (like
  gen_help_html.lua), not an external Rust binary.

If Neovim commits to the migration, our parser and spec become the foundation a
correct converter needs. The research stays valuable. But we don't build the
converter on speculation.

Could revisit as `vimdoc-language-server convert` subcommand if the landscape
changes.

---

## Principles

**Build first, specify second.** The spec emerges from the tooling. We don't
know what the rules should be until we've parsed, formatted, and linted 2.5
million characters of real vimdoc.

**Upstream when ready, not before.** PRs backed by working software and real
data carry weight. PRs based on reading issues don't.

**No new syntax.** We describe and enforce existing vimdoc. No extensions.

**Tooling is the spec.** The formatter and diagnostics are how authors interact
with canonical form. The spec document is for tool authors. The LSP changes
behavior.

**Complement, don't compete.** tree-sitter-vimdoc highlights. gen_help_html.lua
generates HTML. helpview.nvim renders. We serve — LSP requests, diagnostics,
formatting, navigation. Different tools, same ecosystem.

**Rust is the right language for the LSP.** Investigated Lua
(`vim.lsp.server()`) and C alternatives. `vim.lsp.server()` is too immature —
single-process (blocks the editor), Neovim-only, new API. C adds unnecessary
complexity for a tool that benefits from Rust's ecosystem (tree-sitter crate,
LSP crates, cargo install). justinmk's objection is about Neovim's internal
deps, not external tools. Every major LSP server is a standalone binary in its
own language. We ship as an external binary via cargo install, Nix, Mason, and
GitHub releases.
