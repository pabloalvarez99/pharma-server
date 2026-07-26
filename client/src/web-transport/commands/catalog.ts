// Web port of commands/catalog.rs: products, variants, batches, near-expiry,
// CSV import/export.

import { base, errorMessage } from "../errors";
import {
  type CommandArgs,
  type CommandHandler,
  authHeaders,
  doFetch,
  encodePathSegment,
  JSON_HEADERS,
  parseJson,
  parseText,
  putDef,
  putStr,
  qs,
} from "../core";

async function listProducts(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/products${qs({ search: a.search, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de productos inválida del servidor");
}

async function inventorySummary(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/products/stats`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de inventario inválida del servidor");
}

async function createProduct(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { name: a.name, price: a.price };
  putStr(body, "cost_price", a.costPrice);
  putDef(body, "stock", a.stock);
  putStr(body, "category", a.category);
  putStr(body, "laboratory", a.laboratory);
  putStr(body, "active_ingredient", a.activeIngredient);
  putStr(body, "prescription_type", a.prescriptionType);
  putStr(body, "presentation", a.presentation);
  // Pack attrs bag — only attach a non-empty object so we never send `null`.
  if (a.attrs && typeof a.attrs === "object" && Object.keys(a.attrs).length > 0) {
    body.attrs = a.attrs;
  }
  const resp = await doFetch(`${b}/api/v1/products`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de producto inválida del servidor");
}

/** Multipart form wrapping the CSV payload (server reads first field). */
function csvForm(csv: string): FormData {
  const form = new FormData();
  form.append("file", new Blob([csv], { type: "text/csv" }), "import.csv");
  return form;
}

async function importProducts(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/products/import`, {
    method: "POST",
    headers: authHeaders(),
    body: csvForm(a.csv as string),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de importación inválida del servidor");
}

async function importProductsPreview(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/products/import?dry_run=true`, {
    method: "POST",
    headers: authHeaders(),
    body: csvForm(a.csv as string),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de previsualización inválida del servidor");
}

async function exportProducts(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/products/export`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseText(resp, "Respuesta de exportación inválida del servidor");
}

async function productDetail(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/products/${a.id}`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de producto inválida del servidor");
}

async function productByBarcode(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const code = String(a.code ?? "").trim();
  if (code === "") throw "Ingresa un código de barras.";
  const resp = await doFetch(`${b}/api/v1/products/by-barcode/${encodePathSegment(code)}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de barcode inválida del servidor");
}

async function listProductVariants(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/products/${a.productId}/variants`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de variantes inválida del servidor");
}

async function createProductVariant(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {};
  putStr(body, "name", a.name);
  putStr(body, "price", a.price);
  putStr(body, "cost_price", a.costPrice);
  putDef(body, "stock", a.stock);
  putStr(body, "barcode", a.barcode);
  if (a.attrs && typeof a.attrs === "object" && Object.keys(a.attrs).length > 0) {
    body.attrs = a.attrs;
  }
  const resp = await doFetch(`${b}/api/v1/products/${a.parentId}/variants`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de variante inválida del servidor");
}

async function adjustProductStock(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {};
  putDef(body, "set", a.set);
  putDef(body, "delta", a.delta);
  putStr(body, "reason", a.reason);
  const resp = await doFetch(`${b}/api/v1/products/${a.id}/stock`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de ajuste inválida del servidor");
}

async function listBatches(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/batches${qs({
      product: a.product,
      // Sucursal del lote (V2.1): `none` = casa matriz, ausente = todos los locales.
      branch: a.branch,
      expiring_within_days: a.expiringWithinDays,
      only_available: a.onlyAvailable,
      limit: a.limit,
    })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de lotes inválida del servidor");
}

async function createBatch(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    product: a.product,
    batch_code: a.batchCode,
    expiry_date: a.expiryDate,
  };
  // El lote nace en la sucursal activa; `none` lo deja en casa matriz.
  if (a.branch && a.branch !== "none") body.branch = a.branch;
  putDef(body, "stock", a.stock);
  putStr(body, "cost", a.cost);
  putStr(body, "notes", a.notes);
  const resp = await doFetch(`${b}/api/v1/batches`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de lote inválida del servidor");
}

async function nearExpiry(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/reports/near-expiry${qs({ days: a.days, branch: a.branch })}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de vencimientos inválida del servidor");
}

export const catalogCommands: Record<string, CommandHandler> = {
  list_products: listProducts,
  inventory_summary: inventorySummary,
  create_product: createProduct,
  import_products: importProducts,
  import_products_preview: importProductsPreview,
  export_products: exportProducts,
  product_detail: productDetail,
  product_by_barcode: productByBarcode,
  list_product_variants: listProductVariants,
  create_product_variant: createProductVariant,
  adjust_product_stock: adjustProductStock,
  list_batches: listBatches,
  create_batch: createBatch,
  near_expiry: nearExpiry,
};
