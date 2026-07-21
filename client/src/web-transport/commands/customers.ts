// Web port of commands/customers.rs. Every command maps a 404 to the stable
// CUSTOMERS_MODULE_MISSING sentinel so the Clientes view soft-degrades exactly
// like on desktop.

import { base, errorMessage } from "../errors";
import {
  type CommandArgs,
  type CommandHandler,
  authHeaders,
  doFetch,
  JSON_HEADERS,
  parseJson,
  putDef,
  putStr,
  qs,
} from "../core";

const CUSTOMERS_MISSING = "CUSTOMERS_MODULE_MISSING";

async function checkCustomers(resp: Response): Promise<void> {
  if (resp.status === 404) throw CUSTOMERS_MISSING;
  if (!resp.ok) throw await errorMessage(resp);
}

async function customerSearch(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  // The desktop forwards `q` verbatim (no empty filter) — same here.
  const u = new URLSearchParams({ q: String(a.q ?? "") });
  const resp = await doFetch(`${b}/api/v1/customers/search?${u.toString()}`, {
    headers: authHeaders(),
  });
  await checkCustomers(resp);
  return parseJson(resp, "Respuesta de clientes inválida del servidor");
}

async function customerDetail(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/customers/${a.id}`, { headers: authHeaders() });
  await checkCustomers(resp);
  return parseJson(resp, "Respuesta de cliente inválida del servidor");
}

async function createCustomer(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = { name: a.name };
  putStr(body, "rut", a.rut);
  putStr(body, "phone", a.phone);
  putStr(body, "email", a.email);
  const resp = await doFetch(`${b}/api/v1/clientes`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  await checkCustomers(resp);
  return parseJson(resp, "Respuesta de cliente inválida del servidor");
}

async function updateCustomer(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  // NOTE: unlike create, update forwards provided strings VERBATIM (an empty
  // string clears the field server-side) — mirrors the Rust `if let Some(s)`.
  const body: Record<string, unknown> = {};
  for (const k of ["name", "rut", "phone", "email"] as const) {
    if (typeof a[k] === "string") body[k] = a[k];
  }
  putDef(body, "active", a.active);
  const resp = await doFetch(`${b}/api/v1/clientes/${a.id}`, {
    method: "PATCH",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  await checkCustomers(resp);
  return parseJson(resp, "Respuesta de cliente inválida del servidor");
}

async function customerHistory(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/customers/${a.id}/history${qs({ limit: a.limit })}`,
    { headers: authHeaders() },
  );
  await checkCustomers(resp);
  return parseJson(resp, "Respuesta de historial inválida del servidor");
}

export const customerCommands: Record<string, CommandHandler> = {
  customer_search: customerSearch,
  customer_detail: customerDetail,
  create_customer: createCustomer,
  update_customer: updateCustomer,
  customer_history: customerHistory,
};
