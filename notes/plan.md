# vimdoc-language-server — Project Plan

---

## The Situation

Vimdoc is a 30-year-old format with:

- No spec (`:help help-writing` is 20 lines of prose)
- No linter
- No formatter
- No language server
- No cross-file validation
- A tree-sitter grammar that can't express the format's structural semantics and
  has 14 open issues, some 3+ years old

Every tool in the ecosystem re-discovers the same ambiguities independently.
Neovim's own docs have rendering bugs that trace back to format ambiguity, not
parser bugs. tree-sitter-vimdoc's issue tracker is a symptom list for a format
that was never specified.

---

## The Opportunity

We are not building "just an LSP." We are building the **first complete vimdoc
toolchain** — and in doing so, we define what well-formed vimdoc looks like.

This is the same pattern as:

- `prettier` for JavaScript (formatter became the de facto style spec)
- `rustfmt` for Rust (formatter + spec co-evolved)
- `markdownlint` + CommonMark (linter enforces spec compliance)

The difference: those ecosystems already had specs. We write the spec AND the
tooling.

---

## Workstreams

### 1. The Vimdoc Spec (`vimdoc-spec`)

A standalone document (possibly its own repo) that prescriptively defines the
vimdoc format. Not a new format — a **rigorous description of the existing
one**, resolving every ambiguity that tree-sitter-vimdoc's issue tracker
exposes.

Sections:

- **Document structure**: header, modeline, section hierarchy (h1–h4)
- **Block-level elements**: paragraphs, code blocks, lists (flat + nested),
  tables, separator lines
- **Inline elements**: tags, taglinks, optionlinks, codespans, keycodes,
  arguments, URLs
- **Whitespace semantics**: indentation, blank lines, line width
- **Encoding**: UTF-8 only (no latin1 in 2026)

Each element gets:

- Syntax (regex or ABNF)
- Examples (valid + invalid)
- Rationale (why this rule, what ambiguity it resolves)
- tree-sitter-vimdoc issue reference (where applicable)

This spec is the **foundation**. The LSP enforces it. The formatter produces it.
Upstream tools can adopt it.

**Key decisions the spec must make:**

| Ambiguity                                 | Spec resolution                                                            | Resolves                   |
| ----------------------------------------- | -------------------------------------------------------------------------- | -------------------------- |
| Codeblock inside listitem                 | Blank line required before `>`                                             | ts-vimdoc #118, #163       |
| Codeblock terminator `<` vs listitem `- ` | `<` at col 1 always terminates; listitem never starts at col 1 after `<`   | ts-vimdoc #146             |
| Implicit codeblock stop (col-1 text)      | Explicit `<` required in canonical form                                    | eliminates a class of bugs |
| `>` preceded by tab vs space              | Space required (per `:help help-writing`)                                  | ts-vimdoc known issue      |
| `foo~` vs `foo ~`                         | Space required before `~`                                                  | ts-vimdoc #94              |
| Single-char word in h3 (`A FOO`)          | h3 requires 2+ words, each 2+ chars; single-char words allowed mid-heading | ts-vimdoc #98              |
| Tag-only line (pseudo h4)                 | Recognized as h4 when line contains only whitespace + `*tag*`              | ts-vimdoc #110             |
| Unclosed delimiter                        | Invalid; diagnostic required                                               | ts-vimdoc #113             |
| Tab-aligned columns with `~` header       | Defined as table construct                                                 | ts-vimdoc #132             |
| Nested list depth                         | Inferred from indentation; 2-space or tab increments                       | ts-vimdoc #21              |
| Section nesting                           | h1 > h2 > h3 > h4; no skipping levels                                      | ts-vimdoc #95              |

### 2. The Language Server (`vimdoc-language-server`)

This repo. Implements the spec via LSP protocol.

#### Tier 1 — Done

- `textDocument/didOpen`, `didChange`, `didClose`
- `textDocument/publishDiagnostics` (duplicate tags)
- `textDocument/formatting` (separator, prose reflow, heading alignment)
- `textDocument/documentSymbol` (`*tag*` definitions)
- `textDocument/definition` (`|link|` to `*tag*` in same file)

#### Tier 2 — Core LSP Features

- `textDocument/completion` — `|` triggers tag completion from all known tags
- `textDocument/hover` — show tag definition context (surrounding lines)
- `textDocument/references` — find all `|taglinks|` to a `*tag*`
- `textDocument/rename` + `prepareRename` — rename a tag and all its references
- `textDocument/documentHighlight` — highlight matching tag/taglink under cursor
- `textDocument/documentLink` — make `|taglinks|` clickable
- `textDocument/foldingRange` — fold sections (h1/h2/h3), code blocks, lists

#### Tier 3 — Spec Enforcement (Diagnostics)

- Unclosed delimiters (`*`, `|`, `` ` ``)
- Codeblock without explicit `<` terminator
- Codeblock inside listitem without blank line separator
- `>` preceded by tab instead of space
- `~` column heading without preceding space
- Tag not right-aligned to line width
- Missing modeline
- Missing blank line before modeline
- Non-UTF-8 encoding
- Separator line not exactly `line_width` chars
- Section heading without preceding separator
- Broken tag reference (`|link|` to nonexistent `*tag*`)
- h3 heading that doesn't meet minimum requirements

#### Tier 4 — Cross-File Intelligence

- Workspace-wide tag index (scan all `doc/*.txt` files)
- Cross-file `definition` (jump to tag in another file)
- Cross-file `references` (find all files referencing a tag)
- Cross-file `completion` (complete tags from entire workspace)
- Cross-file broken-link diagnostics
- Duplicate tag across files (not just within file)
- Vim runtime tag awareness (know that `|:set|` is valid without the file open)

#### Tier 5 — Advanced Features

- `textDocument/semanticTokens` — keycodes, optionlinks, arguments, URLs
- `textDocument/codeAction` — quick fixes for diagnostics (add `<`, add blank
  line, right-align tag)
- `textDocument/selectionRange` — structural selection (select section, block,
  listitem)
- Incremental parsing (re-parse only changed regions)
- Format-on-type (auto-indent in code blocks, auto-align tags)

### 3. Upstream Contributions

Some tree-sitter-vimdoc issues are genuine parser bugs we should fix upstream,
not work around. Others are structural limitations we address differently.

**Contribute fixes:**

- #98 h3 single-char word — trivial regex fix: `[-A-Z0-9.()_]+` →
  `[-A-Z0-9.()_]*`
- #94 `foo~` column heading — remove space requirement from grammar
- #111 keycode at line start — token priority fix
- #58 `CTRL-P/CTRL-N` — keycode followed by `/` should not break recognition

**Contribute tests / CI:**

- #19 run `gen_help_html.lua` — we could help set this up

**Don't try to fix in tree-sitter (address in spec + LSP instead):**

- #95 structured AST — tree-sitter can't nest sections; our `documentSymbol`
  does
- #20 nested blocks — tree-sitter can't track indentation context
- #21 nested lists — same; we infer depth, tree-sitter can't
- #118/#163 codeblock + listitem — fundamental grammar conflict; spec resolves
  by requiring blank line separator
- #110 h4 — needs semantic analysis tree-sitter can't do
- #132 tables — needs tab-stop awareness tree-sitter can't do
- #113 unclosed backtick — error recovery, not grammar

### 4. Ecosystem Integration

#### nvim-lspconfig PR

- Minimal config entry for `vimdoc-language-server`
- `minimal_init.lua` example in our repo
- `filetypes = { "help" }`
- `root_dir` = find `doc/` ancestor or workspace root

#### Packaging

- `crates.io` — `cargo install vimdoc-language-server`
- Nix flake `packages.default` output
- GitHub release binaries (already have CI for this)
- AUR package (eventually)
- Mason.nvim registry entry (eventually)

#### Spec Publication

- Standalone repo or `spec/` directory in this repo
- Reference from README
- Propose to Neovim as supplementary documentation
- Link from tree-sitter-vimdoc README (they already reference
  `nanotee/vimdoc-notes`)

---

## Sequencing

### Phase 1: Ship the MVP

- Cargo.toml metadata + LICENSE + README
- Nix package output
- `cargo publish` 0.1.0
- nvim-lspconfig PR (draft)
- `minimal_init.lua`

### Phase 2: Core LSP Features (Tier 2)

- Completion, hover, references, rename
- Integration test harness (drives Phase 2 confidence)
- These make the LSP genuinely useful day-to-day

### Phase 3: Spec + Diagnostics (Tiers 3–4)

- Write the spec document
- Implement spec-enforcement diagnostics
- Cross-file tag index + broken-link detection
- Upstream PRs to tree-sitter-vimdoc for fixable issues

### Phase 4: Ecosystem (Tier 5 + integrations)

- Semantic tokens, code actions, selection ranges
- Mason.nvim, AUR
- Propose spec to Neovim project

---

## Non-Goals

- **Replacing tree-sitter-vimdoc.** It's the syntax highlighter; we're the
  language server. Different tools, complementary roles.
- **Inventing new syntax.** The spec describes existing vimdoc. No extensions.
- **Supporting VimScript.** vint exists for that. We are vimdoc-only.
- **Competing with helpview.nvim.** That's a renderer/viewer. We provide the
  data model it could consume.
