# tree-sitter-vimdoc Open Issues — Analysis

Source:
[neovim/tree-sitter-vimdoc issues](https://github.com/neovim/tree-sitter-vimdoc/issues)
(14 open as of 2026-03-07)

---

## Issue Classification

### Structural Limitations (the hard problems)

These are fundamental constraints of tree-sitter's single-pass, context-free
parsing model. They cannot be fixed without major grammar redesign — and some
may be impossible within tree-sitter's framework.

| #   | Title                                  | Core problem                                                     |
| --- | -------------------------------------- | ---------------------------------------------------------------- |
| 95  | More structured AST                    | Sections aren't nested under headings                            |
| 20  | Nested blocks                          | Indented blocks aren't grouped structurally                      |
| 21  | Nested list items                      | List nesting is flat; consumers must infer depth from whitespace |
| 118 | Codeblock adjacent to list item        | Grammar assumes codeblock is not contiguous to listitem          |
| 163 | Lists with codeblocks indented wrong   | Consequence of #118 — codeblock breaks listitem context          |
| 110 | h4: pseudo-heading (right-aligned tag) | Lines with only right-aligned tags have no heading semantics     |
| 132 | Support tables                         | Tab-aligned columns with `~` header have no table node           |

### Parsing Bugs (edge cases in tokenization)

| #   | Title                                         | Core problem                                                   |
| --- | --------------------------------------------- | -------------------------------------------------------------- |
| 113 | Unclosed backtick starts codespan             | `` `< `` parsed as opening a codespan that never closes        |
| 94  | column_heading: tilde without preceding space | `foo~` not recognized, only `foo ~`                            |
| 98  | h3 uppercase_name: single-char word           | `"A AA"` not matched by `[A-Z0-9.()][-A-Z0-9.()_]+`            |
| 111 | Keycode at beginning of line not recognized   | `CTRL-W` as first token parsed as `(word)` not `(keycode)`     |
| 58  | Multiple keycodes without separator           | `CTRL-P/CTRL-N` not parsed as two keycodes                     |
| 146 | `<` not concealed when followed by `-`        | `< - text` parsed as listitem prefix, not codeblock terminator |

### Infra / Testing

| #   | Title                                                        |
| --- | ------------------------------------------------------------ |
| 19  | CI: run `gen_help_html.lua` in PRs                           |
| 1   | More syntax features (mostly done, `[optional]` args remain) |

---

## Why These Matter to Us

### tree-sitter is structurally constrained; an LSP is not

tree-sitter grammars are:

- Single-pass, left-to-right, no backtracking
- Context-free (GLR, but no arbitrary state)
- Incremental (must re-parse subtrees without full context)
- Query-driven (consumers use `(node_type)` patterns, not code)

An LSP server has none of these constraints. We can:

- **Multi-pass parse**: first pass identifies structure (separators, blank
  lines, indentation), second pass classifies content.
- **Carry arbitrary state**: track indentation depth, section nesting, whether
  we're inside a listitem that contains a codeblock.
- **Use semantic context**: a line with only right-aligned `*tags*` after a
  separator is a heading — we don't need a grammar rule to express this.
- **Be opinionated**: our formatter can enforce canonical form, eliminating
  ambiguity at the source.

### Specific issue-to-feature mapping

| Issue                          | What we can do (that tree-sitter can't)                             |
| ------------------------------ | ------------------------------------------------------------------- |
| #95 structured AST             | `documentSymbol` already nests symbols under headings               |
| #20 nested blocks              | Indentation-aware diagnostics and formatting                        |
| #21 nested lists               | Track list depth by indentation for outline/folding                 |
| #118/#163 codeblock + listitem | Multi-pass: detect codeblock boundaries first, then parse listitems |
| #110 h4 pseudo-heading         | Recognize tag-only lines as headings in `documentSymbol`            |
| #132 tables                    | Detect tab-aligned columns; format/align them                       |
| #113 unclosed backtick         | Diagnostic: "unclosed backtick — did you mean `` \` ``?"            |
| #94 `foo~` vs `foo ~`          | Recognize both; diagnostic if `~` has no preceding space            |
| #98 single-char h3             | Recognize `A FOO` as h3; diagnostic if ambiguous                    |
| #146 `< - text`                | Multi-pass: codeblock terminator `<` takes priority over listitem   |

---

## The Case for a Canonical Vimdoc

### The problem

There is no vimdoc spec. There is:

- `:help help-writing` (incomplete, informal)
- `$VIMRUNTIME/syntax/help.vim` (regex-based, no structure)
- `nanotee/vimdoc-notes` (best community effort, but descriptive not
  prescriptive)
- `tree-sitter-vimdoc` (prescriptive but constrained by parser framework)
- Convention from ~1500 runtime help files accumulated over 30 years

The result: every tool that touches vimdoc re-discovers the same edge cases.
tree-sitter-vimdoc has 14 open issues. Many are 2+ years old and unfixable
within the grammar framework.

### What "canonical vimdoc" means

Not a new format. A **subset** of existing vimdoc that is:

1. **Unambiguous** — every construct has exactly one parse.
2. **Round-trippable** — `format(parse(text))` produces identical output.
3. **Strict superset of tree-sitter-vimdoc** — anything canonical parses
   correctly in tree-sitter too (we don't create files that break downstream).
4. **Machine-enforceable** — the formatter and diagnostics together ensure
   canonical form.

### Concrete rules a canonical subset would enforce

**Structure:**

- Blank line required between sections, before/after codeblocks, before
  modeline.
- Separator lines are exactly `line_width` characters (default 78).
- Headings (h1/h2) always have a separator line immediately above.
- h3 headings are all-caps with at least 2 words (avoids #98 ambiguity).
- Right-aligned `*tag*` on a heading line — no floating tags mid-paragraph.

**Code blocks:**

- `>` preceded by exactly one space (not tab). Satisfies the spec requirement
  tree-sitter doesn't enforce.
- Explicit `<` terminator required (no implicit stop by column-1 text).
- Blank line after `<` terminator (avoids #146 `< - text` ambiguity).

**Lists:**

- Listitem prefix is `- ` or `N. ` at consistent indentation.
- Nested lists indented by exactly 2 spaces from parent.
- Codeblocks inside listitems: blank line before `>`, body indented to list
  depth + 2. Avoids #118/#163 entirely.

**Inline:**

- No unclosed delimiters (`*`, `|`, `` ` ``). Diagnostic on mismatch.
- `~` column heading requires preceding space (`foo ~` not `foo~`).
- Tags right-aligned to `line_width`. No mid-line tags except in headings.

**Encoding / Modeline:**

- UTF-8 only (no latin1).
- Modeline required as last line, preceded by blank line.

### What this buys us

- **Formatter**: `textDocument/formatting` produces canonical output. Users
  write loose vimdoc, run format, get clean files.
- **Diagnostics**: warn on non-canonical constructs before they hit tree-sitter
  edge cases. "codeblock inside listitem needs blank line separator" is
  actionable.
- **Interop**: canonical files parse cleanly in tree-sitter-vimdoc,
  `gen_help_html.lua`, helpview.nvim, and any other consumer.
- **Documentation**: the canonical rules become a reference that the ecosystem
  currently lacks.

### What this does NOT mean

- We do NOT fork the format. No new syntax, no extensions.
- We do NOT reject valid vimdoc. The parser accepts everything; the formatter
  and diagnostics nudge toward canonical form.
- We do NOT compete with tree-sitter-vimdoc. We complement it — our diagnostics
  catch inputs that would confuse the grammar.

---

## Priority for vimdoc-language-server

### High (directly impacts existing features)

- **#118/#163 codeblock+listitem**: our formatter already handles codeblocks; we
  need to ensure the formatted output doesn't hit this tree-sitter bug.
- **#110 h4 pseudo-heading**: `documentSymbol` should recognize tag-only lines
  as section markers.
- **#113 unclosed backtick**: our `scan_inline` already skips backtick-delimited
  spans — we should emit a diagnostic if one is unclosed.

### Medium (future features)

- **#132 tables**: formatter could align tab-separated columns.
- **#21 nested lists**: folding ranges, outline depth.
- **#95 structured AST**: already handled by our `documentSymbol` nesting.

### Low (nice to have)

- **#94 `foo~`**: recognize in parser, warn via diagnostic.
- **#98 single-char h3**: minor, rare in practice.
- **#58/#111 keycode parsing**: only relevant if we add semantic tokens.
