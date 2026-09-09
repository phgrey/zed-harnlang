# zed-harnlang

[https://zed.dev/](Zed) extension for [https://harnlang.com/](Harn) language (`.harn`) using external LSP tool.

- Extension ID: `harnlang`, `harnlang-lsp`
- Package: `zed-harnlang`
- Language: `Harn`
- LSP: `harn-lsp`

## Build

```bash
rustup target add wasm32-wasip2
```

## Prerequisites
Ensure `harn-lsp` is installed and accessible in your `PATH` (e.g. `~/.local/bin/harn-lsp`).
Refer guides https://harnlang.com/getting-started.html#install-harn


## Load in Zed
1. Open Zed
2. Command palette → `zed: extensions`
3. Search for `Harn`, click **install** on both `harnlang` and `harnlang-lsp` extensions.

## Language extension is in tree-sitter folder
https://github.com/phgrey/zed-harnlang-lsp

## LICENSE
MIT License. See [LICENSE](LICENSE) for details.
