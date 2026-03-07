vim.api.nvim_create_autocmd('FileType', {
  pattern = 'help',
  callback = function()
    vim.lsp.start({
      name = 'vimdoc',
      cmd = { 'vimdoc-language-server' },
    })
  end,
})
