# Vimdoc Format Reference

Sources: `:help help-writing`,
[nanotee/vimdoc-notes](https://github.com/nanotee/vimdoc-notes),
[neovim/tree-sitter-vimdoc README](https://github.com/neovim/tree-sitter-vimdoc).

---

## Overview

Vimdoc (`.txt` files under `doc/`) is a plain-text format for Vim/Neovim help
files. There is no formal spec — the authoritative sources are
`:help help-writing`, the Vim `syntax/help.vim` highlight definitions, and
convention observed across the Neovim runtime docs.

Files must:

- Use `latin1` or `UTF-8` encoding (UTF-8 detected by non-ASCII in first line).
- End with a `\n` (tree-sitter-vimdoc parser requirement).
- Conventionally end with a blank line before the modeline.

---

## Structure

A typical help file looks like:

```
*plugin.txt*  Short one-line description       *plugin*

==============================================================================
INTRODUCTION                                          *plugin-introduction*

Text here.

------------------------------------------------------------------------------
SECTION TWO                                           *plugin-section-two*

Column heading~

text     text     text

 vim:tw=78:ts=8:ft=help:norl:
```

---

## Syntax Elements

### Tags (`*tag*`)

Defined with asterisks. Must be followed by whitespace or newline. Used by
`:helptags` to generate the `tags` file.

```
*tag-name*
```

Vim syntax rule:

```vim
syn match helpHyperTextEntry  "\*[#-)!+-~]\+\*\s"he=e-1
syn match helpHyperTextEntry  "\*[#-)!+-~]\+\*$"
```

Convention: right-align tags to column 78.

---

### Taglinks (`|tag|`)

Cross-references to tags. Pressing `<C-]>` on a taglink jumps to its definition.

```
|tag-name|
```

Vim syntax rule:

```vim
syn match helpHyperTextJump  "\\\@<!|[#-)!+-~]\+|"
```

---

### Optionlinks (`'option'`)

Link to a Vim option. Only lowercase ASCII, minimum 2 chars. Also `'t_xx'`
(terminal option, two arbitrary chars after `t_`).

```
'textwidth'
't_Co'
```

Vim syntax rules:

```vim
syn match helpOption  "'[a-z]\{2,\}'"
syn match helpOption  "'t_..'"
```

---

### Codespan (`` `code` ``)

Inline code. May contain whitespace. Renders as a command/code highlight.

```
`echo 'hello'`
```

---

### Code Block (`>` ... `<`)

Preformatted block. Start: `>` at end of a line (preceded by space, not tab —
spec requirement, though parsers are lenient). End: `<` at start of line, OR any
line starting at column 1 (implicit stop).

Block content must be indented by at least one space/tab.

```
Example: >
    function! Example() abort
        echo 'blah'
    endfunction
<
```

Optionally, a language tag can follow `>` on the same line (Neovim extension):

```
>lua
    vim.print("hello")
<
```

Vim syntax rule:

```vim
syn region helpExample  matchgroup=helpIgnore start=" >$" start="^>$"
    \ end="^[^ \t]"me=e-1 end="^<" concealends
```

---

### Section Delimiters (h1 / h2)

**h1** (`=` separator): at least 6 `=` signs spanning the line. Followed on the
next line by heading text and a right-aligned tag.

**h2** (`-` separator): same but with `-`.

```
==============================================================================
SECTION NAME                                               *section-tag*

------------------------------------------------------------------------------
Subsection Name                                         *subsection-tag*
```

The delimiter also accepts embedded text:

```
==================================headline====================================
```

Vim syntax rule:

```vim
syn match helpSectionDelim  "^===.*===$"
syn match helpSectionDelim  "^---.*--$"
```

---

### Column Heading (`~`)

Any text followed by `~` at end of line. Renders with heading highlight.

```
Column heading~
```

Also: UPPERCASE words (with or without a right-aligned tag) are treated as
headings and appear in `gO` outline.

```
COLUMN HEADING                                       *column-heading*

COLUMN HEADING
```

Vim syntax rule:

```vim
syn match helpHeader  "\s*\zs.\{-}\ze\s\=\~$" nextgroup=helpIgnore
```

---

### h3 (UPPERCASE heading)

An all-caps line (matching `[A-Z0-9.()][-A-Z0-9.()_]+`) with optional tags. Not
marked by a separator line.

```
UPPERCASE NAME                                          *tag*
```

---

### Keycodes

```
<Esc>  <Enter>  <S-Right>  <C-W>
CTRL-X  CTRL-SHIFT-A  META-U  ALT-J
CTRL-Break  CTRL-PageUp  CTRL-PageDown
CTRL-{char}
```

Vim syntax rules (abbreviated):

```vim
syn match helpSpecial  "<[-a-zA-Z0-9_]\+>"
syn match helpSpecial  "<[SCM]-.>"
syn match helpSpecial  "CTRL-."
syn match helpSpecial  "CTRL-SHIFT-."
syn match helpSpecial  "META-."
syn match helpSpecial  "ALT-."
```

---

### Arguments / Parameters

Required args in `{}`, optional args in `[]`:

```
:command {arg1} {arg2} [, {optionalarg}]
func({required} [, {optional}])
```

`{}` with no whitespace inside.

Vim syntax rule:

```vim
syn match helpSpecial  "{[-_a-zA-Z0-9'"*+/:%#=[\]<>.,]\+}"
```

---

### Notes / Warnings

Auto-highlighted keywords:

```
Note:  NOTE:  Notes  Notes:
Warning:  WARNING:
Deprecated  DEPRECATED  DEPRECATED:
```

---

### URLs

```
https://www.vim.org/
https://neovim.io/
```

Vim syntax rule:

```vim
syn match helpURL `\v<(((https?|ftp|gopher)://|(mailto|file|news):)[^' <>"]+
    \|(www|web|w3)[a-z0-9_-]*\.[a-z0-9._-]+\.[^' <>"]+)[a-zA-Z0-9/]`
```

---

### Modeline

Must be the last line (or last non-blank line), preceded by a blank line.

```
 vim:tw=78:ts=8:ft=help:norl:
```

---

## Lists

### Unordered

```
- Item 1
- Item 2
```

Also `•` (U+2022) bullet character.

### Ordered

Items with `1.`, `2.` etc. (number, period, space). Note: tree-sitter treats any
line starting with `[0-9].` as a listitem, even mid-paragraph.

---

## Tables (Convention Only)

No formal syntax. Two styles in use:

**Style 1** (column heading + aligned text):

```
header1          header2 ~
foo              barbazquux
verylongword     test
```

**Style 2** (pseudo-table):

```
 -------------+-----------
 header1      | header2  ~
 -------------+-----------
 foo          | barbazquux
 -------------+-----------
```

Must start with whitespace to avoid the `---` line being parsed as an h2 section
delimiter.

---

## Line Width Convention

Standard is 78 characters. Tags are typically right-aligned to column 78.
Separator lines span the full width. Prose is reflowed to fit.

---

## Encoding

Files should declare encoding via modeline or be detectable from content. UTF-8
is detected if non-ASCII appears in the first line.
