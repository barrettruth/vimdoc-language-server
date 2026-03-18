# vimdoc-language-server

Language server for vim help files.

## Installation

### Cargo

```sh
cargo install vimdoc-language-server
```

### Nix

```sh
nix run github:barrettruth/vimdoc-language-server
```

### From source

```sh
git clone https://github.com/barrettruth/vimdoc-language-server
cd vimdoc-language-server
cargo install --path .
```

## Usage

Configure `vimdoc-language-server` in your editor of choice, for example with
[Neovim](https://neovim.io):

```lua
vim.lsp.config('vimdoc_ls', {
  cmd = { 'vimdoc-language-server' },
  filetypes = { 'help' },
  root_markers = { 'doc', '.git' },
})
vim.lsp.enable('vimdoc_ls')
```

## Features

- [x] **Formatting** — separator normalization, prose reflow, heading alignment;
      range formatting supported
- [x] **Diagnostics** — duplicate `*tag*` definitions (same-file and
      cross-file), unresolved `|taglinks|`
- [x] **Completion** — tag completion triggered by `|`, context-aware
- [x] **Hover** — tag definition context in a floating window
- [x] **Go-to-definition** — `|tag-ref|` to `*tag*`, same-file and cross-file
- [x] **References** — all `|taglinks|` referencing a `*tag*`, cross-file
- [x] **Rename** — rename a tag and all its references across the workspace
- [x] **Document symbols** — all `*tag*` definitions in the current file
- [x] **Document highlight** — highlight all occurrences of a tag under cursor
- [x] **Document links** — clickable `|taglinks|` with tooltip
- [x] **Folding** — sections (between separators) and code blocks
- [ ] **Code actions** — quick fixes and refactors
- [ ] **Semantic tokens** — syntax-aware highlighting

## License

[MIT](LICENSE)
