# zed-harnlang

[Zed](https://zed.dev/) extension for [Harn](https://harnlang.com/) language (`.harn`) using external LSP tool.

- Extension ID: `harn-lang`
- Package: `zed-harnlang`
- Language: `Harn`
- LSP: `harn-lsp`


## Prerequisites
Ensure `harn-lsp` is installed and accessible in your `PATH` (e.g. `~/.local/bin/harn-lsp`).
Refer guides https://harnlang.com/getting-started.html#install-harn


## Load in Zed
1. Open Zed
2. Command palette → `zed: extensions`
3. Search for `Harn`, click **install**, open any `.harn` file to ensure the extension is loaded.

## Features
Additionally to syntax highlighting, the extension provides:
- LSP support for `.harn` files (hover, go to definition, find references)
- Jumps to Rust source code for Harn standard library functions (if `harn-lsp` is installed with `--with-source` option)

## Development

### Run Tests
```bash
cargo test --workspace --all-targets
```

### Code Coverage
Run coverage using `cargo-llvm-cov`:
```bash
cargo llvm-cov --workspace --all-features
```

Generate LCOV report:
```bash
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
```

## LICENSE
MIT License. See [LICENSE](LICENSE) for details.
