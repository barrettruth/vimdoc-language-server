# Vimdoc to Markdown: Feasibility Analysis

---

## Prior Art: `gen_help_html.lua`

Neovim already ships a vimdoc→HTML converter at `src/gen/gen_help_html.lua`. It
walks the tree-sitter-vimdoc AST via recursive `visit_node()` and emits HTML.
Key properties:

- Uses `tagmap` (tag → source file) for cross-file link resolution
- Handles two layout modes: `old-help-para` (preformatted) and `help-para`
  (reflowed) — most files are still "old" layout
- Detects h4 pseudo-headings by checking indentation of `*tag*` nodes (>8 spaces
  = heading)
- Tracks list nesting depth via whitespace comparison between siblings
- Filters "noise lines" (boilerplate, modelines)
- Has a full validation pass (`visit_validate`) that catches broken links, parse
  errors, and misspellings

This is ~1200 lines of Lua. It works. It ships Neovim's online docs at
neovim.io/doc/user/. It is the most battle-tested vimdoc consumer that exists.

**The implication**: vimdoc→markdown is not a research problem. The node-type
mapping is already solved for HTML. The question is whether markdown can express
everything HTML can.

---

## The Mapping

### Clean mappings (vimdoc → markdown, lossless)

| Vimdoc              | Markdown                           | Notes                                                                       |
| ------------------- | ---------------------------------- | --------------------------------------------------------------------------- |
| `*tag*`             | `<a id="tag"></a>`                 | HTML anchor in markdown. No pure-markdown equivalent for arbitrary anchors. |
| `\|taglink\|`       | `[taglink](file.md#tag)`           | Standard markdown link. Requires tagmap for cross-file resolution.          |
| `'option'`          | `['option'](options.md#option)`    | Same as taglink but with quotes preserved in display text.                  |
| `` `code` ``        | `` `code` ``                       | Direct mapping.                                                             |
| `>lang ... <`       | ` ```lang ... ``` `                | Fenced code block. Clean mapping.                                           |
| `> ... <`           | ` ``` ... ``` `                    | Fenced code block, no language.                                             |
| `======` + heading  | `## Heading`                       | h1 separator → h2 (h1 reserved for doc title).                              |
| `------` + heading  | `### Heading`                      | h2 separator → h3.                                                          |
| `UPPERCASE HEADING` | `#### HEADING`                     | h3 → h4.                                                                    |
| `- item`            | `- item`                           | Direct mapping.                                                             |
| `1. item`           | `1. item`                          | Direct mapping.                                                             |
| `{argument}`        | `{argument}` or `` `{argument}` `` | Could go either way. HTML uses `<code>`.                                    |
| `<C-w>` (keycode)   | `<kbd>C-w</kbd>`                   | HTML `<kbd>` in markdown. Or backtick-wrapped.                              |
| `https://...`       | `<https://...>` or bare URL        | Most markdown renderers auto-link bare URLs.                                |
| `Note:`             | `**Note:**`                        | Bold. Or blockquote `> **Note:**`.                                          |
| modeline            | (dropped)                          | No equivalent. Or HTML comment `<!-- vim:... -->`.                          |

### Lossy / problematic mappings

| Vimdoc                                | Problem                                                      | Options                                                                                   |
| ------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| Right-aligned `*tag*` on heading line | Markdown headings can't have right-aligned content           | (a) Drop alignment, put anchor before heading. (b) Use HTML: `<h2 id="tag">Heading</h2>`. |
| Multiple `*tags*` on one line         | Multiple anchors. Markdown has no multi-anchor syntax.       | Emit multiple `<a id="..."></a>` on preceding line.                                       |
| Tag-only line (h4)                    | No h5 in most renderers; `#####` looks bad                   | Use `<h4 id="tag">tag</h4>` or bold + anchor.                                             |
| Column heading (`foo ~`)              | No direct equivalent                                         | Bold text, or HTML `<strong>`.                                                            |
| Tab-aligned tables                    | Markdown tables require `\|` delimiters and alignment row    | Convert to pipe tables. Lossy for complex alignment.                                      |
| Preformatted layout (most files)      | Hard-wrapped at 78 chars. Markdown expects unwrapped prose.  | Must un-wrap paragraphs. This is the hardest problem.                                     |
| Indented blocks (not code)            | Markdown treats 4-space indent as code                       | Must detect and un-indent. Or use `>` blockquote.                                         |
| `CTRL-W s` then tab then `*CTRL-W_s*` | Keycode + aligned tag pattern. Common in reference sections. | Definition list? Table? No clean markdown equivalent.                                     |

### The hard problem: paragraph un-wrapping

Vimdoc prose is hard-wrapped at 78 characters. Consecutive lines form a
paragraph. But "consecutive lines" is context-dependent:

- Indented lines may be a continuation or a preformatted block
- A line starting with `|tag|` could be a reference list, not prose
- Lines after a column heading (`~`) may be table rows
- Lines in "old layout" files are preformatted — every linebreak is intentional

`gen_help_html.lua` handles this by having two layout modes. The "old" mode
preserves all whitespace (90%+ of files use this). The "new" mode reflows.

A markdown converter must un-wrap paragraphs in the "new" layout files, and for
"old" layout files must decide: preformatted block, or attempt to detect
paragraph boundaries?

Neovim's own approach in `gen_help_html.lua`: punt. Old-layout files get
`<div class="old-help-para">` which is styled with `white-space: pre` in CSS.
The text is preserved verbatim. This is not viable in markdown — there is no
`white-space: pre` equivalent for prose.

---

## Architecture Options

### Option A: Tree-sitter-vimdoc walk (like gen_help_html.lua)

Walk the tree-sitter AST, emit markdown per node.

Pros:

- Proven approach (gen_help_html.lua does exactly this for HTML)
- Reuses existing parser infra
- Could be a Lua script shipped with Neovim (like gen_help_html.lua)

Cons:

- Inherits all tree-sitter-vimdoc limitations (#118 codeblock+listitem, #21
  nested lists, #110 h4, #132 tables)
- The AST is flat — no section nesting, no indentation semantics
- Paragraph un-wrapping requires heuristics the AST can't inform

### Option B: Our parser → markdown

Use vimdoc-language-server's multi-pass parser. We already handle:

- Codeblock boundaries (including implicit stops)
- Section hierarchy
- Tag/taglink extraction with ranges
- Prose vs preformatted classification

Pros:

- Richer structural analysis than tree-sitter
- Can handle edge cases tree-sitter can't
- Paragraph un-wrapping is already partially solved (our formatter reflows)

Cons:

- Our parser is currently line-oriented, not block-structured
- Would need significant enrichment (lists, tables, headings, indentation)
- Separate tool from Neovim's infra

### Option C: Hybrid — tree-sitter AST + our semantic analysis

Use tree-sitter-vimdoc for the primary parse (it's fast, incremental, and
already available in Neovim). Supplement with our analysis for:

- Section nesting (we infer from heading hierarchy)
- Paragraph boundary detection (we use indentation + blank lines)
- List depth (we track indentation)
- Codeblock-in-listitem (we handle correctly)
- h4 detection (we use tag position heuristics)

This is what `gen_help_html.lua` effectively does — it uses tree-sitter for the
AST but carries its own state for indentation, noise filtering, tag validation,
and layout mode.

Pros:

- Best of both worlds
- Can run inside Neovim (tree-sitter is native)
- Leverages tree-sitter's speed and incremental parsing
- Our semantic layer fills the structural gaps

Cons:

- Two-layer architecture is more complex
- Need to handle tree-sitter ERROR nodes gracefully

**Option C is the right answer.** It's what gen_help_html.lua already is, just
targeting markdown instead of HTML, with our richer semantic analysis filling
the gaps tree-sitter can't cover.

---

## What This Means for the Project

A vimdoc→markdown converter is:

1. **Independently valuable** — plugin authors want to generate GitHub-friendly
   docs from their help files. Right now they maintain two copies or use
   google/vimdoc in reverse.

2. **Feeds Neovim #329/#36249** — the markdown migration needs a converter.
   `gen_help_html.lua` proves the approach works. A markdown target is the
   natural next step. Having a battle-tested converter with richer structural
   analysis than `gen_help_html.lua` is a concrete contribution.

3. **Validates our parser** — if our analysis can produce clean markdown from
   real vimdoc files (including Neovim's own runtime docs), that proves the
   parser handles the format correctly.

4. **Could ship as a subcommand** — `vimdoc-language-server convert foo.txt` →
   `foo.md`. Same binary, leveraging the same parser infrastructure.

5. **Informs the spec** — every conversion decision ("how do we represent
   right-aligned tags?") is a spec decision. Writing the converter forces us to
   make every ambiguity explicit.

---

## Open Questions

- Should the converter live in this repo (Rust, `convert` subcommand) or as a
  Lua script (like gen_help_html.lua, runs inside Neovim)?
- Does Neovim want a Lua-based converter they own, or an external tool?
  (justinmk's 2022 comment suggests they'd want it in-tree.)
- For "old layout" files: attempt paragraph detection, or emit as fenced code
  blocks / preformatted?
- Tag anchors: `<a id="tag"></a>` (HTML in markdown) or heading IDs only?
- Should the converter be round-trippable (markdown→vimdoc→markdown = identity)?
  Probably not — this is a one-way migration tool.
