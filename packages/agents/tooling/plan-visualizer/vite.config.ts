import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

/**
 * Dev-server config only. The production single-file builds are driven by
 * build-examples.mjs, which runs one single-input build per example so the
 * single-file plugin can inline everything into one .html (rollup forbids
 * inlineDynamicImports with multiple inputs). Dev just needs the @lib alias;
 * open /action-system.html, /auth-flatten.html, or /future-schema.html.
 */
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@lib": fileURLToPath(new URL("./lib", import.meta.url)) },
  },
});
