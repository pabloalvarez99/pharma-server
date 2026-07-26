// Shared fetch machinery for every shim command: timeout-bound fetch with the
// desktop conn-error mapping, query/body builders that replicate the Rust
// `filter(|s| !s.is_empty())` semantics, and JSON/text parsers that reproduce
// the per-command "Respuesta de X inválida" strings.

import { API_TIMEOUT_MS, connError } from "./errors";
import { tokenOf } from "./session";

export type CommandArgs = Record<string, unknown>;
export type CommandHandler = (a: CommandArgs) => Promise<unknown>;

export interface FetchOpts {
  method?: string;
  headers?: Record<string, string>;
  body?: BodyInit;
  timeoutMs?: number;
}

/** fetch with a hard timeout; connection failures throw the Spanish copy. */
export async function doFetch(url: string, opts: FetchOpts = {}): Promise<Response> {
  try {
    return await fetch(url, {
      method: opts.method ?? "GET",
      headers: opts.headers,
      body: opts.body,
      signal: AbortSignal.timeout(opts.timeoutMs ?? API_TIMEOUT_MS),
    });
  } catch (e) {
    throw connError(e);
  }
}

/** Like `doFetch` but conn errors reject as `"|message"` (coded commands). */
export async function doFetchCoded(url: string, opts: FetchOpts = {}): Promise<Response> {
  try {
    return await fetch(url, {
      method: opts.method ?? "GET",
      headers: opts.headers,
      body: opts.body,
      signal: AbortSignal.timeout(opts.timeoutMs ?? API_TIMEOUT_MS),
    });
  } catch (e) {
    throw `|${connError(e)}`;
  }
}

/** Bearer header from the stored session; throws the Spanish no-session copy. */
export function authHeaders(extra: Record<string, string> = {}): Record<string, string> {
  return { Authorization: `Bearer ${tokenOf()}`, ...extra };
}

/** `tokenOf` for coded commands: the no-session error is prefixed `"|"`. */
export function authHeadersCoded(extra: Record<string, string> = {}): Record<string, string> {
  try {
    return authHeaders(extra);
  } catch (e) {
    throw `|${String(e)}`;
  }
}

export const JSON_HEADERS = { "Content-Type": "application/json" };

/** Build a query string. Strings are skipped when empty (mirrors the Rust
 *  `filter(|s| !s.is_empty())`); numbers/booleans are appended when defined. */
export function qs(params: Record<string, unknown>): string {
  const u = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    if (typeof v === "string") {
      if (v !== "") u.set(k, v);
    } else {
      u.set(k, String(v));
    }
  }
  const s = u.toString();
  return s ? `?${s}` : "";
}

/** Attach `key` to `body` only when `v` is a non-empty string. */
export function putStr(body: Record<string, unknown>, key: string, v: unknown): void {
  if (typeof v === "string" && v !== "") body[key] = v;
}

/** Attach `key` whenever `v` is defined (numbers, booleans, objects). */
export function putDef(body: Record<string, unknown>, key: string, v: unknown): void {
  if (v !== undefined && v !== null) body[key] = v;
}

/** Parse JSON or throw `"{invalid}: {e}"` (per-command Spanish copy). */
export async function parseJson<T>(resp: Response, invalid: string): Promise<T> {
  try {
    return (await resp.json()) as T;
  } catch (e) {
    throw `${invalid}: ${e instanceof Error ? e.message : String(e)}`;
  }
}

/** Read raw text or throw `"{invalid}: {e}"`. */
export async function parseText(resp: Response, invalid: string): Promise<string> {
  try {
    return await resp.text();
  } catch (e) {
    throw `${invalid}: ${e instanceof Error ? e.message : String(e)}`;
  }
}

/** Port of catalog.rs `encode_path_segment`: percent-encode a single URL path
 *  segment leaving only unreserved ASCII literal. */
export function encodePathSegment(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let out = "";
  for (const b of bytes) {
    const c = String.fromCharCode(b);
    if (/[A-Za-z0-9\-_.~]/.test(c)) out += c;
    else out += `%${b.toString(16).toUpperCase().padStart(2, "0")}`;
  }
  return out;
}
