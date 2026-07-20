# vimdoc-language-server

Language server for vim help files.

> [!NOTE]
> Due to GitHub's historic unreliability, active development is hosted on
> [Forgejo](https://git.barrettruth.com/barrettruth/vimdoc-language-server).
> GitHub is maintained as a read-only mirror.

<video src="https://github.com/user-attachments/assets/99ba05f6-9dbf-4645-8491-cf10af208743" width="100%" controls muted playsinline></video>

## Installation

### Cargo

```sh
cargo install vimdoc-language-server
```

### Nix

```sh
nix run git+https://git.barrettruth.com/barrettruth/vimdoc-language-server.git
```

### From source

```sh
git clone https://git.barrettruth.com/barrettruth/vimdoc-language-server.git
cd vimdoc-language-server
cargo install --path .
```

## Usage

Configure `vimdoc-language-server` in your editor of choice, for example with
[Neovim](https://neovim.io) via
[nvim-lspconfig](https://github.com/neovim/nvim-lspconfig):

```lua
vim.lsp.enable('vimdoc_ls')
```

### CLI

The server also provides standalone CLI subcommands that work without an editor.

**Format** vimdoc files (in-place or check-only for CI):

```sh
vimdoc-language-server format doc/
vimdoc-language-server format --check doc/*.txt
vimdoc-language-server --line-width 80 format doc/
```

**Check** for diagnostics (duplicate tags, unresolved taglinks):

```sh
vimdoc-language-server check doc/
vimdoc-language-server check --ignore unresolved-tag doc/
```

## Features

- [x] **Formatting** — separator normalization, prose reflow, heading alignment;
      range formatting supported
- [x] **Diagnostics** — duplicate `*tag*` definitions (same-file and
      cross-file), unresolved `|taglinks|`; push, pull, and CLI (`check`)
- [x] **Completion** — tag completion triggered by `|`, context-aware
- [x] **Hover** — tag definition context in a floating window
- [x] **Go-to-definition** — `|tag-ref|` to `*tag*`, same-file and cross-file
- [x] **References** — all `|taglinks|` referencing a `*tag*`, cross-file
- [x] **Rename** — rename a tag and all its references across the workspace
- [x] **Document symbols** — all `*tag*` definitions in the current file
- [x] **Document highlight** — highlight all occurrences of a tag under cursor
- [x] **Document links** — clickable `|taglinks|` with tooltip
- [x] **Folding** — sections (between separators) and code blocks
- [x] **Code actions** — quick fixes and refactors

## Acknowledgements

- [@skewb1k](https://github.com/skewb1k) - pull diagnostics deduplication fix
