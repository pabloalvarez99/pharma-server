import { defineConfig } from "vite";

// Tauri 2 dev server config. The fixed port + strictPort lets `tauri.conf.json`
// `devUrl` point at it deterministically. `clearScreen: false` keeps Rust
// compiler output visible during `tauri dev`.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 5183 }
      : undefined,
    watch: {
      // Don't watch the Rust side from Vite; Tauri handles that.
      ignored: ["**/src-tauri/**"],
    },
  },
  // Build to ../dist relative to this config (matches frontendDist).
  build: {
    outDir: "dist",
    target: "es2021",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
