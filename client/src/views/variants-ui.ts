// Professional multi-SKU variants UX helpers (pure, no DOM).
//
// Server (B): product parent + children with parent_id, GET by-barcode,
// GET/POST .../variants. Full talla×color matrix grid is still out of scope —
// inventory shows banner + table + barcode-first "agregar variante" modal;
// POS scans child barcodes and refuses selling the parent.
//
// Multi-rubro honesty: variants UI is for physical-stock rubros (tienda, etc.).
// Service rubros (belleza) never offer multi-SKU shell products.

import type { PackAttrField } from "../api/rubro";
import type { NewVariantInput } from "../api/catalog";

// ---------------------------------------------------------------------------
// Copy (español de negocio)
// ---------------------------------------------------------------------------

/** Operator banner when a catalog product is a multi-SKU parent. */
export function variantsParentBanner(count: number): string {
  const n = Math.max(0, Math.trunc(count));
  if (n <= 0) return "";
  const label = n === 1 ? "1 variante" : `${n} variantes`;
  return `Tiene ${label} · vender por código de barras del hijo (talla/SKU). No cobres el padre en el POS.`;
}

/** Section title for product detail variants block. */
export function variantsSectionTitle(count: number): string {
  const n = Math.max(0, Math.trunc(count));
  if (n <= 0) return "Variantes (multi-SKU)";
  return n === 1 ? "1 variante (multi-SKU)" : `${n} variantes (multi-SKU)`;
}

/** Error when the cashier tries to ring the parent instead of a child SKU. */
export function parentWithVariantsError(productName: string): string {
  const name = (productName || "este producto").trim() || "este producto";
  return `«${name}» tiene variantes. Escanea el código de barras de la talla/SKU o elige la variante.`;
}

/** True when a sale-error message is the domain parent-has-variants guard. */
export function isParentHasVariantsMessage(msg: string): boolean {
  const low = (msg || "").toLowerCase();
  return low.includes("tiene variantes") || low.includes("escanee el código");
}

/** Stock column for a parent in detail KPIs. */
export function parentStockLabel(kids: readonly { stock?: number }[]): string {
  if (!kids.length) return "En variantes";
  const sum = sumVariantStock(kids);
  return `En variantes · ${sum} u.`;
}

/** Sum child stock (parent shell stock is not sellable). */
export function sumVariantStock(kids: readonly { stock?: number }[]): number {
  let n = 0;
  for (const k of kids) {
    const s = k.stock;
    if (typeof s === "number" && Number.isFinite(s)) n += Math.trunc(s);
  }
  return n;
}

/** Compact badge for list/card when variant count is known. */
export function variantsListBadge(count: number): string {
  const n = Math.max(0, Math.trunc(count));
  if (n <= 0) return "";
  return n === 1 ? "1 variante" : `${n} variantes`;
}

/** Child row note under the product name in detail. */
export function variantChildNote(): string {
  return "Variante multi-SKU · vender por su código de barras";
}

/** CTA on detail to open the create-variant modal. */
export function addVariantButtonLabel(): string {
  return "+ Agregar variante";
}

/** Modal title for create-variant. */
export function addVariantModalTitle(parentName: string): string {
  const name = (parentName || "producto").trim() || "producto";
  return `Nueva variante · ${name}`;
}

/** Hint under the has-variants toggle on the alta form. */
export function hasVariantsToggleHint(): string {
  return "El stock y el código de barras viven en cada talla/SKU. Después de crear el padre, agrega las variantes desde el detalle.";
}

/** Label for the has-variants checkbox on nuevo producto. */
export function hasVariantsToggleLabel(): string {
  return "Este producto tiene variantes (tallas, colores u otros SKU)";
}

/** Toast after creating a parent with the variants toggle on. */
export function parentCreatedOpenVariantsToast(productName: string): string {
  const name = (productName || "Producto").trim() || "Producto";
  return `«${name}» creado como padre multi-SKU. Abre el detalle para agregar variantes con código de barras.`;
}

/** Empty-state when parent has no children yet. */
export function variantsEmptyHint(): string {
  return "Aún no hay variantes. Agrega la primera con código de barras (talla/color/SKU).";
}

/** POS / inventory stock-0 on a plain product (not parent). */
export function plainOutOfStockError(productName: string): string {
  const name = (productName || "este producto").trim() || "este producto";
  return `«${name}» está sin stock.`;
}

// ---------------------------------------------------------------------------
// Multi-rubro honesty
// ---------------------------------------------------------------------------

/**
 * Variants multi-SKU only make sense when the rubro tracks physical stock.
 * Service rubros must not show toggle/modal (no inventory to split by talla).
 */
export function shouldOfferVariantsUi(physicalStock: boolean): boolean {
  return Boolean(physicalStock);
}

/**
 * Pack attrs that act as variant discriminators (talla/color/sku…).
 * Clinical keys and free-form service attrs are excluded.
 */
export function variantAttrFieldsFromPack(
  attrFields: readonly PackAttrField[] | null | undefined,
): PackAttrField[] {
  const list = attrFields ?? [];
  return list
    .filter((f) => isVariantAttrKey(f.key))
    .map((f) => ({ ...f }));
}

/** Prefer known retail discriminators; also accept pack keys that look like SKU dims. */
export function isVariantAttrKey(key: string): boolean {
  const k = (key || "").trim().toLowerCase();
  if (!k) return false;
  if (
    k === "laboratory" ||
    k === "active_ingredient" ||
    k === "therapeutic_action" ||
    k === "principio_activo" ||
    k === "laboratorio" ||
    k === "presentation" ||
    k === "presentacion" ||
    k === "duracion_min" ||
    k === "duration_min"
  ) {
    return false;
  }
  // Canonical retail + common Spanish pack keys.
  if (
    k === "talla" ||
    k === "color" ||
    k === "sku" ||
    k === "size" ||
    k === "talle" ||
    k === "modelo" ||
    k === "sabor" ||
    k === "capacidad" ||
    k === "gramaje"
  ) {
    return true;
  }
  // Unknown non-clinical pack keys can still discriminate a SKU (e.g. "material").
  return true;
}

/** Default fields when pack has no attrs (offline / farmacia pack). */
export function defaultVariantAttrFields(): PackAttrField[] {
  return [
    { key: "talla", label: "Talla", kind: "text" },
    { key: "color", label: "Color", kind: "text" },
    { key: "sku", label: "SKU interno", kind: "text" },
  ];
}

/**
 * Fields to render in the add-variant modal.
 * Uses pack variant attrs when present; otherwise the default talla/color/sku trio
 * so tienda works even if the pack list is empty offline.
 */
export function variantFormAttrFields(
  packAttrs: readonly PackAttrField[] | null | undefined,
): PackAttrField[] {
  const fromPack = variantAttrFieldsFromPack(packAttrs);
  if (fromPack.length > 0) return fromPack;
  return defaultVariantAttrFields();
}

// ---------------------------------------------------------------------------
// Attrs / display
// ---------------------------------------------------------------------------

/** Short attrs label for a variant row (talla M · color Negro). */
export function variantAttrsLabel(attrs: Record<string, unknown> | null | undefined): string {
  if (!attrs || typeof attrs !== "object") return "";
  const parts: string[] = [];
  for (const key of ["talla", "color", "sku", "size", "talle", "modelo", "sabor"]) {
    const v = (attrs as Record<string, unknown>)[key];
    if (v != null && String(v).trim() !== "") parts.push(String(v).trim());
  }
  if (parts.length === 0) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || String(v).trim() === "") continue;
      if (!isVariantAttrKey(k)) continue;
      parts.push(`${k}: ${String(v).trim()}`);
      if (parts.length >= 3) break;
    }
  }
  return parts.join(" · ");
}

/**
 * Default display name for a child when the operator leaves name blank.
 * Server also defaults; client mirrors for preview / optimistic UI.
 */
export function defaultVariantName(
  parentName: string,
  attrs: Record<string, string> | null | undefined,
): string {
  const parent = (parentName || "Producto").trim() || "Producto";
  const lab = variantAttrsLabel(attrs ?? {});
  if (!lab) return parent;
  return `${parent} — ${lab}`;
}

/** One pure row model for the detail variants table (no HTML). */
export interface VariantTableRow {
  id: string;
  name: string;
  attrsLabel: string;
  barcode: string;
  price: string;
  stock: number;
  active: boolean;
}

/**
 * Map a product-detail-ish child to a table row.
 * Barcode may live on attrs.sku or a dedicated field when the DTO exposes it later.
 */
export function toVariantTableRow(v: {
  id: string;
  name: string;
  price: string;
  stock: number;
  active?: boolean;
  attrs?: Record<string, unknown> | null;
  /** When API starts returning barcode on ProductDto. */
  barcode?: string | null;
}): VariantTableRow {
  const attrs = (v.attrs ?? null) as Record<string, unknown> | null;
  const fromAttrs =
    attrs && typeof attrs === "object"
      ? String((attrs as Record<string, unknown>).barcode ?? "").trim()
      : "";
  const barcode = (v.barcode ?? "").trim() || fromAttrs || "—";
  return {
    id: v.id,
    name: v.name,
    attrsLabel: variantAttrsLabel(attrs),
    barcode,
    price: v.price,
    stock: typeof v.stock === "number" ? v.stock : 0,
    active: v.active !== false,
  };
}

// ---------------------------------------------------------------------------
// Barcode / POS scan intent
// ---------------------------------------------------------------------------

/**
 * Decide POS Enter path intent: prefer barcode lookup when the typed value
 * looks like a scan (mostly digits, length ≥ 4) rather than a name search hit.
 * Name searches still work via the existing first-result path when this is false
 * OR when by-barcode 404s.
 */
export function preferBarcodeLookup(raw: string): boolean {
  const s = (raw ?? "").trim();
  if (s.length < 4) return false;
  // Pure digits (EAN-8/13/UPC) or digit-heavy codes with few separators.
  const digits = s.replace(/\D/g, "");
  if (digits.length >= 8 && digits.length >= s.length - 2) return true;
  // Explicit barcode-ish: long alphanumeric without spaces (internal SKUs).
  if (s.length >= 6 && !/\s/.test(s) && /^[A-Za-z0-9\-_.]+$/.test(s)) return true;
  return false;
}

/** POS search placeholder mentioning barcode for physical rubros. */
export function posVariantsSearchHint(itemWord: string): string {
  const w = (itemWord || "producto").toLowerCase();
  return `Buscar ${w} o escanear código de barras (Enter)…`;
}

// ---------------------------------------------------------------------------
// Create-variant form model (barcode-first)
// ---------------------------------------------------------------------------

export interface NewVariantRaw {
  /** Required for POS scan path — primary field. */
  barcode: string;
  name?: string;
  price?: string;
  costPrice?: string;
  stock?: string;
  attrs?: Record<string, string>;
}

export type NewVariantBuildResult =
  | { ok: true; value: NewVariantInput }
  | { ok: false; error: string };

/**
 * Validate + build POST body for create variant.
 * Barcode-first: empty barcode is rejected (business rule for retail POS).
 * Money as STRING; stock optional ≥ 0.
 */
export function buildNewVariantInput(raw: NewVariantRaw): NewVariantBuildResult {
  const barcode = (raw.barcode ?? "").trim();
  if (barcode === "") {
    return {
      ok: false,
      error: "Ingresa el código de barras de la variante (escáner o teclado).",
    };
  }
  if (barcode.length < 3) {
    return {
      ok: false,
      error: "El código de barras es demasiado corto (mín. 3 caracteres).",
    };
  }
  if (/\s/.test(barcode)) {
    return {
      ok: false,
      error: "El código de barras no debe tener espacios.",
    };
  }

  let price: string | undefined;
  if ((raw.price ?? "").trim() !== "") {
    const p = parseOptionalMoney(raw.price, "Precio de venta");
    if (!p.ok) return p;
    price = p.value;
  }

  let costPrice: string | undefined;
  if ((raw.costPrice ?? "").trim() !== "") {
    const c = parseOptionalMoney(raw.costPrice, "Costo");
    if (!c.ok) return c;
    costPrice = c.value;
  }

  let stock: number | undefined;
  if ((raw.stock ?? "").trim() !== "") {
    const st = parseOptionalNonNegInt(raw.stock);
    if (!st.ok) return st;
    stock = st.value;
  }

  const attrs = cleanAttrs(raw.attrs);
  const name = (raw.name ?? "").trim() || undefined;

  const value: NewVariantInput = {
    barcode,
    name,
    price,
    costPrice,
    stock,
    attrs: Object.keys(attrs).length > 0 ? attrs : undefined,
  };
  return { ok: true, value };
}

function cleanAttrs(bag: Record<string, string> | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  if (!bag) return out;
  for (const [k, v] of Object.entries(bag)) {
    const key = k.trim();
    const val = String(v ?? "").trim();
    if (!key || val === "") continue;
    out[key] = val;
  }
  return out;
}

function parseOptionalMoney(
  raw: string | undefined,
  fieldLabel: string,
): { ok: true; value: string } | { ok: false; error: string } {
  const s = (raw ?? "").trim();
  const digits = s.replace(/[^\d]/g, "");
  if (digits === "" || !/^\d+$/.test(digits)) {
    return {
      ok: false,
      error: `${fieldLabel} inválido. Usa solo pesos enteros (ej. 1990 o 1.990).`,
    };
  }
  const n = Number(digits);
  if (!Number.isFinite(n) || n < 0) {
    return { ok: false, error: `${fieldLabel} debe ser 0 o más.` };
  }
  return { ok: true, value: String(Math.trunc(n)) };
}

function parseOptionalNonNegInt(
  raw: string | undefined,
): { ok: true; value: number } | { ok: false; error: string } {
  const s = (raw ?? "").trim();
  const digits = s.replace(/[^\d]/g, "");
  if (digits === "" || !/^\d+$/.test(digits)) {
    return { ok: false, error: "El stock debe ser un entero ≥ 0." };
  }
  const n = Number(digits);
  if (!Number.isFinite(n) || n < 0) {
    return { ok: false, error: "El stock debe ser un entero ≥ 0." };
  }
  return { ok: true, value: Math.trunc(n) };
}

/**
 * When "tiene variantes" is on at create time, stock on the parent should be 0
 * (sellable units live on children). Pure helper for form wiring.
 */
export function parentStockWhenHasVariants(
  hasVariants: boolean,
  physicalStock: boolean,
  stockRaw: string | undefined,
): number | undefined {
  if (!physicalStock) return undefined;
  if (hasVariants) return 0;
  const s = (stockRaw ?? "").trim();
  if (s === "") return undefined;
  const digits = s.replace(/[^\d]/g, "");
  if (digits === "") return undefined;
  return Math.trunc(Number(digits));
}
