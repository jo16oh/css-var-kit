import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["target", "**/*.md", "**/*.toml"],
  sortImports: true,
});
