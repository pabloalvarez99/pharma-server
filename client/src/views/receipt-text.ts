// Plain-text rendering of a completed-sale boleta. Lives apart from pos.ts (no
// DOM, no CSS) so the cashier "compartir recibo" path is regression-tested
// headless and reused verbatim by the clipboard copy + share affordances.
//
// The goal is a ticket a real cashier can paste into WhatsApp/email: readable
// without a monospace font, money via the same `clp` the on-screen receipt uses,
// vuelto always explicit for cash AND mixed tenders (F-paul-pay-001).
import { clp, num, toNumber } from "../format";
import type { Receipt } from "../api";

const METHOD_LABEL: Record<string, string> = {
  pos_cash: "Efectivo",
  pos_debit: "Débito",
  pos_credit: "Crédito",
  pos_mixed: "Mixto",
};

/** Human date for the ticket; falls back to the raw string if unparseable. */
function when(datetime: string): string {
  const d = new Date(datetime);
  return Number.isNaN(d.getTime()) ? datetime : d.toLocaleString("es-CL");
}

/** A pasteable boleta. Pure: same input → same text, no DOM, no locale traps
 *  beyond `clp`/`toLocaleString` (shared with the rendered receipt). */
export function receiptText(r: Receipt): string {
  const lines: string[] = [];
  lines.push(r.tenant_name);
  lines.push(`Boleta ${r.folio_or_number} · ${when(r.datetime)}`);
  if (r.cashier) lines.push(`Cajero: ${r.cashier}`);
  lines.push("");

  for (const it of r.items) {
    lines.push(`${num(it.qty)}x ${it.name}  ${clp(it.line_total)}`);
  }
  lines.push("");

  lines.push(`Subtotal: ${clp(r.subtotal)}`);
  if (toNumber(r.discount) > 0) lines.push(`Descuento: -${clp(r.discount)}`);
  lines.push(`TOTAL: ${clp(r.total)}`);
  lines.push(`Pago: ${METHOD_LABEL[r.payment_method] ?? r.payment_method}`);

  // Tender breakdown + vuelto — explicit for cash and mixed alike.
  if (r.payment_method === "pos_cash" && r.cash_amount) {
    lines.push(`Recibido: ${clp(r.cash_amount)}`);
    lines.push(`Vuelto: ${clp(r.change ?? "0")}`);
  } else if (r.payment_method === "pos_mixed") {
    lines.push(`Efectivo: ${clp(r.cash_amount ?? "0")}`);
    lines.push(`Tarjeta: ${clp(r.card_amount ?? "0")}`);
    lines.push(`Vuelto: ${clp(r.change ?? "0")}`);
  }

  if (r.loyalty_points_awarded > 0) {
    lines.push(`Puntos ganados: +${num(r.loyalty_points_awarded)}`);
  }

  if (r.footer_note) {
    lines.push("");
    lines.push(r.footer_note);
  }

  return lines.join("\n");
}
