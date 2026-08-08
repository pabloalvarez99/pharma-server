// Web port of commands/dte.rs. `emit_boleta` / `emit_documento` / `send_dte`
// reject coded (`"CODE|message"`) so the views can branch on
// FEATURE_REQUIRES_UPGRADE and cert-config errors, exactly like desktop.

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
  parseText,
  putDef,
  putStr,
  qs,
} from "../core";

async function listDtes(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/dte${qs({ estado: a.estado, tipo: a.tipo, from: a.from, to: a.to, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de boletas inválida del servidor");
}

async function dteCafStatus(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/dte/caf-status${qs({ tipo: a.tipo })}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de folios inválida del servidor");
}

async function dteXml(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/dte/${a.id}/xml`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseText(resp, "Respuesta de XML inválida del servidor");
}

async function dteLibroVentas(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const u = new URLSearchParams({ period: String(a.period) });
  const resp = await doFetch(`${b}/api/v1/dte/libro-ventas?${u.toString()}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseText(resp, "Respuesta de libro de ventas inválida del servidor");
}

async function dteLibroVentasSigned(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/dte/libro-ventas/signed`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify({ period: a.period, cert_passphrase: a.certPassphrase }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseText(resp, "Respuesta de libro firmado inválida del servidor");
}

async function emitBoleta(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    order_id: a.orderId,
    cert_passphrase: a.certPassphrase,
  };
  putStr(body, "receptor_rut", a.receptorRut);
  putStr(body, "razon_social_receptor", a.razonSocialReceptor);
  const resp = await doFetchCoded(`${b}/api/v1/dte/boletas`, {
    method: "POST",
    headers: authHeadersCoded(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await codedError(resp);
  return parseJson(resp, "|Respuesta de emisión inválida del servidor");
}

async function emitDocumento(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    tipo: a.tipo,
    cert_passphrase: a.certPassphrase,
    receptor: a.receptor,
    items: a.items,
  };
  const refs = a.referencias as unknown[] | undefined;
  if (refs && refs.length > 0) body.referencias = refs;
  putDef(body, "ind_traslado", a.indTraslado);
  putStr(body, "order_id", a.orderId);
  const resp = await doFetchCoded(`${b}/api/v1/dte/documentos`, {
    method: "POST",
    headers: authHeadersCoded(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await codedError(resp);
  return parseJson(resp, "|Respuesta de emisión inválida del servidor");
}

async function sendDte(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetchCoded(`${b}/api/v1/dte/${a.id}/send`, {
    method: "POST",
    headers: authHeadersCoded(),
  });
  if (!resp.ok) throw await codedError(resp);
  return parseJson(resp, "|Respuesta de envío inválida del servidor");
}

async function pollDte(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/dte/${a.id}/poll`, {
    method: "POST",
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de consulta SII inválida del servidor");
}

async function cancelDte(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/dte/${a.id}/cancel`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify({ reason: a.reason }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de anulación inválida del servidor");
}

export const dteCommands: Record<string, CommandHandler> = {
  list_dtes: listDtes,
  dte_caf_status: dteCafStatus,
  dte_xml: dteXml,
  dte_libro_ventas: dteLibroVentas,
  dte_libro_ventas_signed: dteLibroVentasSigned,
  emit_boleta: emitBoleta,
  emit_documento: emitDocumento,
  send_dte: sendDte,
  poll_dte: pollDte,
  cancel_dte: cancelDte,
};
