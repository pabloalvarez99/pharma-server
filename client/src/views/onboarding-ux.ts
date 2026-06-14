// Onboarding UX hardening — pure logic (no DOM, single-source) shared by the
// login + dashboard views and their tests. Lane B goal: a first-run never leaves
// the operator stuck or confused. Two concerns, both framework-free so vitest
// can drive them without jsdom:
//   1. connection retry/timeout — bounded backoff + a hard timeout so an
//      unreachable server gives feedback instead of an infinite spinner.
//   2. dashboardCta — classify a freshly-installed (un-seeded) panel so the view
//      can show an actionable CTA instead of a wall of "$0".
// Server URL validation + persistence delegate to first-run.ts's
// `validateServerUrl` (the canonical single source) — this module no longer
// defines its own copy. Spanish user-facing strings; English codes.
// See teamwork_op.txt LANE B.

import { validateServerUrl } from "./first-run";

// --- 1. Connection retry / timeout -----------------------------------------

/** Bounded backoff schedule (ms) for connection retries. The last value repeats
 *  for any attempt past the array; attempts stop at MAX_CONN_ATTEMPTS so the UI
 *  never loops forever. */
export const CONN_BACKOFF_MS = [400, 1200, 3000] as const;
export const MAX_CONN_ATTEMPTS = 4;
/** Per-attempt hard ceiling — a hung socket must not freeze the spinner. */
export const CONN_TIMEOUT_MS = 8000;

/** Delay before retry `attempt` (1-based: delay BEFORE attempt #2 = index 0). */
export function connRetryDelay(attempt: number): number {
  const i = Math.max(0, attempt - 2);
  return CONN_BACKOFF_MS[Math.min(i, CONN_BACKOFF_MS.length - 1)];
}

/** Should the UI try again after a failed attempt? Stops at MAX_CONN_ATTEMPTS. */
export function shouldRetryConn(attempt: number): boolean {
  return attempt < MAX_CONN_ATTEMPTS;
}

export class TimeoutError extends Error {
  constructor() {
    super("La conexión tardó demasiado (timeout).");
    this.name = "TimeoutError";
  }
}

/** Race a promise against a hard timeout. Rejects with TimeoutError if the work
 *  doesn't settle in `ms`. `setTimer` is injectable so tests run without real
 *  time. The pending work is abandoned (JS can't cancel a promise) but the UI
 *  stops waiting — that's the point. */
export function withTimeout<T>(
  work: Promise<T>,
  ms: number = CONN_TIMEOUT_MS,
  setTimer: (cb: () => void, ms: number) => unknown = setTimeout,
  clearTimer: (h: unknown) => void = clearTimeout as (h: unknown) => void,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const handle = setTimer(() => reject(new TimeoutError()), ms);
    work.then(
      (v) => {
        clearTimer(handle);
        resolve(v);
      },
      (e) => {
        clearTimer(handle);
        reject(e);
      },
    );
  });
}

/** Operator-facing message for a failed connection attempt. Distinguishes
 *  timeout from a refused/other error, and signals whether a retry is coming so
 *  the status line reads "Reintentando…" instead of a dead spinner. */
export function connFeedback(
  err: unknown,
  attempt: number,
): { message: string; willRetry: boolean } {
  const willRetry = shouldRetryConn(attempt);
  const base =
    err instanceof TimeoutError
      ? "La conexión tardó demasiado."
      : typeof err === "string"
        ? err
        : "No se pudo contactar al servidor.";
  if (willRetry) {
    const secs = Math.round(connRetryDelay(attempt + 1) / 100) / 10;
    return { message: `${base} Reintentando en ${secs}s…`, willRetry };
  }
  return {
    message: `${base} Verifica que el servidor esté encendido y la IP/puerto sean correctos.`,
    willRetry,
  };
}

// --- 2. Dashboard empty-state CTA ------------------------------------------
//
// NOTE: first-run.ts owns `dashboardReadiness` (the populated/headline summary).
// This is the distinct, richer CTA classifier the dashboard panel uses to drive
// an actionable onboarding banner — kept under a different name so there is no
// ambiguity with first-run's canonical readiness.

export type CtaState = "fresh" | "stock-only" | "ready" | "unknown";

export interface CtaInput {
  /** Total products in the catalog (from /products/stats `total`). */
  productCount: number | null;
  /** Any sales recorded yet (sum of daily orders > 0). */
  hasSales: boolean;
}

export interface DashboardCta {
  state: CtaState;
  /** When non-null, the dashboard shows this onboarding CTA banner. */
  cta: { title: string; body: string; action: "seed-demo" | "first-sale" | "config" } | null;
}

/** Classify a panel so a fresh install gets a next step, not an empty wall.
 *   - no catalog            → seed demo data (or configure)        → "fresh"
 *   - catalog but no sales  → make the first sale                   → "stock-only"
 *   - sales exist           → normal dashboard, no CTA              → "ready"
 *   - stats unavailable     → no CTA (don't nag on a transient error)→ "unknown" */
export function dashboardCta(input: CtaInput): DashboardCta {
  if (input.productCount == null) return { state: "unknown", cta: null };
  if (input.productCount === 0) {
    return {
      state: "fresh",
      cta: {
        title: "Tu negocio está vacío",
        body: "Carga datos de demostración para probar el sistema, o agrega tus productos en Inventario.",
        action: "seed-demo",
      },
    };
  }
  if (!input.hasSales) {
    return {
      state: "stock-only",
      cta: {
        title: "Listo para vender",
        body: "Ya tienes productos cargados. Abre la caja y registra tu primera venta en el POS.",
        action: "first-sale",
      },
    };
  }
  return { state: "ready", cta: null };
}

// --- 3. Server URL persistence ---------------------------------------------

/** Minimal storage shape (a subset of the Web Storage API) so persistence is
 *  testable without a real localStorage. */
export interface KeyStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const SERVER_STORE_KEY = "pharma:last-server";

/** Read the persisted server URL, re-validating it (a corrupt/garbage stored
 *  value must not poison the field). Returns undefined when absent or invalid.
 *  Validation + normalisation delegate to first-run's `validateServerUrl`. */
export function loadStoredServer(store: KeyStore): string | undefined {
  let raw: string | null;
  try {
    raw = store.getItem(SERVER_STORE_KEY);
  } catch {
    return undefined;
  }
  if (!raw) return undefined;
  const res = validateServerUrl(raw);
  return res.ok ? res.url : undefined;
}

/** Persist the server URL in canonical form. No-throw: a storage failure (quota,
 *  privacy mode) must never break login. Returns whether it was stored. */
export function saveStoredServer(store: KeyStore, url: string): boolean {
  const res = validateServerUrl(url);
  if (!res.ok) return false;
  try {
    store.setItem(SERVER_STORE_KEY, res.url);
    return true;
  } catch {
    return false;
  }
}
