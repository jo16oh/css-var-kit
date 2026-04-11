<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/cvk-banner-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/cvk-banner-light.png">
    <img alt="css-var-kit" src="assets/cvk-banner-light.png">
  </picture>
</p>

# css-var-kit

A simple, lightweight toolkit for building design systems with CSS variables, offering **type-aware** completion and linting.

[![npm version](https://img.shields.io/npm/v/css-var-kit.svg)](https://www.npmjs.com/package/css-var-kit)
[![Crates.io](https://img.shields.io/crates/v/css-var-kit.svg)](https://crates.io/crates/css-var-kit)

## Demo

<img alt="demo" src="assets/demo.webp">

## Installation ⬇️

```sh
npm install -D css-var-kit
```

Or install via Cargo:

```sh
cargo install css-var-kit
```

## Commands 🧰

### `cvk lint`

Lints CSS variables and their usage. Detects undefined variables, type mismatches, inconsistent definitions, and enforces variable usage for design tokens.

👉 [More Documentation](docs/linter.md)

### `cvk lsp`

A language server for CSS variables that offers type-aware variable completion and lint warnings.

Supported Features

- **Show diagnostics** from `cvk lint`
- **Type-aware variable completion**
- **Rename variable**
- **Go to defintition**

## Editor Integration

### VSCode

👉 [Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=jo16oh.css-var-kit-vscode)

### Helix

```toml
# languages.toml
[language-server.css-var-kit]
command = "cvk"
args = ["lsp"]

[[language]]
name = "css"
language-servers = ["css-var-kit"]
```

## Planned Features 📝

- [ ] Supports Vue, Svelte, Astro, and style tags within HTML.
- [ ] `cvk prune` command
  - Strips unused CSS variables from the final build output.
- [ ] Zed Extension
- [ ] Adding configuration examples for Vim, Neovim, and Emacs.
