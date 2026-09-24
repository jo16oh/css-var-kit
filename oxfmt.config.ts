import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: [
    "target",
    "**/*.toml",
    // Fixtures intentionally containing BOM, trailing commas, invalid UTF-8 or unclosed comments.
    "crates/css-var-kit/tests/fixtures/bom",
    "crates/css-var-kit/tests/fixtures/trailing-comma",
    "crates/css-var-kit/tests/fixtures/unreadable-config",
    "crates/css-var-kit/tests/fixtures/unterminated-comment",
  ],
  sortImports: true,
});
