import { fileURLToPath, URL } from "node:url";
import { defineConfig, type Plugin } from "vite";

// Tauri 2 dev server config. The fixed port + strictPort lets `tauri.conf.json`
// `devUrl` point at it deterministically. `clearScreen: false` keeps Rust
// compiler output visible during `tauri dev`.
const host = process.env.TAURI_DEV_HOST;

const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));

/** Web-mode (SP3/PWA) HTML rewrite: the desktop `index.html` stays untouched;
 *  in `--mode web` we retitle, relax CSP to allow cross-origin API calls
 *  (connect-src) and wire the PWA manifest + icons. */
function webIndexHtml(): Plugin {
  return {
    name: "rutbusiness-web-index",
    transformIndexHtml(html) {
      return html
        .replace("<title>Pharma Client</title>", "<title>RutBusiness</title>")
        .replace(
          /<meta http-equiv="Content-Security-Policy"[^>]*\/>/,
          `<meta http-equiv="Content-Security-Policy" content="default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' https: http:; manifest-src 'self'" />`,
        )
        .replace(
          "</head>",
          `  <link rel="manifest" href="/manifest.webmanifest" />\n` +
            `    <link rel="icon" type="image/png" href="/icons/pwa-128.png" />\n` +
            `    <link rel="apple-touch-icon" href="/icons/pwa-256.png" />\n` +
            `    <meta name="theme-color" content="#0b0d12" />\n` +
            `  </head>`,
        );
    },
  };
}

export default defineConfig(({ mode }) => {
  const web = mode === "web";
  return {
    clearScreen: false,
    // Web build (SP3): the SAME frontend served as a PWA — `@tauri-apps/api/core`
    // resolves to the fetch shim, the updater plugin to a no-op stub.
    resolve: web
      ? {
          alias: {
            "@tauri-apps/api/core": here("./src/web-transport/index.ts"),
            "@tauri-apps/plugin-updater": here("./src/web-transport/updater-stub.ts"),
          },
        }
      : undefined,
    plugins: web ? [webIndexHtml()] : [],
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
    // Build to ./dist for desktop (matches frontendDist); ./dist-web for web so
    // the two outputs never clobber each other.
    build: {
      outDir: web ? "dist-web" : "dist",
      target: "es2021",
      minify: !process.env.TAURI_DEBUG ? ("esbuild" as const) : false,
      sourcemap: !!process.env.TAURI_DEBUG,
    },
  };
});
