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

- [x] **Formatting** — separator normalization, prose reflow, heading alignment
- [x] **Diagnostics** — duplicate tag definitions
- [x] **Document symbols** — all `*tag*` definitions
- [x] **Go-to-definition** — `|tag-ref|` to `*tag*` in the same file
- [ ] **Completion** — tag completion from `*tag*` definitions
- [ ] **Hover** — documentation preview for tags
- [ ] **References** — find all references to a tag
- [ ] **Rename** — rename tags and their references
- [ ] **Cross-file navigation** — go-to-definition across files
- [ ] **Semantic tokens** — syntax-aware highlighting
- [ ] **Code actions** — quick fixes and refactors
- [ ] **Folding** — section-based fold ranges

## License

[MIT](LICENSE)
