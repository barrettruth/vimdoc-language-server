# Vimdoc Ecosystem

Related tools, parsers, and prior art relevant to vimdoc-language-server.

---

## Parsers / Grammars

### neovim/tree-sitter-vimdoc
[github.com/neovim/tree-sitter-vimdoc](https://github.com/neovim/tree-sitter-vimdoc)

The canonical tree-sitter grammar for vimdoc. Maintained under the Neovim org.
Used by Neovim itself for syntax highlighting and the `vim.lsp.buf` doc
renderer. See `notes/tree-sitter-vimdoc.md` for a full breakdown.

Notable: the grammar was rewritten in PR #16 to remove the hand-written C
scanner (which caused hangs on `filetype.txt`, `usr_24.txt`). Now fully
grammar.js-based.

### dimus/tree-sitter-vim-help
[github.com/dimus/tree-sitter-vim-help](https://github.com/dimus/tree-sitter-vim-help)

An older, alternative tree-sitter grammar for vim help files. Less maintained.
Useful as a reference but `neovim/tree-sitter-vimdoc` is the authoritative one.

---

## Linters / Validators

### vint (Vimjas/vint)
[github.com/Vimjas/vint](https://github.com/Vimjas/vint)

A linter for **Vimscript** (`.vim` files), not vimdoc `.txt` help files. Written
in Python. Configurable via `~/.vintrc.yaml`. Integrated with ALE.

**Not relevant** to vimdoc help file linting. Included here to clarify the
naming confusion — "vint" does not lint vimdoc format.

There is effectively **no dedicated vimdoc linter** in the ecosystem as of
2026. This is a gap vimdoc-language-server can fill via `publishDiagnostics`.

---

## Generators (Doc → Help)

### google/vimdoc
[github.com/google/vimdoc](https://github.com/google/vimdoc)

Generates `.txt` help files from annotated VimL (VimScript). Uses `""` comment
blocks. Handles tag alignment, section structure. Python tool.

Self-described as "a collection of regexes and hacks." Useful reference for
what output a well-formed vimdoc file looks like.

### wincent/docvim
[github.com/wincent/docvim](https://github.com/wincent/docvim)

Haskell-based documentation generator for Vim plugins. Targets both Vim help
and Markdown output. Largely inactive.

---

## Viewers / Renderers

### OXY2DEV/helpview.nvim
[github.com/OXY2DEV/helpview.nvim](https://github.com/OXY2DEV/helpview.nvim)

A Neovim plugin that renders vimdoc with decorations (code blocks, headings,
highlight group names, horizontal rules, inline code, modelines, optionlinks).
No external deps — uses tree-sitter-vimdoc directly. Supports hybrid mode
(edit + render simultaneously) and splitview.

Useful reference for: which nodes are rendered, how heading levels are
distinguished visually, what "decorated" vimdoc should look like.

### vim-jp/vimdoc-en
[github.com/vim-jp/vimdoc-en](https://github.com/vim-jp/vimdoc-en)

HTML rendering of the official Vim help pages. Auto-updated daily from
upstream Vim. Useful as a reference for what correct, well-formed vimdoc looks
like in practice.

---

## Editor Integration

### dense-analysis/ale
[github.com/dense-analysis/ale](https://github.com/dense-analysis/ale)

Asynchronous lint engine for Vim/Neovim. Runs linters on buffer change,
reports via virtual text / sign column. Supports LSP. A vimdoc-language-server
integration would be straightforward here.

### mfussenegger/nvim-lint
[github.com/mfussenegger/nvim-lint](https://github.com/mfussenegger/nvim-lint)

Lightweight async linter plugin for Neovim. Spawns external linters, feeds
output to `vim.diagnostic`. Alternative to ALE for Neovim users.

---

## Specification References

### nanotee/vimdoc-notes
[github.com/nanotee/vimdoc-notes](https://github.com/nanotee/vimdoc-notes)

The most comprehensive community-written vimdoc spec. Covers:
- Official syntax groups with vim regex patterns
- Undocumented "found in the wild" constructs (shell command blocks, tables)
- Critique of the format and candidates for replacement (Markdown, AsciiDoc)

Key insight from this doc: **there is no official spec**. `:help help-writing`
and `:help notation` are incomplete; many syntax groups are convention-only.

### Neovim help-writing docs
[neovim.io/doc/user/helphelp.html](https://neovim.io/doc/user/helphelp.html)

Referenced by tree-sitter-vimdoc as a primary spec source. Covers tag syntax,
link syntax, headings, codeblocks, encoding, modelines.

### Vim syntax/help.vim
`$VIMRUNTIME/syntax/help.vim`

The ground truth for what Vim itself highlights. The regex patterns in this
file define the actual parsing rules better than any prose spec. The
`vimdoc-notes` repo has extracted all the relevant patterns — see
`notes/vimdoc-format.md`.

---

## Neovim's CI Doc Linting Pipeline

### How it works

`make lintdoc` → `scripts/lintdoc.lua` → `gen_help_html.run_validate()`.
Runs on every PR in CI. The validation logic lives in `gen_help_html.lua`
(~1276 lines), which walks the tree-sitter-vimdoc AST using `visit_validate()`.

### What it checks

- Parse errors (tree-sitter ERROR/UNKNOWN nodes)
- Broken taglinks (cross-file resolution via a tagmap built from all help files)
- Bad URLs (basic format validation)
- Misspellings (custom wordlist)
- Has extensive hardcoded exemption tables for known edge cases

### What it does NOT check

- Unclosed delimiters (`` * | ` ``)
- Codeblock without explicit `<` terminator
- Codeblock inside listitem without blank line separator
- `>` preceded by tab instead of space
- Column heading `~` without preceding space
- Missing or malformed modeline
- Separator line length inconsistency
- Heading without preceding separator
- Duplicate tags within a single file (only checks cross-file via tagmap)

### Relevance

This pipeline is the target for goal 6 (upstream Lua contributions). Rules we
develop and validate in the Rust LSP get expressed as Lua additions to
`visit_validate()`. The gap list above defines the concrete contribution
opportunities.

---

## Gaps / Opportunities

The ecosystem lacks:
1. A dedicated vimdoc linter (no equivalent of `markdownlint` for vimdoc).
2. Cross-file tag resolution (vimdoc-language-server can provide this).
3. Rename/refactor support for tags.
4. Completion for `|taglinks|` across the workspace.
5. Hover showing tag definition context.
