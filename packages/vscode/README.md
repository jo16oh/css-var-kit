<p align="center">
  <img alt="css-var-kit" src="https://raw.githubusercontent.com/jo16oh/css-var-kit/main/assets/cvk-banner-light.png">
</p>

# css-var-kit

A VS Code extension for CSS variable tooling — diagnostics, type-aware completion, go-to-definition, and rename powered by the [css-var-kit](https://github.com/jo16oh/css-var-kit) LSP.

## Features

- **Diagnostics** — undefined variables, type mismatches, inconsistent definitions
- **Type-aware completion** — suggests only variables whose type matches the property (e.g. color variables for `color:`, length variables for `padding:`)
- **Go to definition** — jump to CSS variable declarations
- **Rename** — rename a variable across all files

## Getting Started

The extension automatically finds the `cvk` binary. Install it via npm:

```sh
npm install -D css-var-kit
```

If the binary is not found in node_modules, it will be resolved according to the [Binary Resolution Order](#binary-resolution-order).

## Configuration

If your project has a `cvk.json` (or `cvk.jsonc`), the extension uses it directly.

If no config file exists, you can configure rules via VS Code settings:

| Setting                                      | Default        | Description                       |
| -------------------------------------------- | -------------- | --------------------------------- |
| `cvk.path`                                   | `null`         | Path to the `cvk` binary          |
| `cvk.rootDir`                                | `"."`          | Root directory for analysis       |
| `cvk.lookupFiles`                            | `["**/*.css"]` | Glob patterns for CSS files       |
| `cvk.rules.noUndefinedVariableUse`           | `"error"`      | Undefined variable usage          |
| `cvk.rules.noVariableTypeMismatch`           | `"error"`      | Variable type mismatch            |
| `cvk.rules.noInconsistentVariableDefinition` | `"error"`      | Inconsistent variable definitions |
| `cvk.rules.enforceVariableUse`               | `"off"`        | Enforce CSS variable usage        |
| `cvk.lsp.logFile`                            | `null`         | LSP log file path                 |

> When `cvk.json` exists, it takes full precedence and VS Code settings are ignored.

See [Linter documentation](https://github.com/jo16oh/css-var-kit/blob/main/docs/linter.md) for details on rules and configuration options.

## Binary Resolution Order

The extension resolves the `cvk` binary in this order:

1. **`cvk.path` setting** — explicit path configured in VS Code settings
2. **node_modules** — `@css-var-kit/cli-{platform}-{arch}` package in the workspace
3. **PATH** — `cvk` on the system PATH
4. **Bundled binary** — platform-specific binary shipped with the extension

## Links

- [GitHub](https://github.com/jo16oh/css-var-kit)
- [Linter documentation](https://github.com/jo16oh/css-var-kit/blob/main/docs/linter.md)
