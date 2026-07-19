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

/** Compact badge for list/card when variant **count** is known. */
export function variantsListBadge(count: number): string {
  const n = Math.max(0, Math.trunc(count));
  if (n <= 0) return "";
  return n === 1 ? "1 variante" : `${n} variantes`;
}

/**
 * List-row badge from B `variants_stock` (sum of children units, not child count).
 * Prefer this on catalog list — no N+1 GET /variants.
 */
export function variantsStockListBadge(variantsStock: number): string {
  const n = Math.max(0, Math.trunc(variantsStock));
  return `Multi-SKU · ${n} u.`;
}

/**
 * Prefer `variant_count` (N variantes) when B exposes it; fall back to stock sum.
 * Chile retail copy — short for table cells.
 */
export function variantsListBadgeFromDto(p: {
  variant_count?: number | null;
  variants_stock?: number | null;
}): string {
  if (p.variant_count != null && typeof p.variant_count === "number") {
    return variantsListBadge(p.variant_count);
  }
  if (p.variants_stock != null && typeof p.variants_stock === "number") {
    return variantsStockListBadge(p.variants_stock);
  }
  return "";
}

/** Stock cell for a variant row: number + optional Agotado pill label. */
export function variantStockCellLabel(stock: number): { text: string; out: boolean } {
  const n = typeof stock === "number" && Number.isFinite(stock) ? Math.trunc(stock) : 0;
  if (n <= 0) return { text: "Agotado", out: true };
  return { text: String(n), out: false };
}

/** Accessible label for a variant table row. */
export function variantRowAriaLabel(row: {
  name: string;
  barcode?: string;
  stock: number;
  attrsLabel?: string;
}): string {
  const name = (row.name || "Variante").trim() || "Variante";
  const parts = [name];
  if (row.attrsLabel) parts.push(row.attrsLabel);
  if (row.barcode && row.barcode !== "—") parts.push(`código ${row.barcode}`);
  const st = variantStockCellLabel(row.stock);
  parts.push(st.out ? "agotado" : `stock ${st.text}`);
  return parts.join(", ");
}

/** Loading skeleton copy (announced / aria-busy). */
export function variantsLoadingLabel(): string {
  return "Cargando variantes…";
}

/** Minimal HTML skeleton for the variants block (no DOM API; inventory injects). */
export function variantsLoadingHtml(escapeHtml: (s: string) => string): string {
  return `<div class="pd-section pd-variants" aria-busy="true" role="status">
    <div class="pd-section-head"><h4>${escapeHtml(variantsLoadingLabel())}</h4></div>
    <div class="pd-variants-skel">
      <div class="skel-line"></div>
      <div class="skel-line skel-short"></div>
      <div class="skel-line"></div>
    </div>
  </div>`;
}

/** Load error (Spanish, operator-facing). */
export function variantsLoadError(detail?: string): string {
  const d = (detail || "").trim();
  if (d) return `No se pudieron cargar las variantes. ${d}`;
  return "No se pudieron cargar las variantes. Revisa la conexión e intenta de nuevo.";
}

/**
 * Thin matrix helper (no full grid API): cartesian of talla × color values
 * for "quick add" chips / honesty about missing combos. Caps at 24 combos.
 */
export function matrixComboSuggestions(
  tallas: readonly string[],
  colores: readonly string[],
  existing?: readonly { talla?: string; color?: string }[],
): { talla: string; color: string; label: string; missing: boolean }[] {
  const ts = uniqueNonEmpty(tallas).slice(0, 8);
  const cs = uniqueNonEmpty(colores).slice(0, 8);
  if (ts.length === 0 && cs.length === 0) return [];
  const have = new Set(
    (existing ?? []).map((e) => `${(e.talla || "").trim().toLowerCase()}|${(e.color || "").trim().toLowerCase()}`),
  );
  const out: { talla: string; color: string; label: string; missing: boolean }[] = [];
  if (ts.length > 0 && cs.length > 0) {
    for (const t of ts) {
      for (const c of cs) {
        const key = `${t.toLowerCase()}|${c.toLowerCase()}`;
        out.push({
          talla: t,
          color: c,
          label: `${t} · ${c}`,
          missing: !have.has(key),
        });
        if (out.length >= 24) return out;
      }
    }
    return out;
  }
  // One dimension only.
  const singles = ts.length > 0 ? ts : cs;
  const dim = ts.length > 0 ? "talla" : "color";
  for (const v of singles) {
    const key = dim === "talla" ? `${v.toLowerCase()}|` : `|${v.toLowerCase()}`;
    out.push({
      talla: dim === "talla" ? v : "",
      color: dim === "color" ? v : "",
      label: v,
      missing: !have.has(key) && !have.has(`${v.toLowerCase()}|`) && !have.has(`|${v.toLowerCase()}`),
    });
    if (out.length >= 24) break;
  }
  return out;
}

function uniqueNonEmpty(vals: readonly string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of vals) {
    const v = (raw ?? "").trim();
    if (!v) continue;
    const k = v.toLowerCase();
    if (seen.has(k)) continue;
    seen.add(k);
    out.push(v);
  }
  return out;
}

/** Chile copy: soft note when edit/delete variant API is not exposed. */
export function variantEditBlockedHint(): string {
  return "Editar o desactivar variantes: por API de producto (PATCH) o CSV. Próximamente en este panel.";
}

/** Keyboard help under variant form. */
export function variantFormKeyboardHint(): string {
  return "Enter en el código de barras crea la variante · Esc cierra el formulario.";
}

/**
 * Future PATCH body for edit variant (name/price/stock/attrs).
 * No Tauri command yet — pure validation for when B/C wire update.
 */
export interface EditVariantRaw {
  name?: string;
  price?: string;
  costPrice?: string;
  stock?: string;
  attrs?: Record<string, string>;
  active?: boolean;
}

export type EditVariantBuildResult =
  | {
      ok: true;
      value: {
        name?: string;
        price?: string;
        costPrice?: string;
        stock?: number;
        attrs?: Record<string, string>;
        active?: boolean;
      };
    }
  | { ok: false; error: string };

export function buildEditVariantInput(raw: EditVariantRaw): EditVariantBuildResult {
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
  return {
    ok: true,
    value: {
      name,
      price,
      costPrice,
      stock,
      attrs: Object.keys(attrs).length > 0 ? attrs : undefined,
      active: raw.active,
    },
  };
}

/**
 * Soft barcode rules for create form (required, min length, no spaces).
 * Internal 13-digit non-EAN codes are allowed — many Chilean SKUs are not GS1.
 * Use {@link isEan13ChecksumValid} only as optional UI hint, not hard reject.
 */
export function validateBarcodeSoft(raw: string): { ok: true } | { ok: false; error: string } {
  const s = (raw ?? "").trim();
  if (s === "") {
    return { ok: false, error: "Ingresa el código de barras de la variante (escáner o teclado)." };
  }
  if (s.length < 3) {
    return { ok: false, error: "El código de barras es demasiado corto (mín. 3 caracteres)." };
  }
  if (/\s/.test(s)) {
    return { ok: false, error: "El código de barras no debe tener espacios." };
  }
  return { ok: true };
}

/** Optional GS1 EAN-13 checksum (hint only — never blocks internal codes). */
export function isEan13ChecksumValid(raw: string): boolean {
  const s = (raw ?? "").trim();
  if (!/^\d{13}$/.test(s)) return false;
  let sum = 0;
  for (let i = 0; i < 12; i++) {
    const d = Number(s[i]);
    sum += i % 2 === 0 ? d : d * 3;
  }
  const check = (10 - (sum % 10)) % 10;
  return check === Number(s[12]);
}

/** Operator hint when 13 digits fail GS1 (still allowed to save). */
export function ean13ChecksumHint(raw: string): string {
  const s = (raw ?? "").trim();
  if (!/^\d{13}$/.test(s)) return "";
  if (isEan13ChecksumValid(s)) return "";
  return "Aviso: este EAN-13 no cuadra el dígito verificador. Si es un código interno, puedes guardarlo igual.";
}

/** Inactive child row note (when active=false). */
export function variantInactiveLabel(): string {
  return "Inactiva";
}

/** List-row presentation model (inventory table). Pure for tests. */
export interface ProductListVariantMeta {
  isParent: boolean;
  badge: string;
  stockDisplay: string;
  statusPill: "agotado" | "multi-sku" | "ok" | "servicio" | null;
  subNote: string;
}

export function productListVariantMeta(
  p: {
    stock: number;
    variant_count?: number | null;
    variants_stock?: number | null;
  },
  physicalStock: boolean,
): ProductListVariantMeta {
  if (!physicalStock) {
    return {
      isParent: false,
      badge: "",
      stockDisplay: "—",
      statusPill: "servicio",
      subNote: "",
    };
  }
  const isParent =
    (p.variant_count != null && typeof p.variant_count === "number") ||
    (p.variants_stock != null && typeof p.variants_stock === "number");
  if (isParent) {
    return {
      isParent: true,
      badge: variantsListBadgeFromDto(p),
      stockDisplay:
        p.variants_stock != null ? `${Math.trunc(Number(p.variants_stock))} u. en variantes` : "—",
      statusPill: "multi-sku",
      subNote: "Vender por código de barras del hijo",
    };
  }
  if (p.stock <= 0) {
    return {
      isParent: false,
      badge: "",
      stockDisplay: "0",
      statusPill: "agotado",
      subNote: "",
    };
  }
  return {
    isParent: false,
    badge: "",
    stockDisplay: String(Math.trunc(p.stock)),
    statusPill: "ok",
    subNote: "",
  };
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
  const bc = validateBarcodeSoft(barcode);
  if (!bc.ok) return bc;

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
