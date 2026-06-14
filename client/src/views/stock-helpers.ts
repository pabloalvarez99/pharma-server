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

// --- CSV / JSON export (vendor-agnostic: the owner owns their data) ---------

/** Escape one value as an RFC-4180 CSV field: wrap in double quotes and double
 *  any embedded quote when the value contains a comma, quote, CR or LF; otherwise
 *  pass it through. `null`/`undefined` → "". Numbers/booleans are stringified.
 *  CSV-injection guard: a value starting with `= + - @` is prefixed with a tab so
 *  a spreadsheet treats it as text, not a formula (product names are operator
 *  input → an injection vector). */
export function csvField(value: unknown): string {
  if (value == null) return "";
  let s = String(value);
  if (/^[=+\-@]/.test(s)) s = `\t${s}`;
  if (/[",\r\n]/.test(s)) s = `"${s.replace(/"/g, '""')}"`;
  return s;
}

/** Join a header row + data rows into a CSV string (CRLF line endings per
 *  RFC-4180, Excel-friendly). No BOM here — the caller prepends `﻿` for the
 *  download so the in-memory string stays clean for tests / round-trips. */
export function toCsv(
  header: readonly string[],
  rows: readonly (readonly unknown[])[],
): string {
  return [header, ...rows].map((r) => r.map(csvField).join(",")).join("\r\n");
}

/** The subset of a product needed to export it (a superset of `api.Product`). */
export interface ExportProduct {
  id: string;
  name: string;
  price: string;
  stock: number;
  active: boolean;
  laboratory?: string | null;
  active_ingredient?: string | null;
}

/** A built export ready to download: the CSV + pretty JSON strings, the row
 *  `count`, and `truncated` = the fetch hit its page cap (more data exists). */
export interface ExportBundle {
  csv: string;
  json: string;
  count: number;
  truncated: boolean;
}

/** Build the inventory export (CSV + pretty JSON) for a product list.
 *  Multi-rubro: `includePharma` adds the pharmacy-only columns (laboratorio,
 *  principio activo) — a minimarket / general vertical omits them so the file
 *  carries only the fields that rubro uses. `cap`, when given, is the page size
 *  the fetch was limited to; a list that fills it exactly is flagged `truncated`
 *  (the server caps a single product page). CSV headers are Spanish (operator);
 *  the JSON keeps stable snake_case keys (machine round-trip). Money stays the
 *  raw Decimal STRING — no locale formatting → re-imports losslessly. */
export function buildInventoryExport(
  products: readonly ExportProduct[],
  includePharma: boolean,
  cap?: number,
): ExportBundle {
  const header = ["id", "nombre", "precio", "stock", "activo"];
  if (includePharma) header.push("laboratorio", "principio_activo");
  const rows = products.map((p) => {
    const row: unknown[] = [p.id, p.name, p.price, p.stock, p.active ? "sí" : "no"];
    if (includePharma) row.push(p.laboratory ?? "", p.active_ingredient ?? "");
    return row;
  });
  const json = JSON.stringify(
    products.map((p) => {
      const base: Record<string, unknown> = {
        id: p.id,
        name: p.name,
        price: p.price,
        stock: p.stock,
        active: p.active,
      };
      if (includePharma) {
        base.laboratory = p.laboratory ?? null;
        base.active_ingredient = p.active_ingredient ?? null;
      }
      return base;
    }),
    null,
    2,
  );
  return {
    csv: toCsv(header, rows),
    json,
    count: products.length,
    truncated: cap != null && products.length >= cap,
  };
}

/** Filename stem for an export, e.g. `inventario-2026-06-14`, in the operator's
 *  local (CL) calendar day. The caller appends the extension. */
export function exportFilename(prefix: string, now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${prefix}-${y}-${m}-${d}`;
}

// --- empty / error states (never a blank screen) ----------------------------

/** Operator-facing copy for an empty list. `filtered` ⇒ a search/filter is
 *  active, so it's "no matches" (the data may still be there) not "nothing yet".
 *  `cta`, when present, labels the action that resolves the empty — the view
 *  wires it to a button so the operator always has a way forward. */
export interface EmptyCopy {
  title: string;
  hint: string;
  cta?: string;
}

export function inventoryEmpty(filtered: boolean): EmptyCopy {
  return filtered
    ? {
        title: "Sin coincidencias",
        hint: "Ningún producto coincide con tu búsqueda. Prueba con otro término.",
      }
    : {
        title: "Aún no hay productos",
        hint: "Crea tu primer producto para controlar stock, lotes y vencimientos.",
        cta: "+ Nuevo producto",
      };
}

export function comprasEmpty(filtered: boolean): EmptyCopy {
  return filtered
    ? {
        title: "Sin órdenes para ese filtro",
        hint: "Ninguna orden de compra coincide con el estado seleccionado.",
      }
    : {
        title: "Aún no hay órdenes de compra",
        hint: "Crea una orden para registrar lo que pides a tus proveedores.",
        cta: "+ Nueva OC",
      };
}

export function gastosEmpty(filtered: boolean): EmptyCopy {
  return filtered
    ? {
        title: "Sin gastos para ese filtro",
        hint: "Ningún gasto coincide con la categoría seleccionada.",
      }
    : {
        title: "Aún no hay gastos",
        hint: "Registra tus egresos y caja chica para llevar el control del día.",
        cta: "Nuevo gasto",
      };
}

/** What kind of failure a fetch hit, for operator-facing copy. */
export type FetchErrorKind = "forbidden" | "offline" | "generic";

export interface ErrorCopy {
  kind: FetchErrorKind;
  title: string;
  hint: string;
}

/** Classify a fetch failure (the api layer rejects with a Spanish string) into
 *  operator copy. A permission problem (403 / "denegado" / "permiso") → "sin
 *  acceso"; a connection / network failure (the Tauri `conn_error` text, or a
 *  timeout) → "sin conexión" with a retry hint; anything else → the raw message
 *  so no real error is swallowed. `resource` customizes the forbidden line
 *  ("a las compras"). Never throws — safe to call in any catch. */
export function classifyFetchError(err: unknown, resource = "esta sección"): ErrorCopy {
  const msg = typeof err === "string" ? err : "";
  const low = msg.toLowerCase();
  if (low.includes("403") || low.includes("denegado") || low.includes("permiso")) {
    return {
      kind: "forbidden",
      title: "Sin acceso",
      hint: `Tu rol no tiene permiso para ver ${resource}. Contacta al administrador.`,
    };
  }
  if (
    low.includes("no se pudo conectar") ||
    low.includes("error de red") ||
    low.includes("conexión") ||
    low.includes("conexion") ||
    low.includes("timeout") ||
    low.includes("timed out")
  ) {
    return {
      kind: "offline",
      title: "Sin conexión al servidor",
      hint: "No se pudo conectar a pharma-server. Verifica que esté corriendo e inténtalo de nuevo.",
    };
  }
  return {
    kind: "generic",
    title: "No se pudo cargar",
    hint: msg || "Ocurrió un error al cargar la información. Inténtalo de nuevo.",
  };
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
