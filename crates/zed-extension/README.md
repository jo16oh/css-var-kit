# css-var-kit for Zed

Zed extension that provides type-aware CSS variable completion, diagnostics, rename, and go-to-definition by wrapping the [`cvk`](https://github.com/jo16oh/css-var-kit) language server.

## Installation

Install the **css-var-kit** extension from Zed's Extensions panel.

The extension resolves the `cvk` binary in this order:

1. `lsp.css-var-kit.binary.path` in your Zed settings
2. `cvk` on `$PATH` (e.g., installed via `cargo install css-var-kit` or `npm install -D css-var-kit`)
3. The matching `css-var-kit-{platform}-{arch}.tar.gz` from the latest [GitHub release](https://github.com/jo16oh/css-var-kit/releases), downloaded automatically into the extension's working directory

## Configuration

If a `cvk.json` or `cvk.jsonc` exists in the workspace root, it takes precedence and `initialization_options` are ignored.

Example `settings.json`:

```json
{
  "lsp": {
    "css-var-kit": {
      "binary": {
        "path": "/usr/local/bin/cvk",
        "args": ["lsp", "--log"]
      },
      "initialization_options": {
        "rootDir": ".",
        "lookupFiles": ["**/*.css"],
        "rules": {
          "no-undefined-variable-use": "error",
          "no-variable-type-mismatch": "error",
          "no-inconsistent-variable-definition": "error",
          "enforce-variable-use": "off"
        },
        "lsp": {
          "logFile": "/path/to/cvk.log"
        }
      }
    }
  }
}
```

See [docs/config.md](https://github.com/jo16oh/css-var-kit/blob/main/docs/config.md) for the full set of options.
