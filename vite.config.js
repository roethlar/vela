// Vela's frontend is a single-page Svelte 5 app. Tauri has no Node server: it
// embeds the static files Vite writes to build/ (src-tauri/tauri.conf.json
// "frontendDist": "../build"). tests/frontend-shell.test.mjs keeps this file,
// tsconfig.json, index.html, src/main.ts and tauri.conf.json in agreement.
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [svelte()],

  resolve: {
    // `$lib` is defined once, in tsconfig.json "paths"; svelte-check reads the same entry.
    tsconfigPaths: true,
  },

  build: {
    outDir: "build",
    // Clear stale output (including a pre-migration build/_app/) before every build.
    emptyOutDir: true,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
