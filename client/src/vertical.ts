// Business vertical (rubro) — the core signal of the freemium multi-rubro pivot.
//
// pharma-server is sold as a generic on-prem ERP; "farmacia" is the beachhead,
// not the only vertical. The operator picks their rubro in Configuración and it
// is persisted as the `business.vertical` admin_setting. The UI reads it to
// show/hide vertical-specific sections so a minimarket never sees the
// pharmacy-only Recetas/controlados (Ley 20.000) module.
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

/** DTE/boleta/factura is universal in Chile — every rubro emits. Kept as an
 *  explicit predicate so callers read intent rather than assuming. */
export function hasDte(_v: Vertical): boolean {
  return true;
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
 *  no demo pack exists yet for that rubro. */
export type SeedVertical = "pharmacy" | "minimarket" | null;

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
  /** Seed pack to request (es→en), or `null` if none yet. */
  seedVertical: SeedVertical;
}

/** Catalog v1 — taxonomy reused from the founder's DSS project + farmacia/
 *  minimarket. `✅` packs today: farmacia, minimarket; the rest seed nothing
 *  until their rubro is validated. */
export const RUBRO_CATALOG: readonly RubroCard[] = [
  {
    value: "farmacia",
    label: "Farmacia",
    icon: "💊",
    help: "Recetas + libro de controlados (Ley 20.000), principio activo, lotes.",
    seedVertical: "pharmacy",
  },
  {
    value: "minimarket",
    label: "Minimarket / Almacén",
    icon: "🛒",
    help: "Abarrotes y perecibles. POS, inventario y boletas. Sin recetas.",
    seedVertical: "minimarket",
  },
  {
    value: "restaurant",
    label: "Restaurant / Comida",
    icon: "🍽",
    help: "ERP genérico. Pack demo próximamente.",
    seedVertical: null,
  },
  {
    value: "cafe",
    label: "Café / Pastelería",
    icon: "☕",
    help: "ERP genérico. Pack demo próximamente.",
    seedVertical: null,
  },
  {
    value: "tienda",
    label: "Tienda / Retail",
    icon: "🛍",
    help: "ERP genérico. POS e inventario.",
    seedVertical: null,
  },
  {
    value: "belleza",
    label: "Belleza / Estética",
    icon: "💅",
    help: "Servicios y agenda; poco stock físico.",
    seedVertical: null,
  },
  {
    value: "servicios",
    label: "Servicios / Oficios",
    icon: "🔧",
    help: "Ventas de servicios sin inventario físico.",
    seedVertical: null,
  },
  {
    value: "otro",
    label: "Otro",
    icon: "➕",
    help: "ERP genérico, sin secciones específicas de un rubro.",
    seedVertical: null,
  },
] as const;

/** The seed pack (en) for a stored rubro value, or `null` when none exists. */
export function seedVerticalFor(value: string | null | undefined): SeedVertical {
  return RUBRO_CATALOG.find((r) => r.value === (value ?? "").trim())?.seedVertical ?? null;
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
}

// Per-rubro flags. Kept literal (not derived) so the table reads like the spec.
// Rubros without their own row fall back to the generic ERP profile below.
const RUBRO_FEATURES: Readonly<Record<Rubro, RubroFeatures>> = {
  farmacia: { recetas: true, lotes: true, physicalStock: true, clinical: true },
  minimarket: { recetas: false, lotes: true, physicalStock: true, clinical: false },
  cafe: { recetas: false, lotes: true, physicalStock: true, clinical: false },
  restaurant: { recetas: false, lotes: false, physicalStock: true, clinical: false },
  tienda: { recetas: false, lotes: false, physicalStock: true, clinical: false },
  otro: { recetas: false, lotes: false, physicalStock: true, clinical: false },
  // Service rubros: sales without physical stock — exercises the agnostic core.
  belleza: { recetas: false, lotes: false, physicalStock: false, clinical: false },
  servicios: { recetas: false, lotes: false, physicalStock: false, clinical: false },
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
