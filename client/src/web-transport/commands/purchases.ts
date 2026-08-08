// Web port of commands/purchases.rs: OCs, proveedores, pagos, recepción.

import { base, errorMessage } from "../errors";
import {
  type CommandArgs,
  type CommandHandler,
  authHeaders,
  doFetch,
  JSON_HEADERS,
  parseJson,
  putStr,
  qs,
} from "../core";

async function listPurchaseOrders(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/purchase-orders${qs({ status: a.status, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de órdenes de compra inválida del servidor");
}

async function getPurchaseOrder(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/purchase-orders/${a.id}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de orden de compra inválida del servidor");
}

async function getPoPayments(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/purchase-orders/${a.id}/payments`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de pagos de OC inválida del servidor");
}

async function createPoPayment(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { amount: a.amount };
  putStr(body, "payment_method", a.paymentMethod);
  putStr(body, "cash_session", a.cashSession);
  putStr(body, "reference", a.reference);
  const resp = await doFetch(`${b}/api/v1/purchase-orders/${a.id}/payments`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de pago de OC inválida del servidor");
}

async function listSuppliers(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/suppliers${qs({ search: a.search, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de proveedores inválida del servidor");
}

async function createSupplier(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { name: a.name };
  putStr(body, "rut", a.rut);
  putStr(body, "contact_name", a.contactName);
  putStr(body, "contact_email", a.contactEmail);
  putStr(body, "contact_phone", a.contactPhone);
  const resp = await doFetch(`${b}/api/v1/suppliers`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de proveedor inválida del servidor");
}

async function createPurchaseOrder(a: CommandArgs): Promise<unknown> {
  const items = a.items as unknown[];
  if (!items || items.length === 0) throw "La orden de compra requiere al menos un ítem.";
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { supplier: a.supplier, items };
  putStr(body, "currency", a.currency);
  putStr(body, "notes", a.notes);
  putStr(body, "external_ref", a.externalRef);
  // Local al que ENTRA la mercadería (V2.1): los lotes de la recepción nacen
  // ahí. `none` = casa matriz y el server ya lo asume, no hace falta mandarlo.
  if (a.branch && a.branch !== "none") body.branch = a.branch;
  const resp = await doFetch(`${b}/api/v1/purchase-orders`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de creación de OC inválida del servidor");
}

async function receivePurchaseOrder(a: CommandArgs): Promise<unknown> {
  const lines = a.lines as unknown[];
  if (!lines || lines.length === 0) throw "La recepción requiere al menos una línea.";
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { lines };
  putStr(body, "notes", a.notes);
  const resp = await doFetch(`${b}/api/v1/purchase-orders/${a.id}/receive`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de recepción inválida del servidor");
}

async function sendPurchaseOrder(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/purchase-orders/${a.id}/send`, {
    method: "POST",
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de emisión de OC inválida del servidor");
}

export const purchaseCommands: Record<string, CommandHandler> = {
  list_purchase_orders: listPurchaseOrders,
  get_purchase_order: getPurchaseOrder,
  get_po_payments: getPoPayments,
  create_po_payment: createPoPayment,
  list_suppliers: listSuppliers,
  create_supplier: createSupplier,
  create_purchase_order: createPurchaseOrder,
  receive_purchase_order: receivePurchaseOrder,
  send_purchase_order: sendPurchaseOrder,
};
