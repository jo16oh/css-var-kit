import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: [
    "target",
    "**/*.toml",
    // Fixtures intentionally containing BOM, trailing commas or invalid UTF-8.
    "crates/css-var-kit/tests/fixtures/bom",
    "crates/css-var-kit/tests/fixtures/trailing-comma",
    "crates/css-var-kit/tests/fixtures/unreadable-config",
  ],
  sortImports: true,
});
