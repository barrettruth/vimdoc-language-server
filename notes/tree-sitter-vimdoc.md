# tree-sitter-vimdoc Notes

Repo: [neovim/tree-sitter-vimdoc](https://github.com/neovim/tree-sitter-vimdoc)

---

## Design Philosophy

> Predictable results are the primary goal, so that _output_ formats (e.g. HTML)
> are well-formed; the _input_ (vimdoc) is secondary.

The grammar does not attempt to handle every quirk in real-world vimdoc files.
The preferred fix is to clean the input, not relax the grammar.

---

## Top-Level Structure

```
help_file
  _blank*
  block*
  modeline*
```

`block` is the fundamental structural unit — a paragraph or group of adjacent
lines separated by blank lines.

---

## Node Reference

### `help_file`

Root node. Consumes leading blank lines, then alternates `block` and `modeline`.

### `block`

A group of adjacent lines (one or more `line` or `line_li`), followed by a blank
line or `<` (codeblock terminator). Blank lines after the terminator are
consumed here.

A `block` contains either:

- `repeat1(line)`
- `repeat1(line_li)`
- `repeat1(line)` then `repeat1(line_li)`

### `line`

A single content line. Can be one of:

- `column_heading`
- `h1`
- `h2`
- `h3`
- `codeblock`
- `_line_noli` (plain content line, not starting with a listitem token)

### `line_li`

A listitem. Starts with `prefix` (`-`, `•`, or `N.`). Consumes its first line
and all subsequent adjacent non-listitem lines. Nesting is ignored — indented
listitems are parsed as siblings. Consumers check leading whitespace to infer
depth.

### `codeblock`

Contained by `line` or `line_li` (not `block`), because `>` can start a
codeblock at the end of any line. Contains `line` (aliased from `line_code`)
nodes — raw text including whitespace, not parsed further. Optional `language`
node when `>lang` is used. Ends when a line starts at column 1; the terminating
`<` is discarded as anonymous.

### `line_code`

Raw codeblock line: either blank (`\n`) or indented (`[\t ]+[^\n]+\n`).

### `modeline`

`vim:[^\n]+\n` with `prec(2)`. Must be preceded by a blank line.

---

## Headings

### `h1`

```
delimiter  (===...===)
heading    (text atoms)
tag?
\n
```

Delimiter: `/============+[\t ]*\n/` (token.immediate, so no whitespace before).

### `h2`

Same as h1 but `/------------+[\t ]*\n/`.

### `h3`

```
heading    (uppercase_name)
tag?
atoms*
\n
```

`uppercase_name`: starts with `[A-Z0-9.()][-A-Z0-9.()_]+`, then optional
additional uppercase words.

### `column_heading`

```
heading    (_column_heading alias)
delimiter  (~)
\n (token.immediate)
```

`_column_heading` is a hidden rule aliased to `heading`. Only recognizes `~`
preceded by a space (`foo ~` not `foo~`).

---

## Inline Atoms

All `_atom` variants share `_atom_common`:

| Node         | Pattern           | Notes                                                |
| ------------ | ----------------- | ---------------------------------------------------- | --------- |
| `tag`        | `*text*`          | `text = /[^*\n\t ]+/`                                |
| `taglink`    | `\|text\|`        | `text = /[^                                          | \n\t ]+/` |
| `optionlink` | `'text'`          | `text = /[a-z][a-z]+/` (min 2 chars)                 |
| `codespan`   | `` `text` ``      | May contain whitespace: `/[^``\n]+/`                 |
| `argument`   | `{text}`          | No whitespace: `/[^}\n\t ]+/`, optional trailing `?` |
| `url`        | bare URL          | `https?://...`, strips trailing `.,)].,:`            |
| `keycode`    | `<Key>`, `CTRL-x` | Various patterns                                     |
| `note`       | keywords          | `Note:`, `WARNING:`, `Deprecated`, etc.              |
| `word`       | fallback          | `/[^.,(\[\n\t ]+/` at `prec(-1)`                     |

`_atom_noli` is `_atom` but `word` is replaced by `word_noli` — a word that
cannot begin with a listitem token (`-`, `•`, or digit+`.`).

---

## Token Disambiguation

From the grammar comments:

- **Match Specificity**: string literals beat regexes (tree-sitter rule).
- **Rule Order**: earlier rules win on ties.
- Underscore-prefixed rules are hidden (not exposed as named nodes).
- Use JS regex (`/\n/`) not string literals (`'\n'`) unless the node should
  appear as anonymous in queries.

Conflicts declared:

```js
conflicts: ($) => [[$._line_noli, $._column_heading], [$._column_heading]]
```

`_column_heading` uses `prec.dynamic(1, ...)` to resolve ambiguity with a plain
`_line_noli`.

---

## `_word_common` — Explicit Non-Matches

The grammar explicitly captures a number of characters/sequences as `word`
(plain text) to prevent them from being mistakenly parsed as delimiters:

- `*` alone (not a tag)
- `'` alone, `'x` (single non-lowercase char), `'x'` (single char) — not
  optionlink
- `||`, `|` — not taglink
- `{`, `{}`, `{{...` — not argument
- `(`, `)`, `[`, `]`, `~`, `>`, `,`, `.`

---

## Known Issues / Limitations

- Input must end with `\n`.
- Input should end with a blank line (not strictly enforced in practice).
- Any line starting with `1.` (or other digit) is treated as a listitem, even
  mid-paragraph. Example: `"Foo was 0, not\n1. Uh oh."` breaks.
- Codeblock delimiter `>` must technically be preceded by a space (not tab) per
  spec; the grammar does not enforce this.
- `url` cannot contain `]` anywhere (workaround: URL-encode `%5D`). This is
  intentional to support `[text](url)` markdown-style links.
- `column_heading` only matches `foo ~` (space before `~`), not `foo~`. Covers
  99% of real files.
- `column_heading` children are parsed as `_atom` (not plaintext), noted as a
  TODO.
- `modeline` must be preceded by a blank line.

---

## TODO (from upstream)

- `h4` "tag heading": a line containing only tags, or ending with a tag.

---

## Relevance to vimdoc-language-server

The grammar is the most complete structural description of vimdoc that exists.
Key things to cross-reference:

- **Parser**: our `src/parser.rs` should agree with tree-sitter node semantics
  for `tag`, `taglink`, `optionlink`, `h1`/`h2`/`h3`, `column_heading`,
  `codeblock`, `line_li`.
- **Diagnostics**: the grammar's known-issue list is a good source of diagnostic
  rules (e.g. listitem-mid-paragraph, codeblock preceded by tab).
- **Formatter**: heading alignment, separator normalization, codeblock
  indentation all map to grammar nodes.
- **Hover / completion**: `taglink` and `optionlink` are the primary hover
  targets.
