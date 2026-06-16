// First-run journey — the pure, DOM-free logic behind getting a brand-new RUT
// (a real Chilean operator) from a fresh install to a populated, working app
// WITHOUT touching a CLI or reading a manual. The North Star of the multi-rubro
// pivot is "install -> login -> first sale, alone, frictionless"; this module is
// the single source of truth for the decisions that journey makes so login.ts /
// configuracion.ts / shell.ts / dashboard.ts all agree and so the flow is
// unit-testable end to end (vitest journeys simulate it, no clicks, no jsdom).
//
// What lives here (all pure):
//  - server URL resolution + first-launch detection (extracted from login.ts so
//    there is ONE definition, not a drifting copy);
//  - inline validation of the server URL the operator types (Spanish errors);
//  - mapping a /health probe to an operator-facing connection state (an
//    unreachable server must yield a clear Spanish message, never a crash);
//  - the first-run step machine (what to do next, with a soft block on rubro);
//  - whether the dashboard looks "poblado" after loading demo data;
//  - which ERP modules a rubro shows (the gate shell.ts applies, single-sourced).
import type { HealthInfo, InventorySummary, DailySalesRow } from "../api";
import { type Vertical, type Rubro, hasRecetas, featuresForRubro } from "../vertical";

// --- Server URL resolution + first-launch detection -------------------------

/** Loopback guess used when nothing else is known. A LAN client on another
 *  machine must override this — hence [`resolveServerConfig`] flags it. */
export const FALLBACK_SERVER = "http://127.0.0.1:8080";

export interface ServerResolution {
  /** Value to pre-fill the server field with. */
  value: string;
  /** True when neither a remembered server nor a build-time override exists, so
   *  the loopback is only a guess and the "Conexión avanzada" panel should open
   *  by default (a LAN client must not be silently pointed at its own host). */
  firstLaunch: boolean;
}

/** Resolve the initial server URL + whether this looks like a never-configured
 *  first launch. Priority: a server the operator already logged into > a
 *  build-time `VITE_SERVER_URL` baked for a field install > the loopback guess. */
export function resolveServerConfig(opts: {
  stored?: string | null;
  env?: string | null;
}): ServerResolution {
  const stored = (opts.stored ?? "").trim() || undefined;
  const env = (opts.env ?? "").trim() || undefined;
  return {
    value: stored ?? env ?? FALLBACK_SERVER,
    firstLaunch: !stored && !env,
  };
}

// --- Inline server URL validation (Spanish) ---------------------------------

export type ServerUrlCheck =
  | { ok: true; url: string }
  | { ok: false; error: string };

/** Validate (and normalise) a server URL the operator typed, inline, so a typo
 *  is caught before they fill credentials. Requires an http(s) scheme + a host,
 *  strips a trailing slash. Errors are operator-facing Spanish. */
export function validateServerUrl(raw: string): ServerUrlCheck {
  const v = (raw ?? "").trim();
  if (!v) {
    return { ok: false, error: "La URL del servidor no puede estar vacía." };
  }
  if (!/^https?:\/\//i.test(v)) {
    return {
      ok: false,
      error: "La URL debe empezar con http:// o https:// (ej: http://192.168.1.50:8080).",
    };
  }
  let parsed: URL;
  try {
    parsed = new URL(v);
  } catch {
    return { ok: false, error: "La URL del servidor no es válida. Revisa que esté bien escrita." };
  }
  if (!parsed.hostname) {
    return { ok: false, error: "Falta la dirección del servidor (IP o nombre)." };
  }
  // Normalise: drop a single trailing slash on the root path so the stored value
  // is stable across "…:8080" and "…:8080/".
  const normalized = v.replace(/\/+$/, "");
  return { ok: true, url: normalized };
}

// --- Connection state from a /health probe ----------------------------------

export type ConnKind = "checking" | "ok" | "degraded" | "unreachable";

export interface ConnState {
  kind: ConnKind;
  /** Operator-facing Spanish line for the status pill. */
  message: string;
}

/** Map a /health probe to an operator-facing connection state. `threw` is true
 *  when the probe itself failed (server down / wrong URL); a null `health` with
 *  `threw=false` means "still probing". Crucially: an unreachable server yields
 *  a clear Spanish message here, the view never has to crash on a thrown probe. */
export function connectionState(
  health: HealthInfo | null,
  threw = false,
): ConnState {
  if (threw || (health && !health.reachable)) {
    return {
      kind: "unreachable",
      message: "No se pudo contactar al servidor. Verifica que esté encendido y la URL sea correcta.",
    };
  }
  if (!health) {
    return { kind: "checking", message: "Probando conexión…" };
  }
  if (health.status === "ok" && health.db === "ok") {
    return { kind: "ok", message: "✓ Servidor accesible." };
  }
  return {
    kind: "degraded",
    message: `El servidor responde pero está degradado (estado: ${health.status}, base de datos: ${health.db}).`,
  };
}

// --- First-run step machine -------------------------------------------------

export interface FirstRunState {
  /** /health probe says the server is reachable AND ok. */
  serverReachable: boolean;
  /** A session was obtained (login succeeded). */
  loggedIn: boolean;
  /** `business.vertical` is set server-side (operator picked a rubro). */
  verticalChosen: boolean;
  /** Catalog has at least one product (demo loaded or real data entered). */
  hasData: boolean;
}

export type FirstRunStep =
  | "configurar-servidor"
  | "iniciar-sesion"
  | "elegir-rubro"
  | "cargar-datos"
  | "listo";

export interface FirstRunGuidance {
  step: FirstRunStep;
  title: string;
  hint: string;
  /** A soft block points the operator at a step but never traps them (no dead
   *  end / hard modal). Hard blocks gate progress (can't login to a down server). */
  soft: boolean;
}

/** Decide what the operator should do next on first run. Order mirrors the real
 *  flow: reach the server (hard) -> log in (hard) -> pick a rubro (SOFT: the app
 *  works as generic ERP meanwhile) -> load demo or real data (soft) -> done. */
export function firstRunStep(state: FirstRunState): FirstRunGuidance {
  if (!state.serverReachable) {
    return {
      step: "configurar-servidor",
      title: "Conecta tu servidor",
      hint: "Escribe la dirección del servidor de tu local (o usa el de este equipo) y prueba la conexión.",
      soft: false,
    };
  }
  if (!state.loggedIn) {
    return {
      step: "iniciar-sesion",
      title: "Inicia sesión",
      hint: "Ingresa con la cuenta que te entregó tu administrador.",
      soft: false,
    };
  }
  if (!state.verticalChosen) {
    return {
      step: "elegir-rubro",
      title: "Elige tu rubro",
      hint: "En Configuración, elige tu rubro para mostrar solo las secciones que usas. Puedes seguir usando el ERP genérico mientras tanto.",
      soft: true,
    };
  }
  if (!state.hasData) {
    return {
      step: "cargar-datos",
      title: "Carga tus productos",
      hint: "Importa tu catálogo, créalo a mano, o carga datos demo para probar el sistema.",
      soft: true,
    };
  }
  return {
    step: "listo",
    title: "Todo listo",
    hint: "Tu negocio está configurado. Abre el POS para tu primera venta.",
    soft: false,
  };
}

// --- Dashboard readiness ----------------------------------------------------

export interface DashboardReadiness {
  /** The dashboard has believable data to show (vs an empty-state screen). */
  populated: boolean;
  /** One-line Spanish summary for an empty/populated banner. */
  headline: string;
}

/** After loading demo (or real) data, is the dashboard "poblado"? Drives whether
 *  the operator sees a believable overview or a friendly empty state — a fresh
 *  install must NOT dead-end on blank tiles with no guidance. */
export function dashboardReadiness(
  summary: InventorySummary | null,
  sales: DailySalesRow[] | null,
): DashboardReadiness {
  const products = summary?.total ?? 0;
  const soldToday = (sales ?? []).some((r) => r.orders > 0);
  if (products <= 0) {
    return {
      populated: false,
      headline: "Aún no hay productos. Carga tu catálogo o datos demo para empezar.",
    };
  }
  return {
    populated: true,
    headline: soldToday
      ? `${products} producto(s) en catálogo · ventas registradas hoy.`
      : `${products} producto(s) en catálogo · sin ventas aún hoy.`,
  };
}

// --- Module visibility per rubro (single-sources shell's nav gate) ----------

/** Canonical ERP module ids, in nav order. `recetas` is the only pharmacy-only
 *  one (Ley 20.000); everything else is universal (boleta/factura DTE included,
 *  since every CL business emits). */
export const ALL_MODULES = [
  "dashboard",
  "pos",
  "devoluciones",
  "inventory",
  "importar",
  "caja",
  "clientes",
  "compras",
  "recetas",
  "boletas",
  "facturas",
  "gastos",
  "reports",
  "auditoria",
  "configuracion",
] as const;

export type ModuleId = (typeof ALL_MODULES)[number];

/** Operator-facing label per module (Spanish), for the onboarding preview that
 *  shows which sections a rubro turns on. */
export const MODULE_LABELS: Readonly<Record<ModuleId, string>> = {
  dashboard: "Panel",
  pos: "Punto de venta",
  devoluciones: "Devoluciones",
  inventory: "Inventario",
  importar: "Importar",
  caja: "Caja",
  clientes: "Clientes",
  compras: "Compras",
  recetas: "Recetas",
  boletas: "Boletas",
  facturas: "Facturas",
  gastos: "Gastos",
  reports: "Reportes",
  auditoria: "Auditoría",
  configuracion: "Configuración",
};

/** Modules visible for a rubro: drop `recetas` unless it's farmacia. Single
 *  source of the gate shell.ts applies in `hydrateBranding`. Kept on the coarse
 *  {@link Vertical} for back-compat; prefer {@link visibleModulesForRubro}. */
export function visibleModules(vertical: Vertical): ModuleId[] {
  return ALL_MODULES.filter((m) => (m === "recetas" ? hasRecetas(vertical) : true));
}

/** Modules visible for a full rubro, gated by its capability flags
 *  ({@link featuresForRubro}). Service rubros (belleza/servicios) also drop the
 *  physical-stock modules (inventario/compras) — they sell without inventory.
 *  Single source the shell nav + onboarding preview both read. */
export function visibleModulesForRubro(rubro: Rubro): ModuleId[] {
  const f = featuresForRubro(rubro);
  return ALL_MODULES.filter((m) => {
    if (m === "recetas") return f.recetas;
    if (m === "inventory" || m === "compras") return f.physicalStock;
    return true;
  });
}
