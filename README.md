# zed-harnlang

Native Zed extension for Harn language (`.harn`).

- Extension ID: `harnlang`
- Package: `zed-harnlang`
- Language: `Harn`
- LSP: `harn-lsp`

## Build

```bash
rustup target add wasm32-wasip2
```

## Load in Zed

1. Open Zed
2. Command palette → `zed: extensions`
3. Click **Install Dev Extension**
4. Select this directory

## Publishing Notes

- The extension defines the `harnlang` language server in `extension.toml` and links it to the `Harn` language.
- The language config in `languages/harn/config.toml` declares the `harn` grammar and the `.harn` / `.harn` suffixes.
- Replace the placeholder `tree-sitter-harn` repository and `rev` in `extension.toml` with your actual grammar repository and commit SHA before publishing.
- Ensure the global `harn-lsp` binary is available on the user's `PATH`.
