import type { Dte } from "../api";
import { clp, fechaHora, num } from "../format";
import { escapeHtml } from "./view-blocks";

const ESTADO_BADGE: Record<string, { label: string; cls: string }> = {
  draft: { label: "Borrador", cls: "pill" },
  signed: { label: "Firmado", cls: "pill pill-ok" },
  sent: { label: "Enviado SII", cls: "pill pill-warn" },
  accepted: { label: "Aceptado", cls: "pill pill-ok" },
  rejected: { label: "Rechazado", cls: "pill pill-danger" },
  cancelled: { label: "Anulado", cls: "pill" },
};

export interface DteRowOptions {
  prefix: "bol" | "fac";
  sendPlan: "Pro" | "Business";
}

/** Render one server-backed DTE row without allowing malformed fields into HTML. */
export function dteRowHtml(d: Dte, { prefix, sendPlan }: DteRowOptions): string {
  const badge = ESTADO_BADGE[d.estado] ?? { label: d.estado, cls: "pill" };
  const sii = d.track_id !== null
    ? `track ${escapeHtml(String(d.track_id))}${d.sii_glosa ? ` · ${escapeHtml(d.sii_glosa)}` : ""}`
    : "—";
  const k = dteCssKey(d.id);
  const actions: string[] = [];
  if (d.has_xml) {
    actions.push(`<button class="btn-ghost rb-btn ghost" id="${prefix}-xml-${k}" title="Descargar XML firmado">XML</button>`);
  }
  if (d.estado === "signed") {
    actions.push(`<button class="btn-ghost rb-btn ghost" id="${prefix}-send-${k}" title="Enviar al SII (requiere plan ${sendPlan})">Enviar SII</button>`);
    actions.push(`<button class="btn-ghost-danger" id="${prefix}-cancel-${k}" title="Anular antes de enviar">Anular</button>`);
  }
  if (d.estado === "sent") {
    actions.push(`<button class="btn-ghost rb-btn ghost" id="${prefix}-poll-${k}" title="Consultar veredicto del SII">Consultar</button>`);
  }
  return `
    <tr data-dte="${escapeHtml(d.id)}">
      <td class="num">${num(d.folio)}</td>
      <td>${escapeHtml(fechaHora(d.fecha_emision))}</td>
      <td><div class="cell-main">${escapeHtml(d.razon_social_receptor)}</div><div class="muted">${escapeHtml(d.rut_receptor)}</div></td>
      <td class="num">${clp(d.monto_total)}</td>
      <td><span class="${escapeHtml(badge.cls)}">${escapeHtml(badge.label)}</span></td>
      <td class="muted">${sii}</td>
      <td class="bol-actions">${actions.join("") || "—"}</td>
    </tr>
  `;
}

export function dteCssKey(id: string): string {
  return id.replace(/[^a-zA-Z0-9_-]/g, "-");
}
