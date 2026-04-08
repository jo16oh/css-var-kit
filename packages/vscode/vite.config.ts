import { defineConfig } from "vite";

export default defineConfig({
  build: {
    lib: {
      entry: "src/extension.ts",
      formats: ["es"],
      fileName: "extension",
    },
    rollupOptions: {
      external: ["vscode"],
    },
    outDir: "dist",
  },
});
