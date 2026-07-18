// Server base URL persistence (ADR-0015 P0 — configurable server target).
//
// The client is API-first (ADR-0015): every command takes an explicit
// `serverUrl`, which flows from login. These helpers persist/recall the target
// so a LAN terminal (tablet caja) or a relocated desktop can point at the
// on-prem server's IP without rebuilding. localhost stays the default. Shared
// with login.ts via the same localStorage key.

/** localStorage key holding the last/working server URL (shared with login). */
export const SERVER_STORE_KEY = "pharma:last-server";
/** Loopback default — desktop co-installed with the server. */
export const FALLBACK_SERVER_URL = "http://127.0.0.1:8080";

/** Resolve the configured server base URL: persisted value, else loopback.
 *  Never throws — onboarding must not dead-end on a storage hiccup. */
export function storedServerUrl(): string {
  try {
    return localStorage.getItem(SERVER_STORE_KEY)?.trim() || FALLBACK_SERVER_URL;
  } catch {
    return FALLBACK_SERVER_URL;
  }
}

/** Persist the server base URL so the next launch/login pre-fills it. */
export function rememberServerUrl(url: string): void {
  try {
    localStorage.setItem(SERVER_STORE_KEY, url.trim());
  } catch {
    /* noop — running without persistence is still usable this session */
  }
}
