/// <reference types="vite/client" />
// Web transport shim (SP3, ADR-0015 P2): drop-in replacement of
// `@tauri-apps/api/core` for the browser build. Vite aliases the import here in
// `--mode web`; the 18 views and `src/api/*` stay byte-identical. Each command
// maps 1:1 to the HTTP call its Rust counterpart performs, with the same
// Spanish error strings.

import { registry, DESKTOP_ONLY_ERROR } from "./registry";

/** Web default server (SaaS API). Overridable at build time via
 *  `VITE_DEFAULT_SERVER_URL`; the desktop keeps its loopback default. */
const DEFAULT_WEB_SERVER: string =
  import.meta.env.VITE_DEFAULT_SERVER_URL || "https://api.rutbusiness.cl";

// Seed the shared server-url store (localStorage `pharma:last-server`, see
// src/api/server-url.ts) so the login screen pre-fills the SaaS API on first
// visit instead of the desktop loopback fallback.
try {
  if (!localStorage.getItem("pharma:last-server")) {
    localStorage.setItem("pharma:last-server", DEFAULT_WEB_SERVER);
  }
} catch {
  /* private mode — login still lets the operator type a URL */
}

// PWA: register the shell-only service worker (production builds only; data
// requests always hit the network — the SW never caches API calls).
if (import.meta.env.PROD && typeof navigator !== "undefined" && "serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker.register("/sw.js").catch(() => {
      /* SW is an enhancement; the app works without it */
    });
  });
}

/** Drop-in for Tauri's `invoke`: dispatch to the fetch-backed handler. Unknown
 *  commands (e.g. `plugin:*` internals) degrade with the desktop-only error —
 *  callers that care already catch and fall back. */
export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = registry[cmd];
  if (!handler) throw DESKTOP_ONLY_ERROR;
  return (await handler(args)) as T;
}
