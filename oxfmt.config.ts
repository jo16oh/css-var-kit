import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["target", "crates/css-var-kit/tests", "**/*.md", "**/*.toml"],
  sortImports: true,
});
