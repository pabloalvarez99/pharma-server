// Pure product-form model (alta / edición) — pack-driven, no DOM.
//
// Multi-rubro: the form is NOT a pharmacy sheet with fields hidden. Each rubro
// pack declares `attrs` (talla, color, sku, duracion_min, …) and features
// (clinical, physicalStock). This module maps that pack into a wire payload
// the inventory modal posts, with Spanish validation and money-as-string.
//
// Known attr keys that still live as top-level NewProduct columns
// (laboratory / active_ingredient / …) are promoted so they persist today;
// every other key goes into `product.attrs` JSON.
//
// ## BLOCKED (persist attrs end-to-end) — 2026-07-18
//
// **Read path OK:** `ProductDto.attrs` + GET detail return the bag; client
// `ProductDetail.attrs` deserializes it (variants already write attrs).
//
// **Write path BLOCKED on B (domain):**
// - `NewProduct` has NO `attrs` field → serde drops the key on POST /products.
// - `UpdateProduct` has NO `attrs` field → PATCH cannot update the bag.
// - `repo::create_product` SQL does not SET `attrs` (only `create_variant_product` does).
//
// Client already POSTs the correct serde key (see {@link productAttrsWireBody}):
//   `{ "attrs": { "talla": "M", "color": "Negro", "sku": "…" } }`
// When B adds `attrs: Option<serde_json::Value>` on NewProduct/UpdateProduct and
// binds it in repo create/update, round-trip works with zero client renames.
//
// Variantes multi-SKU (parent_id / N SKU) use a separate API (`NewVariant`);
// this form is the flat product + attrs path for a single SKU row.

import type { PackAttrField, PackVocab } from "../api/rubro";
import type { NewProductInput } from "../api/catalog";

/** Attr keys that still map to top-level product columns on create. */
export const TOP_LEVEL_ATTR_KEYS = new Set([
  "laboratory",
  "active_ingredient",
  "therapeutic_action",
  "presentation",
]);

/** Operator-facing copy for the form chrome (title, CTAs, stock vs service). */
export interface ProductFormLabels {
  title: string;
  nameLabel: string;
  namePlaceholder: string;
  priceLabel: string;
  costLabel: string;
  stockLabel: string;
  stockHint: string | null;
  submitLabel: string;
  itemWord: string;
}

export interface ProductFormOptions {
  /** Pack vocabulary (Producto / Servicio / …). */
  vocab: PackVocab;
  /** Show stock field and require physical-product copy. */
  physicalStock: boolean;
  /** Show clinical ficha (farmacia). When false, never show lab/ingredient chrome. */
  clinical: boolean;
  /** Dynamic fields from `cachedPack().attrs` (may be empty offline). */
  attrFields: readonly PackAttrField[];
}

/** Raw values the modal collects before validation. */
export interface ProductFormRaw {
  name: string;
  price: string;
  costPrice?: string;
  stock?: string;
  presentation?: string;
  laboratory?: string;
  activeIngredient?: string;
  /** Values keyed by PackAttrField.key (includes clinical keys when pack-driven). */
  attrs?: Record<string, string>;
}

export type ProductFormResult =
  | { ok: true; value: NewProductInput }
  | { ok: false; error: string };

/** Labels for the alta modal — pack vocab + physical vs service UX. */
export function productFormLabels(opts: Pick<ProductFormOptions, "vocab" | "physicalStock">): ProductFormLabels {
  const item = (opts.vocab.item || "Producto").trim() || "Producto";
  const lower = item.toLowerCase();
  if (!opts.physicalStock) {
    return {
      title: `Nuevo ${lower}`,
      nameLabel: `Nombre del ${lower} *`,
      namePlaceholder: `Ej. Corte de cabello, manicure…`,
      priceLabel: "Precio (CLP) *",
      costLabel: "Costo (CLP)",
      stockLabel: "No aplica stock",
      stockHint: "Este rubro vende servicios: no se controla inventario físico.",
      submitLabel: `Crear ${lower}`,
      itemWord: item,
    };
  }
  return {
    title: `Nuevo ${lower}`,
    nameLabel: `Nombre *`,
    namePlaceholder: `Nombre del ${lower}`,
    priceLabel: "Precio venta (CLP) *",
    costLabel: "Costo (CLP)",
    stockLabel: "Stock inicial",
    stockHint: null,
    submitLabel: `Crear ${lower}`,
    itemWord: item,
  };
}

/**
 * Fields the form should render as pack attrs.
 * - Drops clinical keys when `!clinical` (even if a stale pack listed them).
 * - Keeps non-clinical attrs always (talla, color, sku, duracion_min…).
 */
export function visibleAttrFields(
  attrFields: readonly PackAttrField[] | null | undefined,
  clinical: boolean,
): PackAttrField[] {
  const list = attrFields ?? [];
  if (clinical) return list.map((f) => ({ ...f }));
  return list.filter((f) => !isClinicalAttrKey(f.key));
}

/** Whether a pack attr key is part of the pharmacy clinical ficha. */
export function isClinicalAttrKey(key: string): boolean {
  const k = key.trim().toLowerCase();
  return (
    k === "laboratory" ||
    k === "active_ingredient" ||
    k === "therapeutic_action" ||
    k === "principio_activo" ||
    k === "laboratorio"
  );
}

/**
 * Parse a CLP money field to a non-negative integer STRING for the wire.
 * Accepts "1990", "1.990", "$1.990", blanks → null when optional.
 * Returns `{ ok:false }` with Spanish error when invalid.
 */
export function parseMoneyString(
  raw: string | undefined | null,
  opts: { required: boolean; fieldLabel: string },
): { ok: true; value: string | undefined } | { ok: false; error: string } {
  const s = (raw ?? "").trim();
  if (s === "") {
    if (opts.required) {
      return { ok: false, error: `Ingresa ${opts.fieldLabel.toLowerCase()} (0 o más).` };
    }
    return { ok: true, value: undefined };
  }
  // Digits only (CL grouping dots / spaces / $ stripped). No floats on wire.
  const digits = s.replace(/[^\d]/g, "");
  if (digits === "" || !/^\d+$/.test(digits)) {
    return {
      ok: false,
      error: `${opts.fieldLabel} inválido. Usa solo pesos enteros (ej. 1990 o 1.990).`,
    };
  }
  // Reject leading zeros that aren't zero itself? Keep as number string without leading zeros.
  const n = Number(digits);
  if (!Number.isFinite(n) || n < 0) {
    return { ok: false, error: `${opts.fieldLabel} debe ser 0 o más.` };
  }
  return { ok: true, value: String(Math.trunc(n)) };
}

/** Optional non-negative integer (stock). Blank → undefined. */
export function parseOptionalInt(
  raw: string | undefined | null,
): { ok: true; value: number | undefined } | { ok: false; error: string } {
  const s = (raw ?? "").trim();
  if (s === "") return { ok: true, value: undefined };
  if (!/^-?\d+$/.test(s.replace(/\./g, ""))) {
    // Allow "1.000" grouping? Stock is usually small; accept plain digits only.
  }
  const digits = s.replace(/[^\d-]/g, "");
  const n = Number(digits);
  if (!Number.isFinite(n) || !Number.isInteger(n) || n < 0) {
    return { ok: false, error: "El stock debe ser un entero ≥ 0." };
  }
  return { ok: true, value: n };
}

/**
 * Build `NewProductInput` from raw form values + pack options.
 * Money always leaves as STRING. Empty optional attrs are omitted.
 */
export function buildProductInput(raw: ProductFormRaw, opts: ProductFormOptions): ProductFormResult {
  const name = (raw.name ?? "").trim();
  const item = (opts.vocab.item || "producto").toLowerCase();
  if (name === "") {
    return { ok: false, error: `Ingresa el nombre del ${item}.` };
  }

  const price = parseMoneyString(raw.price, { required: true, fieldLabel: "Precio de venta" });
  if (!price.ok) return price;
  const cost = parseMoneyString(raw.costPrice, { required: false, fieldLabel: "Costo" });
  if (!cost.ok) return cost;

  let stock: number | undefined;
  if (opts.physicalStock) {
    const st = parseOptionalInt(raw.stock);
    if (!st.ok) return st;
    stock = st.value;
  }
  // Service rubros: never send stock (server defaults 0; no inventory UX).

  const attrValues = { ...(raw.attrs ?? {}) };
  // Legacy fixed clinical fields → merge into attr bag under canonical keys so
  // one code path promotes top-level vs attrs bag.
  if (opts.clinical) {
    if (raw.laboratory?.trim()) attrValues.laboratory = raw.laboratory.trim();
    if (raw.activeIngredient?.trim()) attrValues.active_ingredient = raw.activeIngredient.trim();
    if (raw.presentation?.trim() && !attrValues.presentation) {
      attrValues.presentation = raw.presentation.trim();
    }
  } else if (raw.presentation?.trim()) {
    // Presentación stays universal (pack size / format) even without clinical.
    attrValues.presentation = raw.presentation.trim();
  }

  // Only keep attrs that are visible for this pack (drop clinical if !clinical).
  const allowed = new Set(visibleAttrFields(opts.attrFields, opts.clinical).map((f) => f.key));
  // Always allow presentation + promoted clinical keys we just set.
  allowed.add("presentation");
  if (opts.clinical) {
    allowed.add("laboratory");
    allowed.add("active_ingredient");
    allowed.add("therapeutic_action");
  }

  const bag: Record<string, string> = {};
  for (const [k, v] of Object.entries(attrValues)) {
    const key = k.trim();
    if (!key) continue;
    if (!allowed.has(key) && !TOP_LEVEL_ATTR_KEYS.has(key)) continue;
    // If clinical is off, never ship clinical keys.
    if (!opts.clinical && isClinicalAttrKey(key)) continue;
    const val = String(v ?? "").trim();
    if (val === "") continue;
    // Number attrs: keep digits (and one decimal comma/dot) as plain text wire value.
    bag[key] = val;
  }

  const top = promoteTopLevel(bag);
  const restAttrs = stripTopLevel(bag);

  const input: NewProductInput = {
    name,
    price: price.value!,
    costPrice: cost.value,
    stock,
    laboratory: top.laboratory,
    activeIngredient: top.active_ingredient,
    presentation: top.presentation ?? (raw.presentation?.trim() || undefined),
    // Flexible bag — server persists when NewProduct.attrs is wired (P0.2 field
    // exists on ProductDto; create path may ignore until B completes write path).
    attrs: Object.keys(restAttrs).length > 0 ? restAttrs : undefined,
  };

  return { ok: true, value: input };
}

/** Split pack attr values into top-level columns vs flexible attrs bag. */
export function promoteTopLevel(bag: Record<string, string>): {
  laboratory?: string;
  active_ingredient?: string;
  therapeutic_action?: string;
  presentation?: string;
} {
  return {
    laboratory: bag.laboratory?.trim() || undefined,
    active_ingredient: bag.active_ingredient?.trim() || undefined,
    therapeutic_action: bag.therapeutic_action?.trim() || undefined,
    presentation: bag.presentation?.trim() || undefined,
  };
}

/** attrs bag without keys already promoted to top-level columns. */
export function stripTopLevel(bag: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(bag)) {
    if (TOP_LEVEL_ATTR_KEYS.has(k)) continue;
    if (v.trim() === "") continue;
    out[k] = v.trim();
  }
  return out;
}

/**
 * HTML inputs for pack attrs (pure string builder — view injects). Escape
 * labels via caller if untrusted; pack labels are server-controlled constants.
 */
export function attrFieldsHtml(
  fields: readonly PackAttrField[],
  escapeHtml: (s: string) => string,
  idPrefix = "np-attr-",
): string {
  if (fields.length === 0) return "";
  const inputs = fields
    .map((f) => {
      const id = `${idPrefix}${escapeHtml(f.key)}`;
      const kind = (f.kind || "text").toLowerCase();
      const inputMode =
        kind === "number" || kind === "money" ? ' inputmode="numeric"' : "";
      const type = kind === "date" ? "date" : "text";
      const ph =
        kind === "money" ? "0" : kind === "number" ? "0" : "opcional";
      return `
        <label class="field modal-field">
          <span class="modal-label">${escapeHtml(f.label)}</span>
          <input id="${id}" data-attr-key="${escapeHtml(f.key)}" data-attr-kind="${escapeHtml(kind)}"
                 type="${type}"${inputMode} autocomplete="off" placeholder="${ph}" />
        </label>`;
    })
    .join("");
  return `<div class="inv-form-row inv-form-attrs">${inputs}</div>`;
}

/** Read attr input values from a root element (modal). */
export function readAttrValues(
  root: ParentNode,
  fields: readonly PackAttrField[],
  idPrefix = "np-attr-",
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const f of fields) {
    const el = root.querySelector<HTMLInputElement>(`#${cssEscape(idPrefix + f.key)}`);
    if (el) out[f.key] = el.value;
  }
  return out;
}

// Minimal CSS.escape polyfill for attr ids (keys are [a-z0-9_]).
function cssEscape(s: string): string {
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") return CSS.escape(s);
  return s.replace(/([^a-zA-Z0-9_-])/g, "\\$1");
}

// Offline attr list lives in vertical.ts (`localAttrsForRubro`) so the pack
// cache and this form share one source without a views↔vertical import cycle.

// --- wire contract (serde field names B must accept) -------------------------

/**
 * Shape of the JSON body fragment for pack attrs on create/update product.
 * Single source so Tauri + tests never drift from the domain serde key `attrs`.
 * Returns `undefined` when empty (omit key — never send `null`/`{}`).
 */
export function productAttrsWireBody(
  attrs: Record<string, string> | null | undefined,
): { attrs: Record<string, string> } | undefined {
  if (!attrs) return undefined;
  const clean: Record<string, string> = {};
  for (const [k, v] of Object.entries(attrs)) {
    const key = k.trim();
    const val = String(v ?? "").trim();
    if (!key || val === "") continue;
    clean[key] = val;
  }
  if (Object.keys(clean).length === 0) return undefined;
  return { attrs: clean };
}
