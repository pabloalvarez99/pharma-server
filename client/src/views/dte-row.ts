import type { Dte } from "../api";
import { clp, fechaHora, num } from "../format";
import { escapeHtml } from "./view-blocks";

const ESTADO_BADGE: Record<string, { feminine: string; masculine: string; cls: string }> = {
  draft: { feminine: "Borrador", masculine: "Borrador", cls: "pill" },
  signed: { feminine: "Firmada", masculine: "Firmado", cls: "pill pill-ok" },
  sent: { feminine: "Enviada SII", masculine: "Enviado SII", cls: "pill pill-warn" },
  accepted: { feminine: "Aceptada", masculine: "Aceptado", cls: "pill pill-ok" },
  rejected: { feminine: "Rechazada", masculine: "Rechazado", cls: "pill pill-danger" },
  cancelled: { feminine: "Anulada", masculine: "Anulado", cls: "pill" },
};

export interface DteRowOptions {
  prefix: "bol" | "fac";
  sendPlan: "Pro" | "Business";
}

/** Render one server-backed DTE row without allowing malformed fields into HTML. */
export function dteRowHtml(d: Dte, { prefix, sendPlan }: DteRowOptions): string {
  const badgeDef = Object.prototype.hasOwnProperty.call(ESTADO_BADGE, d.estado)
    ? ESTADO_BADGE[d.estado]
    : null;
  const badge = badgeDef
    ? { label: prefix === "bol" ? badgeDef.feminine : badgeDef.masculine, cls: badgeDef.cls }
    : { label: d.estado, cls: "pill" };
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
  // Encode each Unicode code point at a fixed width so distinct server IDs
  // cannot collapse to the same DOM id (`a.b` vs `a-b`). The alphabet is
  // limited to CSS-selector-safe characters.
  return `dte-${Array.from(id)
    .map((char) => (char.codePointAt(0) ?? 0).toString(16).padStart(6, "0"))
    .join("-")}`;
}
