// Web port of commands/cash.rs: apertura, arqueo, cierre de caja.

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

async function cashSessions(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/cash-sessions${qs({ status: a.status, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de cajas inválida del servidor");
}

async function openCashSession(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    register_name: a.registerName,
    opening_cash: a.openingCash,
  };
  putStr(body, "notes", a.notes);
  const resp = await doFetch(`${b}/api/v1/cash-sessions`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de apertura inválida del servidor");
}

async function cashArqueo(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/cash-sessions/${a.id}/arqueo`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de arqueo inválida del servidor");
}

async function closeCashSession(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    closing_cash_counted: a.closingCashCounted,
  };
  putStr(body, "notes", a.notes);
  const resp = await doFetch(`${b}/api/v1/cash-sessions/${a.id}/close`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de cierre inválida del servidor");
}

export const cashCommands: Record<string, CommandHandler> = {
  cash_sessions: cashSessions,
  open_cash_session: openCashSession,
  cash_arqueo: cashArqueo,
  close_cash_session: closeCashSession,
};
