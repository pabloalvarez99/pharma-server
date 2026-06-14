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
