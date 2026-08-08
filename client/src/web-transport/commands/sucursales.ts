// Web port of commands/sucursales.rs: sucursales, cajas, stock por sucursal y
// transferencias entre locales (V2 multi-sucursal operativo).

import { base, errorMessage, codedError } from "../errors";
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

async function sucursales(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/sucursales${qs({ active: a.active })}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de sucursales inválida del servidor");
}

async function cajas(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/cajas${qs({ branch: a.branch, active: a.active })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de cajas inválida del servidor");
}

async function stockPorSucursal(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/stock/sucursales${qs({
      product: a.product,
      branch: a.branch,
      non_zero: a.nonZero,
    })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de stock por sucursal inválida del servidor");
}

async function stockPorSucursalReporte(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/stock/sucursales/reporte${qs({
      branch: a.branch,
      non_zero: a.nonZero,
    })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta del reporte por sucursal inválida del servidor");
}

async function transferirStock(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { product: a.product, qty: a.qty };
  // "none" = casa matriz: se manda ausente, no como string.
  if (a.fromBranch && a.fromBranch !== "none") body.from_branch = a.fromBranch;
  if (a.toBranch && a.toBranch !== "none") body.to_branch = a.toBranch;
  putStr(body, "notes", a.notes);
  const resp = await doFetch(`${b}/api/v1/stock/transferencias`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  // Igual que la venta: el error llega como `CODE|mensaje` para que la UI
  // distinga INSUFFICIENT_STOCK del resto.
  if (!resp.ok) throw await codedError(resp);
  return parseJson(resp, "Respuesta de transferencia inválida del servidor");
}

export const sucursalCommands: Record<string, CommandHandler> = {
  sucursales,
  cajas,
  stock_por_sucursal: stockPorSucursal,
  stock_por_sucursal_reporte: stockPorSucursalReporte,
  transferir_stock: transferirStock,
};
