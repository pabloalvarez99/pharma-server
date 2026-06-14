// Pure helpers for the stock / money-out views (inventory · compras · gastos).
// NO DOM, NO Tauri imports — kept import-free on purpose so they unit-test under
// vitest's node env (like format.ts) and stay shared without coupling the views.

/** A `<input type="date">` value (`YYYY-MM-DD`) → RFC3339 the server parses as
 *  `DateTime<Utc>`, anchored at **noon UTC**. Noon (not midnight) so the stored
 *  instant never crosses a day boundary when re-rendered in Chile's local TZ
 *  (UTC-3/-4): `2026-05-01T00:00:00Z` shows as 30-04-2026 in es-CL, but
 *  `2026-05-01T12:00:00Z` stays 01-05-2026. Empty input ⇒ undefined (the server
 *  defaults to "now"). Mirrors the expiry/incurred-at date contract. */
export function toRfc3339Noon(dateInput: string): string | undefined {
  const v = dateInput.trim();
  if (!v) return undefined;
  return `${v}T12:00:00Z`;
}

/** Stock level bucket for the inventory pill. `lowThreshold` is the inclusive
 *  upper bound for "low" (≤ threshold but > 0). Defaults to 5 (the legacy UI
 *  constant). A non-positive stock is always "out". */
export type StockLevel = "out" | "low" | "ok";
export function stockLevel(stock: number, lowThreshold = 5): StockLevel {
  if (!(stock > 0)) return "out"; // also catches NaN / negatives
  return stock <= lowThreshold ? "low" : "ok";
}

/** Advisory expiry verdict for a lote, computed on whole-day boundaries so it
 *  matches the server's `days_to_expiry` (floor of calendar days from `now` to
 *  the expiry date, both at UTC day granularity). `< 0` ⇒ already expired. */
export interface ExpiryStatus {
  days: number;
  expired: boolean;
  tone: "danger" | "warn" | "ok" | "muted";
  label: string;
}
export function expiryStatus(iso: string, now: Date = new Date()): ExpiryStatus {
  const exp = new Date(iso);
  if (Number.isNaN(exp.getTime())) {
    return { days: NaN, expired: false, tone: "muted", label: "—" };
  }
  // Truncate both ends to the UTC calendar day so a same-day expiry reads 0, not
  // a fractional ±1 depending on the time component.
  const DAY = 86_400_000;
  const expDay = Math.floor(exp.getTime() / DAY);
  const nowDay = Math.floor(now.getTime() / DAY);
  const days = expDay - nowDay;
  if (days < 0) return { days, expired: true, tone: "danger", label: "Caducado" };
  if (days <= 30) return { days, expired: false, tone: "warn", label: "Por vencer" };
  return { days, expired: false, tone: "ok", label: "Vigente" };
}

/** Whether the pharmacy-only product fields (laboratorio, principio activo,
 *  tipo de receta) should be shown for a given `business_vertical` admin setting.
 *  Multi-rubro: a minimarket / general store doesn't use them, so they're hidden
 *  there. Anything unknown/unset (incl. a `getSetting` 403/null) defaults to
 *  TRUE — pharmacy is the historical default and the safe back-compat choice. */
export function pharmaFieldsVisible(vertical: string | null | undefined): boolean {
  if (vertical == null) return true;
  const v = vertical.trim().toLowerCase();
  if (v === "") return true;
  // Explicit non-pharmacy verticals hide the clinical fields.
  const nonPharma = new Set([
    "general",
    "minimarket",
    "market",
    "almacen",
    "almacén",
    "retail",
    "otro",
    "other",
  ]);
  return !nonPharma.has(v);
}
