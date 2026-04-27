# css-var-kit for Zed

Zed extension that provides type-aware CSS variable completion, diagnostics, rename, and go-to-definition by wrapping the [`cvk`](https://github.com/jo16oh/css-var-kit) language server.

Supports CSS, SCSS, HTML, Vue, Svelte, and Astro.

## Installation

Install the **css-var-kit** extension from Zed's Extensions panel.

The extension resolves the `cvk` binary in this order:

1. `lsp.css-var-kit.binary.path` in your Zed settings
2. `cvk` on `$PATH` (e.g., installed via `cargo install css-var-kit` or `npm install -D css-var-kit`)
3. The matching `css-var-kit-{platform}-{arch}.tar.gz` from the latest [GitHub release](https://github.com/jo16oh/css-var-kit/releases), downloaded automatically into the extension's working directory

## Configuration

Configuration mirrors the VS Code extension. If a `cvk.json` or `cvk.jsonc` exists in the workspace root, it takes precedence and Zed settings for the language server are ignored.

Example `settings.json`:

```json
{
  "lsp": {
    "css-var-kit": {
      "binary": {
        "path": "/usr/local/bin/cvk"
      },
      "initialization_options": {
        "rootDir": ".",
        "lookupFiles": ["**/*.css"],
        "excludeFiles": [],
        "rules": {
          "no-undefined-variable-use": "error",
          "no-variable-type-mismatch": "error",
          "no-inconsistent-variable-definition": "error",
          "enforce-variable-use": "off"
        },
        "lsp": {
          "logFile": null
        }
      }
    }
  }
}
```

See [docs/config.md](https://github.com/jo16oh/css-var-kit/blob/main/docs/config.md) for the full set of options.

## Development

```sh
rustup target add wasm32-wasip2
cd crates/zed
cargo build --release --target wasm32-wasip2
```

In Zed: `extensions` panel → `Install Dev Extension` → select `crates/zed/`.
