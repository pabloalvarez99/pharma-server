// Web port of the small single-command domains: reports.rs, expenses.rs,
// prescriptions.rs, settings.rs, audit.rs, seed.rs, rubro.rs, license.rs,
// assist.rs. Grouped here to keep the shim tree shallow; each block mirrors its
// Rust module 1:1.

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

// --- reports.rs -------------------------------------------------------------

async function salesDaily(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/reports/sales-daily`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de ventas inválida del servidor");
}

async function topProducts(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/reports/top-products${qs({ limit: a.limit })}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de ranking inválida del servidor");
}

async function marginsDaily(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetchCoded(
    `${b}/api/v1/reports/margins-daily${qs({ from: a.from, to: a.to })}`,
    { headers: authHeadersCoded() },
  );
  // Non-2xx (incl. 402 FEATURE_REQUIRES_UPGRADE) → coded "CODE|message".
  if (!resp.ok) throw await codedError(resp);
  return parseJson(resp, "|Respuesta de márgenes inválida del servidor");
}

async function stockRotation(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/reports/stock-rotation${qs({ from: a.from, to: a.to })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de rotación inválida del servidor");
}

async function dashboardReport(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/reports/dashboard`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta del panel inválida del servidor");
}

// --- compliance.rs (libro de compras + IVA F29) -----------------------------

async function libroCompras(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/reports/libro-compras${qs({ period: a.period })}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de libro de compras inválida del servidor");
}

async function ivaSummary(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/reports/iva${qs({ period: a.period })}`, {
    headers: authHeaders(),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de IVA inválida del servidor");
}

async function setPoInvoice(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {};
  putStr(body, "folio", a.folio);
  putStr(body, "date", a.date);
  putDef(body, "tipo", a.tipo);
  putStr(body, "neto", a.neto);
  putStr(body, "iva", a.iva);
  putStr(body, "total", a.total);
  const resp = await doFetch(`${b}/api/v1/purchase-orders/${a.id}/factura`, {
    method: "PATCH",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de factura inválida del servidor");
}

// --- expenses.rs ------------------------------------------------------------

async function listExpenses(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/expenses${qs({ category: a.category, payment_method: a.paymentMethod, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de gastos inválida del servidor");
}

async function createExpense(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    category: a.category,
    description: a.description,
    amount: a.amount,
  };
  putStr(body, "payment_method", a.paymentMethod);
  putStr(body, "note", a.note);
  putStr(body, "incurred_at", a.incurredAt);
  const resp = await doFetch(`${b}/api/v1/expenses`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de gasto inválida del servidor");
}

// --- prescriptions.rs -------------------------------------------------------

async function listPrescriptions(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/prescriptions${qs({ patient_rut: a.patientRut, controlled: a.controlled, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de recetas inválida del servidor");
}

async function getPrescription(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/prescriptions/${a.id}`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de receta inválida del servidor");
}

async function createPrescription(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const body: Record<string, unknown> = {
    patient_name: a.patientName,
    patient_rut: a.patientRut,
    controlled: a.controlled,
  };
  putStr(body, "doctor_name", a.doctorName);
  putStr(body, "doctor_rut", a.doctorRut);
  putStr(body, "product", a.product);
  putStr(body, "customer", a.customer);
  putStr(body, "folio", a.folio);
  const resp = await doFetch(`${b}/api/v1/prescriptions`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify(body),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de receta inválida del servidor");
}

async function libroRecetas(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/libro-recetas${qs({ patient_rut: a.patientRut, limit: a.limit })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta del libro de recetas inválida del servidor");
}

async function exportLibroRecetas(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/libro-recetas/export${qs({ patient_rut: a.patientRut })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseText(resp, "Respuesta de exportación inválida del servidor");
}

// --- settings.rs ------------------------------------------------------------

async function getSetting(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/settings/${a.key}`, { headers: authHeaders() });
  if (resp.status === 404) return null; // unset key → default/empty state
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de configuración inválida del servidor");
}

async function setSetting(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/settings/${a.key}`, {
    method: "PUT",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify({ value: a.value }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de configuración inválida del servidor");
}

// --- audit.rs ---------------------------------------------------------------

async function queryAuditLog(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(
    `${b}/api/v1/admin/audit-log${qs({
      from: a.from,
      to: a.to,
      user: a.user,
      table: a.table,
      action: a.action,
      limit: a.limit,
      offset: a.offset,
    })}`,
    { headers: authHeaders() },
  );
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de auditoría inválida del servidor");
}

// --- seed.rs ----------------------------------------------------------------

async function seedDemo(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/admin/seed-demo`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify({ vertical: a.vertical, force: a.force }),
  });
  if (resp.status === 409) throw "SEED_ALREADY_EXISTS";
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de seed inválida del servidor");
}

// --- rubro.rs ---------------------------------------------------------------

async function rubroPack(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/rubro-pack`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de pack de rubro inválida del servidor");
}

// --- license.rs -------------------------------------------------------------

async function licenseStatus(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/admin/license/status`, { headers: authHeaders() });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta de licencia inválida del servidor");
}

// --- assist.rs --------------------------------------------------------------

async function assistAsk(a: CommandArgs): Promise<unknown> {
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/assist/ask`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify({ question: a.question }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta del agente inválida del servidor");
}

async function assistAct(a: CommandArgs): Promise<unknown> {
  const confirmToken = String(a.confirmToken ?? "");
  if (confirmToken.trim() === "") throw "No hay una acción para confirmar.";
  const b = base(a.serverUrl as string);
  const resp = await doFetch(`${b}/api/v1/assist/act`, {
    method: "POST",
    headers: authHeaders(JSON_HEADERS),
    body: JSON.stringify({ confirm_token: confirmToken }),
  });
  if (!resp.ok) throw await errorMessage(resp);
  return parseJson(resp, "Respuesta del agente inválida del servidor");
}

export const miscCommands: Record<string, CommandHandler> = {
  libro_compras: libroCompras,
  iva_summary: ivaSummary,
  set_po_invoice: setPoInvoice,
  sales_daily: salesDaily,
  top_products: topProducts,
  margins_daily: marginsDaily,
  stock_rotation: stockRotation,
  dashboard_report: dashboardReport,
  list_expenses: listExpenses,
  create_expense: createExpense,
  list_prescriptions: listPrescriptions,
  get_prescription: getPrescription,
  create_prescription: createPrescription,
  libro_recetas: libroRecetas,
  export_libro_recetas: exportLibroRecetas,
  get_setting: getSetting,
  set_setting: setSetting,
  query_audit_log: queryAuditLog,
  seed_demo: seedDemo,
  rubro_pack: rubroPack,
  license_status: licenseStatus,
  assist_ask: assistAsk,
  assist_act: assistAct,
};
