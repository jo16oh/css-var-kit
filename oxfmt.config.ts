import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["dist", "build", "target", "crates/css-var-kit/tests", "**/*.md", "**/*.toml"],
});
