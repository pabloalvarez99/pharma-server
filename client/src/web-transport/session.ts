// Web replacement of client/src-tauri/src/state.rs: the JWT lives in memory,
// mirrored to sessionStorage so a browser reload (F5) keeps the session alive
// for that tab. NEVER localStorage — closing the tab/browser drops the token
// (same "re-login each launch" spirit as the desktop client).

const TOKEN_KEY = "rb.web.token";

let memToken: string | null = null;

/** Pull the session JWT or throw the exact Spanish "no session" string the
 *  desktop `token_of` uses. */
export function tokenOf(): string {
  if (memToken) return memToken;
  try {
    const t = sessionStorage.getItem(TOKEN_KEY);
    if (t) {
      memToken = t;
      return t;
    }
  } catch {
    /* storage unavailable → treated as no session */
  }
  throw "No hay sesión activa. Inicia sesión primero.";
}

/** Store a freshly-issued token (login / first-run setup). */
export function storeToken(token: string): void {
  memToken = token;
  try {
    sessionStorage.setItem(TOKEN_KEY, token);
  } catch {
    /* memory-only session still works for this page lifetime */
  }
}

/** Forget the session (logout / return to LoginView). */
export function clearToken(): void {
  memToken = null;
  try {
    sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    /* noop */
  }
}
