// Catalog / inventory wrappers (client/src-tauri/src/commands/catalog.rs).
// Money (`price`/`cost_price`/`cost`) is ALWAYS a STRING (Decimal) — never f64.
import { invoke } from "@tauri-apps/api/core";

/** A catalog product (trimmed projection from the server `ProductDto`).
 *  `price` is a STRING — money crosses the wire as `rust_decimal::serde::str`. */
export interface Product {
  id: string;
  name: string;
  price: string;
  stock: number;
  active: boolean;
  laboratory: string | null;
  active_ingredient: string | null;
}

/** `/products/stats` payload. `inventory_value` is a STRING (Decimal). */
export interface InventorySummary {
  total: number;
  active: number;
  low_stock: number;
  out_of_stock: number;
  inventory_value: string;
  expired: number;
}

/** GET /api/v1/products (Bearer). `search` + `limit` are optional filters. */
export function listProducts(
  serverUrl: string,
  search?: string,
  limit?: number,
): Promise<Product[]> {
  return invoke<Product[]>("list_products", { serverUrl, search, limit });
}

/** GET /api/v1/products/stats (Bearer) — inventory KPIs. */
export function inventorySummary(serverUrl: string): Promise<InventorySummary> {
  return invoke<InventorySummary>("inventory_summary", { serverUrl });
}

// --- inventario writes + lotes / vencimientos ------------------------------

/** Full product detail (`ProductDto`). Money (`price`/`cost_price`) are STRINGS. */
export interface ProductDetail {
  id: string;
  name: string;
  slug: string;
  description: string | null;
  price: string;
  cost_price: string | null;
  stock: number;
  category: string | null;
  active: boolean;
  laboratory: string | null;
  therapeutic_action: string | null;
  active_ingredient: string | null;
  prescription_type: string;
  presentation: string | null;
  discount_percent: number | null;
  /**
   * Per-rubro flexible bag (P0.2 / ProductDto.attrs). Present on GET when the
   * row has attrs (variants already; plain create once B wires NewProduct.attrs).
   * Wire key is always `attrs` (serde).
   */
  attrs?: Record<string, unknown> | null;
  /** Set when this row is a sellable multi-SKU child (migración 0034). */
  parent_id?: string | null;
}

/** One product batch / lote (`BatchDto`). `expiry_date` RFC3339; `cost` STRING|null. */
export interface Batch {
  id: string;
  product: string;
  batch_code: string;
  expiry_date: string;
  stock: number;
  cost: string | null;
  notes: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** A soon-to-expire / expired batch row (`NearExpiryRow`).
 *  `days_to_expiry` < 0 ⇒ already expired (also `expired === true`). */
export interface NearExpiryRow {
  product_id: string;
  product_name: string;
  batch_id: string;
  batch_code: string;
  expiry_date: string;
  stock: number;
  days_to_expiry: number;
  expired: boolean;
}

/** Fields the "Nuevo producto" form collects. `price`/`costPrice` are STRINGS.
 *  `attrs` is the pack-driven flexible bag (talla, color, sku, …). */
export interface NewProductInput {
  name: string;
  price: string;
  costPrice?: string;
  stock?: number;
  laboratory?: string;
  activeIngredient?: string;
  presentation?: string;
  /** Pack attrs not promoted to top-level columns. Omitted when empty. */
  attrs?: Record<string, string>;
}

/** POST /api/v1/products (Bearer, admin+). Rejects with a Spanish string
 *  ("Permiso denegado…" on a non-admin 403). Money stays STRING on the wire.
 *
 *  `attrs` is the pack bag (`{ talla, color, sku, … }`). Wire key is **`attrs`**
 *  (serde, snake-free). **BLOCKED persist:** until domain `NewProduct.attrs` +
 *  `repo::create_product` SET attrs, the server ignores this field (no 4xx).
 *  GET detail already returns attrs when present (variants / future create). */
export function createProduct(
  serverUrl: string,
  input: NewProductInput,
): Promise<ProductDetail> {
  return invoke<ProductDetail>("create_product", {
    serverUrl,
    name: input.name,
    price: input.price,
    costPrice: input.costPrice,
    stock: input.stock,
    laboratory: input.laboratory,
    activeIngredient: input.activeIngredient,
    presentation: input.presentation,
    // Serde key on the HTTP body is `attrs` (object). Tauri arg name matches.
    attrs: input.attrs,
  });
}

/** One rejected row from a bulk CSV import (1-based line + reason). */
export interface ImportRowError {
  line: number;
  message: string;
}

/** Outcome of `POST /products/import` — row counts + per-row errors. */
export interface ImportSummary {
  created: number;
  updated: number;
  failed: number;
  errors: ImportRowError[];
}

/** POST /api/v1/products/import (Bearer, admin+). `csv` = raw file text; the
 *  server upserts by `external_id` (idempotent). Returns the import summary. */
export function importProducts(
  serverUrl: string,
  csv: string,
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_products", { serverUrl, csv });
}

/** POST /api/v1/products/import?dry_run=true (Bearer, admin+). Validates + counts
 *  the CSV WITHOUT writing anything — powers the preview shown before the operator
 *  confirms a catalog migration. Same {@link ImportSummary} shape as a real run. */
export function importProductsPreview(
  serverUrl: string,
  csv: string,
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_products_preview", { serverUrl, csv });
}

/** GET /api/v1/products/export (Bearer) — full catalog as CSV text. The webview
 *  wraps it in a Blob for download. Columns round-trip with {@link importProducts}. */
export function exportProducts(serverUrl: string): Promise<string> {
  return invoke<string>("export_products", { serverUrl });
}

/** GET /api/v1/products/{id} (Bearer) — full detail for the drawer. */
export function productDetail(
  serverUrl: string,
  id: string,
): Promise<ProductDetail> {
  return invoke<ProductDetail>("product_detail", { serverUrl, id });
}

// --- multi-SKU variants (API fase 1 / B) ------------------------------------
// GET by-barcode, GET/POST .../variants. Full matrix UI is out of scope for C;
// inventory shows a light banner + list; POS scans barcodes to children.

/** GET /api/v1/products/by-barcode/{code} — sellable product (variant or plain). */
export function productByBarcode(
  serverUrl: string,
  code: string,
): Promise<ProductDetail> {
  return invoke<ProductDetail>("product_by_barcode", { serverUrl, code });
}

/** GET /api/v1/products/{id}/variants — children of a parent (empty if none). */
export function listProductVariants(
  serverUrl: string,
  productId: string,
): Promise<ProductDetail[]> {
  return invoke<ProductDetail[]>("list_product_variants", {
    serverUrl,
    productId,
  });
}

/** Body for POST /api/v1/products/{id}/variants. Money strings; attrs bag. */
export interface NewVariantInput {
  name?: string;
  price?: string;
  costPrice?: string;
  stock?: number;
  barcode?: string;
  attrs?: Record<string, string>;
}

/**
 * POST /api/v1/products/{id}/variants (admin+).
 * Wired for when inventory grows a thin "agregar variante" form — matrix UI TODO.
 */
export function createProductVariant(
  serverUrl: string,
  parentId: string,
  input: NewVariantInput,
): Promise<ProductDetail> {
  return invoke<ProductDetail>("create_product_variant", {
    serverUrl,
    parentId,
    name: input.name,
    price: input.price,
    costPrice: input.costPrice,
    stock: input.stock,
    barcode: input.barcode,
    attrs: input.attrs,
  });
}

/** Map detail → POS list Product projection. */
export function productFromDetail(d: ProductDetail): Product {
  return {
    id: d.id,
    name: d.name,
    price: d.price,
    stock: d.stock,
    active: d.active,
    laboratory: d.laboratory,
    active_ingredient: d.active_ingredient,
  };
}

/** POST /api/v1/products/{id}/stock (Bearer, admin+). Pass `set` (absolute) or
 *  `delta` (signed) + optional `reason`. Returns the updated product. */
export function adjustProductStock(
  serverUrl: string,
  id: string,
  opts: { set?: number; delta?: number; reason?: string },
): Promise<ProductDetail> {
  return invoke<ProductDetail>("adjust_product_stock", {
    serverUrl,
    id,
    set: opts.set,
    delta: opts.delta,
    reason: opts.reason,
  });
}

/** GET /api/v1/batches (Bearer). Optional `product` + `expiringWithinDays`
 *  + `onlyAvailable` filters. */
export function listBatches(
  serverUrl: string,
  product?: string,
  expiringWithinDays?: number,
  onlyAvailable?: boolean,
  limit?: number,
): Promise<Batch[]> {
  return invoke<Batch[]>("list_batches", {
    serverUrl,
    product,
    expiringWithinDays,
    onlyAvailable,
    limit,
  });
}

/** POST /api/v1/batches (Bearer, admin+). `expiryDate` must be RFC3339
 *  (`YYYY-MM-DDT00:00:00Z`). `cost` is a STRING when present. */
export function createBatch(
  serverUrl: string,
  product: string,
  batchCode: string,
  expiryDate: string,
  opts: { stock?: number; cost?: string; notes?: string } = {},
): Promise<Batch> {
  return invoke<Batch>("create_batch", {
    serverUrl,
    product,
    batchCode,
    expiryDate,
    stock: opts.stock,
    cost: opts.cost,
    notes: opts.notes,
  });
}

/** GET /api/v1/reports/near-expiry?days=N (Bearer). Core/free, not gated. */
export function nearExpiry(
  serverUrl: string,
  days?: number,
): Promise<NearExpiryRow[]> {
  return invoke<NearExpiryRow[]>("near_expiry", { serverUrl, days });
}
