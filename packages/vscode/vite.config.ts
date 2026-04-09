import { defineConfig } from "vite";

export default defineConfig({
  build: {
    ssr: "src/extension.ts",
    rollupOptions: {
      external: ["vscode"],
      output: {
        format: "cjs",
        entryFileNames: "extension.cjs",
      },
    },
    outDir: "dist",
  },
});
