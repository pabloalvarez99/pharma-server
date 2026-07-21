// Web port of commands/pos.rs: sales, refunds/devoluciones, receipts.
// `pos_sale` keeps the coded `"CODE|message"` reject shape and mints a fresh
// Idempotency-Key per attempt, exactly like the desktop.

import { base, codedError, errorMessage } from "../errors";
import {
  type CommandArgs,
  type CommandHandler,
  authHeaders,
  authHeadersCoded,
  doFetch,
  doFetchCoded,
  JSON_HEADERS,
  parseJson,
  putStr,
  qs,
} from "../core";

async function posSale(a: CommandArgs): Promise<unknown> {
  const items = a.items as unknown[];
  if (!items || items.length === 0) throw "|El carrito está vacío.";
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    items,
    payment_method: a.paymentMethod,
  };
  putStr(body, "cash_amount", a.cashAmount);
  putStr(body, "card_amount", a.cardAmount);
  putStr(body, "discount", a.discount);
  putStr(body, "customer", a.customer);
  const resp = await doFetchCoded(`${b}/api/v1/pos/sale`, {
    method: "POST",
    headers: authHeadersCoded({ ...JSON_HEADERS, "Idempotency-Key": crypto.randomUUID() }),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await codedError(resp);
  return parseJson(resp, "|Respuesta de venta inválida del servidor");
}

async function createRefund(a: CommandArgs): Promise<unknown> {
  const items = a.items as unknown[];
  if (!items || items.length === 0) throw "La devolución requiere al menos un ítem.";
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    tipo: a.tipo,
    motivo: a.motivo,
    items,
  };
  putStr(body, "order", a.order);
  putStr(body, "notas", a.notas);
  putStr(body, "metodo_reembolso", a.metodoReembolso);
  const resp = await doFetch(`${b}/api/v1/pos/returns`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de devolución inválida del servidor");
}

async function listRefunds(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/returns${qs({ order: a.order, tipo: a.tipo, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de devoluciones inválida del servidor");
}

async function getReceipt(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/orders/${a.id}/receipt`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de boleta inválida del servidor");
}

export const posCommands: Record<string, CommandHandler> = {
  pos_sale: posSale,
  create_refund: createRefund,
  list_refunds: listRefunds,
  get_receipt: getReceipt,
};
