# vimdoc-language-server

> **⚠️ Experimental — not ready for use.**
> Everything is incomplete. The parser is naive, diagnostics are minimal,
> and the formatter will mangle files. Do not use this on anything you
> care about.

Language server for vim help files.

## Status

Pre-alpha. The following capabilities exist in skeletal form:

- `textDocument/formatting` — separator normalization, heading alignment, prose reflow
- `textDocument/publishDiagnostics` — duplicate tag definitions
- `textDocument/documentSymbol` — tag definitions
- `textDocument/definition` — same-file tag links

None of these are reliable yet.

## Install

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

```lua
vim.lsp.start({
  name = 'vimdoc',
  cmd = { 'vimdoc-language-server' },
  filetypes = { 'help' },
  root_dir = vim.fn.getcwd(),
})
```

See [`minimal_init.lua`](minimal_init.lua) for a standalone example.

## CLI

```
vimdoc-language-server [OPTIONS]

Options:
  --line-width <N>    Target line width [default: 78]
  --no-formatting     Disable formatting
  --no-diagnostics    Disable diagnostics
  --log-file <PATH>   Write logs to file
  -v, --verbose       Increase verbosity
```

## License

[MIT](LICENSE)
