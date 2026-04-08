import { defineConfig } from "oxlint";

export default defineConfig({
  plugins: ["eslint", "typescript", "unicorn", "oxc"],
  categories: {
    correctness: "error",
  },
  rules: {},
  env: {
    builtin: true,
  },
  options: {
    typeAware: true,
  },
});
