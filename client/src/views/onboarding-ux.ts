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
import { featuresForRubro, type Rubro } from "../vertical";

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

// --- 2b. Rubro-aware dashboard activation (guided first value) --------------
//
// `dashboardCta` classifies a panel from catalog/sales alone. The dashboard,
// though, must also be MULTI-RUBRO native: a service rubro (belleza/servicios,
// `physicalStock:false`) sells without inventory, so "tu negocio está vacío —
// importa productos" is wrong and confusing for a peluquero. This layer wraps
// the canonical classifier and re-frames the next step per rubro, so there is
// ONE place that decides "what should this operator do next on the panel".
//
// It reuses `dashboardCta` for product rubros (thresholds never diverge) and
// drives service rubros purely off sales. The returned CTA carries the nav id
// the dashboard button should jump to (the existing sidebar nav), so empty
// states teach the real next move instead of dead-ending.

/** Nav ids the activation CTA can route to (subset of the shell nav). */
export type ActivationNav = "importar" | "pos" | "configuracion";

export interface ActivationInput {
  /** Total products in the catalog (null = stats unknown / not fetched). */
  productCount: number | null;
  /** Any sales recorded yet. */
  hasSales: boolean;
  /** Whether the sales stat is actually known (false = fetch failed → don't
   *  nag a rubro whose activation depends on sales). */
  salesKnown: boolean;
  /** The operator's chosen rubro — decides product- vs service-shaped guidance. */
  rubro: Rubro;
}

export interface ActivationCta {
  title: string;
  /** Teaching body — explains the next step, never just a label. */
  body: string;
  /** Primary button label (the action it teaches). */
  label: string;
  /** Sidebar nav id the primary button jumps to. */
  nav: ActivationNav;
  /** Optional secondary link (e.g. "o carga datos demo") so demo stays one
   *  click away without competing with the primary teaching action. */
  secondary?: { label: string; nav: ActivationNav };
}

export interface DashboardActivation {
  state: CtaState;
  /** Whether this rubro tracks physical stock — the dashboard uses it to pick
   *  product-shaped (valor/stock/vencimiento) vs service-shaped KPIs. */
  physicalStock: boolean;
  /** The guided next-step CTA, or null when the panel is "ready". */
  cta: ActivationCta | null;
}

/** Decide the guided next step for the dashboard, rubro-aware. Product rubros
 *  flow fresh → import catalog → first sale → ready; service rubros skip the
 *  inventory steps entirely (no physical stock) and flow no-sales → first
 *  service → ready. Single source the panel renders from. */
export function dashboardActivation(input: ActivationInput): DashboardActivation {
  const physicalStock = featuresForRubro(input.rubro).physicalStock;

  // Service rubros: productCount is meaningless (they sell no stock), so the
  // only signal is sales. An unknown sales stat must NOT nag.
  if (!physicalStock) {
    if (!input.salesKnown) return { state: "unknown", physicalStock, cta: null };
    if (input.hasSales) return { state: "ready", physicalStock, cta: null };
    return {
      state: "stock-only",
      physicalStock,
      cta: {
        title: "Listo para tu primer cobro",
        body: "Tu rubro vende servicios, sin inventario. Abre la caja y registra tu primer servicio en el POS.",
        label: "Registrar primer servicio",
        nav: "pos",
      },
    };
  }

  // Product rubros: reuse the canonical classifier so thresholds never drift.
  const base = dashboardCta({ productCount: input.productCount, hasSales: input.hasSales });
  if (!base.cta) return { state: base.state, physicalStock, cta: null };

  if (base.cta.action === "seed-demo") {
    // Empty catalog → teach the real first move: load your own products. Demo
    // stays reachable as the secondary link for someone just trying the system.
    return {
      state: base.state,
      physicalStock,
      cta: {
        title: "Carga tu catálogo",
        body: "Importa tus productos desde un archivo CSV para empezar a vender. ¿Solo estás probando? Carga datos de demostración.",
        label: "Importar productos",
        nav: "importar",
        secondary: { label: "Cargar datos demo", nav: "configuracion" },
      },
    };
  }

  // Catalog present, no sales yet → make the first sale.
  return {
    state: base.state,
    physicalStock,
    cta: {
      title: "Listo para vender",
      body: "Ya tienes productos cargados. Abre la caja y registra tu primera venta en el POS.",
      label: "Hacer primera venta",
      nav: "pos",
    },
  };
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

// --- 4. Branch slug ("Sucursal") persistence -------------------------------

export const TENANT_STORE_KEY = "pharma:tenant_slug";

/** Generic fallback when no branch slug has been stored yet. Matches the
 *  operator manual's suggested branch name. */
export const DEFAULT_TENANT_SLUG = "principal";

/** Read the persisted branch slug, defaulting to {@link DEFAULT_TENANT_SLUG}.
 *  The server derives the slug from the business NAME at first-run setup (e.g.
 *  "Almacén Don José" -> "almacen-don-jose"), which is almost never "principal";
 *  persisting it keeps the next launch's Sucursal pre-fill correct so the lone
 *  owner isn't locked out of their own freshly-installed server. */
export function loadStoredTenant(store: KeyStore): string {
  let raw: string | null;
  try {
    raw = store.getItem(TENANT_STORE_KEY);
  } catch {
    return DEFAULT_TENANT_SLUG;
  }
  const v = raw?.trim();
  return v ? v : DEFAULT_TENANT_SLUG;
}

/** Persist the branch slug that worked. No-throw (storage may be unavailable);
 *  a blank slug is ignored so it never overwrites a good value. */
export function saveStoredTenant(store: KeyStore, slug: string): void {
  const v = slug.trim();
  if (!v) return;
  try {
    store.setItem(TENANT_STORE_KEY, v);
  } catch {
    /* noop — field just falls back to the default next launch */
  }
}
