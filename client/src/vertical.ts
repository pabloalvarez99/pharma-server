// Business vertical (rubro) — the core signal of the freemium multi-rubro pivot.
//
// pharma-server is sold as a generic on-prem ERP. From 2026-08-08 the primary
// focus is **feria / calle** (ADR-0022); farmacia remains a full vertical pack
// but is not the product north star. The operator picks their rubro in
// Configuración (`business.vertical`). The UI reads the pack flags to show/hide
// sections so a feriante never sees barcode/printer/DTE as first-day work.
//
// This module is the single source of truth for the concept so every lane
// (onboarding shell nav here, Recetas/Facturas conditioning in the compliance
// lane) agrees on the key, the allowed values, and the gating rules. Pure
// helpers (no I/O) live here too so they are unit-testable.

import { getSetting } from "./api";

/** Admin setting key holding the chosen rubro. */
export const VERTICAL_KEY = "business.vertical";
/** Admin setting key holding the operator-chosen business display name. */
export const BUSINESS_NAME_KEY = "business.name";

/** Vertical the install operates as. `otro` = generic ERP, no vertical extras. */
export type Vertical = "farmacia" | "minimarket" | "otro";

/** Default when the setting is unset — generic, NOT pharmacy. The pivot rule
 *  is "no hardcodear farmacia por defecto". */
export const DEFAULT_VERTICAL: Vertical = "otro";

/** Human label per vertical (Spanish, operator-facing). */
export function verticalLabel(v: Vertical): string {
  switch (v) {
    case "farmacia":
      return "Farmacia";
    case "minimarket":
      return "Minimarket / Almacén";
    case "otro":
      return "Otro rubro";
  }
}

/** Catalog for the Configuración selector (value + label + one-line help). */
export const VERTICAL_OPTIONS: ReadonlyArray<{
  value: Vertical;
  label: string;
  help: string;
}> = [
  {
    value: "farmacia",
    label: "Farmacia",
    help: "Habilita Recetas y el Libro de controlados (Ley 20.000).",
  },
  {
    value: "minimarket",
    label: "Minimarket / Almacén",
    help: "POS, inventario y boletas SII. Sin módulo de recetas.",
  },
  {
    value: "otro",
    label: "Otro rubro",
    help: "ERP genérico. Sin secciones específicas de un rubro.",
  },
] as const;

/** Coerce an arbitrary stored string into a known `Vertical`, defaulting to
 *  [`DEFAULT_VERTICAL`] for unset/unknown values (forward-compatible). */
export function parseVertical(raw: string | null | undefined): Vertical {
  switch ((raw ?? "").trim().toLowerCase()) {
    case "farmacia":
      return "farmacia";
    case "minimarket":
      return "minimarket";
    case "otro":
      return "otro";
    default:
      return DEFAULT_VERTICAL;
  }
}

/** Whether the pharmacy-only Recetas / controlled-drug ledger applies. Only
 *  `farmacia` keeps Ley 20.000 obligations; every other rubro hides it. */
export function hasRecetas(v: Vertical): boolean {
  return v === "farmacia";
}

/** DTE/boleta: most formal CL rubros emit; feria (informal) does not on day 1.
 *  Prefer {@link featuresForRubro} / pack `dte` when the full rubro key is known. */
export function hasDte(v: Vertical | string): boolean {
  const r = parseRubro(typeof v === "string" ? v : v);
  return featuresForRubro(r).dte;
}

/** Load the persisted vertical from the server, defaulting on any miss. Never
 *  throws — onboarding must not dead-end on a settings hiccup. */
export async function loadVertical(serverUrl: string): Promise<Vertical> {
  try {
    const s = await getSetting(serverUrl, VERTICAL_KEY);
    return parseVertical(s?.value ?? null);
  } catch {
    return DEFAULT_VERTICAL;
  }
}

/** Load the operator's business display name, or `null` if unset. Used for
 *  branding instead of a hardcoded "Tu Farmacia". */
export async function loadBusinessName(serverUrl: string): Promise<string | null> {
  try {
    const s = await getSetting(serverUrl, BUSINESS_NAME_KEY);
    const v = (s?.value ?? "").trim();
    return v.length > 0 ? v : null;
  } catch {
    return null;
  }
}

// --- rubro catalog (onboarding "elige tu rubro") ----------------------------
//
// The onboarding grid (docs/strategy/rubro-catalog.md). The catalog LISTS every
// rubro from day one; the seed pack + vertical features are built per-rubro as
// each is validated with a real client (anti-premature-framework discipline).
//
// Naming (docs/strategy/rubro-catalog.md §naming): the seed-demo endpoint speaks
// English (`pharmacy`/`minimarket`). The client historically stored Spanish
// (`farmacia`) for the gated rubros, and other lanes (recetas/facturas) gate on
// those Spanish values via {@link parseVertical}. To avoid breaking that
// contract we KEEP the stored value as-is and only map es→en at the seed call
// (see `seedVertical`). Core rubros keep their Spanish stored value; extras use
// their catalog key (which `parseVertical` coerces to `otro` = generic ERP).

/** Seed pack a rubro maps to (en, for the seed-demo endpoint), or `null` when
 *  no demo pack exists yet for that rubro. The string values mirror
 *  `domain::seed::SeedVertical::parse` exactly. */
export type SeedVertical = "pharmacy" | "minimarket" | "cafe" | "tienda" | null;

/** One card of the onboarding rubro grid. */
export interface RubroCard {
  /** Value persisted as `business.vertical`. */
  value: string;
  /** Operator-facing label (Spanish). */
  label: string;
  /** Emoji icon for the card. */
  icon: string;
  /** One-line description shown under the label. */
  help: string;
  /** Short, emotional tagline shown in the live ERP preview header. Makes each
   *  rubro feel native ("hecho para mi negocio") — pure copy, no feature build. */
  tagline: string;
  /** Custom icon id (key into {@link RUBRO_ICONS}) — the self-hosted line-art
   *  shown on the showcase card + preview header. Distinct from {@link icon}:
   *  `icon` is the legacy emoji kept only for the login <option> dropdown. */
  iconId: string;
  /** Per-rubro accent (6-digit hex). Themes the card states (hover/selected
   *  border, icon, ✓) and the preview, so each rubro reads as its own identity
   *  instead of the single brand teal. Set as `--rubro-accent` on the card. */
  accent: string;
  /** Rubro-native value bullets ("Específico de tu rubro" in the live preview).
   *  Present-tense, in the rubro's own language, describing ONLY what the rubro
   *  actually turns on — must stay honest vs {@link featuresForRubro} (e.g. only
   *  farmacia claims recetas/Ley 20.000; service rubros say "sin inventario").
   *  Empty for the generic `otro`. Pure copy; building features is separate. */
  valueLines: string[];
  /** Roadmap items shown with grace as "Próximamente" — direction documented per
   *  rubro-catalog §Disciplina, NOT yet built. Never a dead-end: the rubro still
   *  gives a working ERP today; this only signals where it's headed. Empty for
   *  fully-built rubros (farmacia/minimarket) and the generic `otro`. */
  comingSoon: string[];
  /** Seed pack to request (es→en), or `null` if none yet. */
  seedVertical: SeedVertical;
}

/** Catalog v1 — taxonomy reused from the founder's DSS project + farmacia/
 *  minimarket. `✅` packs today: farmacia, minimarket, cafe, tienda; the rest
 *  seed nothing until their rubro is validated. */
export const RUBRO_CATALOG: readonly RubroCard[] = [
  {
    value: "feria",
    label: "Feria / Calle",
    icon: "🥬",
    help: "Puesto de feria o calle. Hablás y listo: sin código de barras ni impresora.",
    tagline: "Tu puesto, sin cuaderno.",
    iconId: "feria",
    accent: "#f59e0b",
    valueLines: [
      "Le hablás: «vendí tres kilos de tomate a dos mil»",
      "Fiado simple: quién te debe y cuánto",
      "Sirve sin señal: la venta se guarda y se manda después",
    ],
    comingSoon: ["Venta por peso en el celular", "Recordatorios de quién te debe"],
    seedVertical: null,
  },
  {
    value: "farmacia",
    label: "Farmacia",
    icon: "💊",
    help: "Recetas + libro de controlados (Ley 20.000), principio activo, lotes.",
    tagline: "Tu farmacia, en regla.",
    iconId: "farmacia",
    accent: "#1fd3a3",
    valueLines: [
      "Recetas retenidas y Libro de controlados (Ley 20.000)",
      "Ficha clínica: principio activo, laboratorio e interacciones",
      "Lotes y vencimiento con alertas de caducidad",
    ],
    comingSoon: [],
    seedVertical: "pharmacy",
  },
  {
    value: "minimarket",
    label: "Minimarket / Almacén",
    icon: "🛒",
    help: "Abarrotes y perecibles. POS, inventario y boletas. Sin recetas.",
    tagline: "Tu almacén, al día.",
    iconId: "minimarket",
    accent: "#f5b53d",
    valueLines: [
      "Lotes y vencimiento para perecibles, con alertas",
      "Proveedores y reposición por código de barras",
      "POS rápido para alto volumen de tickets",
    ],
    comingSoon: [],
    seedVertical: "minimarket",
  },
  {
    value: "restaurant",
    label: "Restaurant / Comida",
    icon: "🍽",
    help: "ERP genérico. Pack demo próximamente.",
    tagline: "Tu cocina, bajo control.",
    iconId: "restaurant",
    accent: "#ff6b5d",
    valueLines: [
      "Insumos y stock de cocina",
      "Boletas y facturas SII, sin ficha clínica",
    ],
    comingSoon: ["Comandas y gestión de mesas"],
    seedVertical: null,
  },
  {
    value: "cafe",
    label: "Café / Pastelería",
    icon: "☕",
    help: "Café, pastelería y sándwiches con lote y vencimiento. Datos demo incluidos.",
    tagline: "Tu café, listo cada mañana.",
    iconId: "cafe",
    accent: "#c98a4b",
    valueLines: [
      "Lotes y vencimiento de pastelería perecible",
      "POS rápido para el turno de la mañana",
    ],
    comingSoon: ["Producción y recetas de elaboración"],
    seedVertical: "cafe",
  },
  {
    value: "tienda",
    label: "Tienda / Retail",
    icon: "🛍",
    help: "Vestuario, librería y electrónica menor. POS e inventario. Datos demo incluidos.",
    tagline: "Tu tienda, ordenada.",
    iconId: "tienda",
    accent: "#5aa9ff",
    valueLines: [
      "POS e inventario por producto",
      "Compras y costo promedio ponderado",
    ],
    comingSoon: ["Variantes y tallas (un producto, varios SKU)"],
    seedVertical: "tienda",
  },
  {
    value: "belleza",
    label: "Belleza / Estética",
    icon: "💅",
    help: "Servicios y agenda; poco stock físico.",
    tagline: "Tu salón, agendado.",
    iconId: "belleza",
    accent: "#e879c7",
    valueLines: [
      "Venta de servicios sin inventario ni lotes",
      "Boletas y facturas SII por cada servicio",
    ],
    comingSoon: ["Agenda de horas y profesionales"],
    seedVertical: null,
  },
  {
    value: "servicios",
    label: "Servicios / Oficios",
    icon: "🔧",
    help: "Ventas de servicios sin inventario físico.",
    tagline: "Tu oficio, facturado.",
    iconId: "servicios",
    accent: "#94a3c4",
    valueLines: [
      "Servicios sin inventario: cobrás por el trabajo, no por stock",
      "Boletas y facturas SII por trabajo realizado",
    ],
    comingSoon: ["Órdenes de trabajo y presupuestos"],
    seedVertical: null,
  },
  {
    value: "otro",
    label: "Otro",
    icon: "➕",
    help: "ERP genérico, sin secciones específicas de un rubro.",
    tagline: "Tu negocio, a tu manera.",
    iconId: "otro",
    accent: "#8b97ad",
    valueLines: [],
    comingSoon: [],
    seedVertical: null,
  },
] as const;

/** The seed pack (en) for a stored rubro value, or `null` when none exists. */
export function seedVerticalFor(value: string | null | undefined): SeedVertical {
  return RUBRO_CATALOG.find((r) => r.value === (value ?? "").trim())?.seedVertical ?? null;
}

/** The catalog card for a stored rubro value, or `undefined` when unknown. Lets
 *  the preview pull a rubro's native copy/icon/accent from the single source. */
export function rubroCard(value: string | null | undefined): RubroCard | undefined {
  return RUBRO_CATALOG.find((r) => r.value === (value ?? "").trim());
}

// --- per-rubro feature gating ------------------------------------------------
//
// `Vertical` (above) is the COARSE signal: it collapses every catalog extra to
// `otro`, so it cannot tell a café (perishables → lotes/vencimiento) apart from
// a peluquería (a service → no physical stock). The onboarding grid persists the
// full 8-rubro catalog key, so the gate must key off THAT, not the coarse trio.
//
// `Rubro` is the full catalog key; `featuresForRubro` is the single source of
// truth mapping a rubro to the capabilities its UI turns on. Every lane that
// shows/hides a vertical-specific module routes through here so the rules live in
// exactly one tested place (docs/strategy/rubro-catalog.md §"features gated").

/** Full rubro key — every value in {@link RUBRO_CATALOG}. Distinct from the
 *  coarse {@link Vertical}, which collapses extras to `otro`. */
export type Rubro =
  | "feria"
  | "farmacia"
  | "minimarket"
  | "restaurant"
  | "cafe"
  | "tienda"
  | "belleza"
  | "servicios"
  | "otro";

/** Default rubro when unset/unknown — generic ERP, never farmacia (pivot rule). */
export const DEFAULT_RUBRO: Rubro = "otro";

/** Coerce an arbitrary stored string into a known {@link Rubro}, defaulting to
 *  [`DEFAULT_RUBRO`] for unset/unknown values. Unlike {@link parseVertical} this
 *  preserves catalog extras (cafe/belleza/…) instead of folding them to `otro`. */
export function parseRubro(raw: string | null | undefined): Rubro {
  const v = (raw ?? "").trim().toLowerCase();
  return (RUBRO_CATALOG.find((r) => r.value === v)?.value as Rubro | undefined) ?? DEFAULT_RUBRO;
}

/** Capabilities a rubro turns on. Pure data — the UI gate reads these flags so
 *  the per-rubro rules are defined once and unit-tested. */
export interface RubroFeatures {
  /** Recetas + libro de controlados (Ley 20.000). Farmacia only. */
  recetas: boolean;
  /** Lotes + vencimiento (perecibles). Drugs, abarrotes and pastelería track
   *  expiry; a retail/service rubro does not. */
  lotes: boolean;
  /** Tracks physical stock at all (inventario + compras). Service rubros
   *  (belleza, servicios) sell without inventory — the core stays agnóstico. */
  physicalStock: boolean;
  /** Clinical product fields (principio activo, laboratorio, interacciones).
   *  Farmacia only. */
  clinical: boolean;
  /** Agent chat is the primary home (feria). */
  agentHome: boolean;
  /** Barcode / camera scanner in POS. */
  barcode: boolean;
  /** Thermal printer flows. */
  printer: boolean;
  /** Electronic invoice / DTE / SII surfaces. */
  dte: boolean;
  /** Informal day-1 (no RUT empresa / no SII). */
  informalOk: boolean;
}

const FORMAL_RETAIL = {
  agentHome: false,
  barcode: true,
  printer: true,
  dte: true,
  informalOk: false,
} as const;

// Per-rubro flags. Kept literal (not derived) so the table reads like the spec.
const RUBRO_FEATURES: Readonly<Record<Rubro, RubroFeatures>> = {
  feria: {
    recetas: false,
    lotes: false,
    physicalStock: true,
    clinical: false,
    agentHome: true,
    barcode: false,
    printer: false,
    dte: false,
    informalOk: true,
  },
  farmacia: { recetas: true, lotes: true, physicalStock: true, clinical: true, ...FORMAL_RETAIL },
  minimarket: { recetas: false, lotes: true, physicalStock: true, clinical: false, ...FORMAL_RETAIL },
  cafe: { recetas: false, lotes: true, physicalStock: true, clinical: false, ...FORMAL_RETAIL },
  restaurant: { recetas: false, lotes: false, physicalStock: true, clinical: false, ...FORMAL_RETAIL },
  tienda: { recetas: false, lotes: false, physicalStock: true, clinical: false, ...FORMAL_RETAIL },
  otro: { recetas: false, lotes: false, physicalStock: true, clinical: false, ...FORMAL_RETAIL },
  belleza: {
    recetas: false,
    lotes: false,
    physicalStock: false,
    clinical: false,
    ...FORMAL_RETAIL,
  },
  servicios: {
    recetas: false,
    lotes: false,
    physicalStock: false,
    clinical: false,
    ...FORMAL_RETAIL,
  },
};

/** The capability flags for a rubro (or any stored string, coerced). Single
 *  source of truth for per-rubro UI gating. */
export function featuresForRubro(rubro: Rubro | string | null | undefined): RubroFeatures {
  return RUBRO_FEATURES[parseRubro(rubro as string)];
}

/** Load the persisted rubro (full catalog key) from the server, defaulting on
 *  any miss. Never throws — onboarding must not dead-end on a settings hiccup. */
export async function loadRubro(serverUrl: string): Promise<Rubro> {
  try {
    const s = await getSetting(serverUrl, VERTICAL_KEY);
    return parseRubro(s?.value ?? null);
  } catch {
    return DEFAULT_RUBRO;
  }
}

// --- server rubro pack (source of truth) ------------------------------------
//
// Since P0.3 the server serves the declarative pack (`GET /api/v1/rubro-pack`,
// `domain::rubro`). The client consumes it with the LOCAL constants above as
// an offline fallback: a LAN hiccup or an old server must never break gating.

import { rubroPack, type RubroPack, type PackFeatures, type PackVocab, type PackAttrField } from "./api";

let packCache: RubroPack | null = null;

/**
 * Offline attr catalog (mirrors `domain::rubro` packs). Used when the server
 * pack is unreachable so the product form still shows talla/color/sku etc.
 * Keep in sync with crates/domain/src/rubro.rs — C owns the client mirror.
 */
export function localAttrsForRubro(rubro: string | null | undefined): PackAttrField[] {
  switch ((rubro ?? "").trim().toLowerCase()) {
    case "farmacia":
      return [
        { key: "active_ingredient", label: "Principio activo", kind: "text" },
        { key: "laboratory", label: "Laboratorio", kind: "text" },
        { key: "therapeutic_action", label: "Acción terapéutica", kind: "text" },
      ];
    case "tienda":
      return [
        { key: "talla", label: "Talla", kind: "text" },
        { key: "color", label: "Color", kind: "text" },
        { key: "sku", label: "SKU", kind: "text" },
      ];
    case "belleza":
      return [{ key: "duracion_min", label: "Duración (min)", kind: "number" }];
    case "feria":
      return [
        { key: "unidad", label: "Se vende por", kind: "text" },
        { key: "precio_ref", label: "Precio de referencia", kind: "money" },
      ];
    default:
      return [];
  }
}

/** Build a pack-shaped object from the local constants (offline fallback).
 *  Attrs mirror domain::rubro so the product form still shows talla/color/sku
 *  when the LAN is down or the server is older than P0.3. */
function localPack(rubro: Rubro): RubroPack {
  const card = rubroCard(rubro);
  const f = featuresForRubro(rubro);
  return {
    rubro,
    label: card?.label ?? "Otro",
    tagline: card?.tagline ?? "Tu negocio, a tu manera.",
    accent: card?.accent ?? "#8b97ad",
    features: {
      recetas: f.recetas,
      lotes: f.lotes,
      physical_stock: f.physicalStock,
      clinical: f.clinical,
      agent_home: f.agentHome,
      barcode: f.barcode,
      printer: f.printer,
      dte: f.dte,
      informal_ok: f.informalOk,
    },
    vocab: {
      item: card?.value === "feria" ? "Cosa" : f.physicalStock ? "Producto" : "Servicio",
      catalog: card?.value === "feria" ? "Lo que vendo" : "Inventario",
    },
    attrs: localAttrsForRubro(rubro),
    seed_vertical: card?.seedVertical ?? null,
    coming_soon: card?.comingSoon ?? [],
  };
}

/** Load the tenant's rubro pack from the server and cache it. On any failure
 *  (offline LAN, old server without the route) returns the locally-built pack
 *  — never throws, gating must not dead-end. Call once after login (shell). */
export async function loadRubroPack(serverUrl: string): Promise<RubroPack> {
  try {
    packCache = await rubroPack(serverUrl);
  } catch {
    packCache = localPack(await loadRubro(serverUrl));
  }
  return packCache;
}

/** Drop the cached pack (logout / tenant switch). Next login reloads. */
export function clearPackCache(): void {
  packCache = null;
}

/** The cached pack, or `null` before {@link loadRubroPack} runs. */
export function cachedPack(): RubroPack | null {
  return packCache;
}

/** Capability flags from a server pack, in the client's `RubroFeatures` shape
 *  (snake_case on the wire → camelCase here). */
export function featuresFromPack(p: PackFeatures): RubroFeatures {
  return {
    recetas: p.recetas,
    lotes: p.lotes,
    physicalStock: p.physical_stock,
    clinical: p.clinical,
    agentHome: p.agent_home ?? false,
    barcode: p.barcode ?? true,
    printer: p.printer ?? true,
    dte: p.dte ?? true,
    informalOk: p.informal_ok ?? false,
  };
}

/** Live gating flags after login: prefer the server pack cache; fall back to
 *  local constants for the given rubro (or generic `otro`). Use this in shell
 *  views so a pack reload can change the UI without editing every call site. */
export function activeFeatures(fallbackRubro?: string | null): RubroFeatures {
  if (packCache) return featuresFromPack(packCache.features);
  return featuresForRubro(fallbackRubro);
}

/** Operator-facing vocabulary (item/catalog labels) after login. Prefer the
 *  cached server pack; offline or pre-login falls back to the same local
 *  defaults as {@link localPack} so UI never hardcodes "Producto" when the
 *  pack says "Servicio" / "Plato". */
export function activeVocab(fallbackRubro?: string | null): PackVocab {
  if (packCache) return packCache.vocab;
  const f = featuresForRubro(fallbackRubro);
  return {
    item: f.physicalStock ? "Producto" : "Servicio",
    catalog: "Inventario",
  };
}

/** Ensure the pack is loaded, then return its features. Never throws. */
export async function loadFeatures(serverUrl: string): Promise<RubroFeatures> {
  const pack = await loadRubroPack(serverUrl);
  return featuresFromPack(pack.features);
}
