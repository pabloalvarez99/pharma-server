// DTE / compliance pure helpers — the single source of truth for the logic the
// Boletas and Facturas views duplicated inline (CAF folio tone, estado badge,
// neto/IVA breakdown, 402→upsell note, id/css + date formatting) plus the
// reports "today" picker. Pure (no DOM, no I/O) so it is unit-tested in
// dte.test.ts — that test IS the compliance coverage for neto/IVA, CAF/folio,
// the 402 upsell path, and empty/large data, since the views themselves are
// untestable DOM renderers.
import { desgloseIva } from "./format";
import { parseSaleError } from "./api";

/** Record ids (`dte:abc`) are not valid in CSS selectors — strip to the key. */
export function cssKey(id: string): string {
  return id.replace(/[^a-zA-Z0-9_-]/g, "-");
}

/** es-CL short date+time for a DTE timestamp; echoes the raw string on a bad
 *  date rather than rendering "Invalid Date". */
export function fmtDateCl(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-CL", {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** CAF folio supply → pill tone. `<= 0` is depleted (danger), `<= low` is
 *  running out (warn), otherwise healthy (ok). Shared by both DTE views with
 *  their own `low` thresholds (boleta 50, factura 20). */
export function cafTone(foliosRestantes: number, low: number): "pill-danger" | "pill-warn" | "pill-ok" {
  if (foliosRestantes <= 0) return "pill-danger";
  if (foliosRestantes <= low) return "pill-warn";
  return "pill-ok";
}

/** Estado → badge label + css class. `kind` only switches the gender of the
 *  Spanish label (boleta=Firmada, documento=Firmado). Unknown estados fall
 *  back to the raw value with a neutral pill so a new server estado never
 *  blanks the cell. */
export function estadoBadge(
  estado: string,
  kind: "boleta" | "documento" = "boleta",
): { label: string; cls: string } {
  const fem = kind === "boleta";
  const map: Record<string, { label: string; cls: string }> = {
    draft: { label: "Borrador", cls: "pill" },
    signed: { label: fem ? "Firmada" : "Firmado", cls: "pill pill-ok" },
    sent: { label: fem ? "Enviada SII" : "Enviado SII", cls: "pill pill-warn" },
    accepted: { label: fem ? "Aceptada" : "Aceptado", cls: "pill pill-ok" },
    rejected: { label: fem ? "Rechazada" : "Rechazado", cls: "pill pill-danger" },
    cancelled: { label: fem ? "Anulada" : "Anulado", cls: "pill" },
  };
  return map[estado] ?? { label: estado, cls: "pill" };
}

/** One editable document line as the Facturas form holds it. `cantidad` and
 *  `precio` are raw input strings (parsed defensively here). */
export interface DocLine {
  cantidad: string;
  precio: string;
  exento: boolean;
}

/** Neto/IVA/exento/total mirroring the server's `desglose_iva`
 *  (`crates/dte/src/emit.rs`): each line truncates to integer CLP, the affected
 *  subtotal is split via {@link desgloseIva} (neto + iva == afecto), exento is
 *  summed apart, and total = afecto + exento. Garbage/negative/zero lines are
 *  skipped so a half-typed row never poisons the preview. */
export function computeDocTotals(
  items: ReadonlyArray<DocLine>,
): { neto: number; iva: number; exento: number; total: number } {
  let afecto = 0;
  let exento = 0;
  for (const it of items) {
    const qty = Number(it.cantidad);
    const price = Number(it.precio);
    if (!Number.isFinite(qty) || !Number.isFinite(price) || qty <= 0 || price < 0) continue;
    const monto = Math.trunc(qty * price);
    if (it.exento) exento += monto;
    else afecto += monto;
  }
  const { neto, iva } = desgloseIva(afecto);
  return { neto, iva, exento, total: afecto + exento };
}

/** Map a rejected Tauri command error to the operator-facing note. On a
 *  `FEATURE_REQUIRES_UPGRADE` (402) the message is prefixed with a calm upsell
 *  for `plan` (Pro/Business) — never a crash, never a dark pattern. Returns the
 *  bare server message for every other error. `isUpsell` lets the caller pick a
 *  soft card vs a plain toast. */
export function upgradeNote(
  err: unknown,
  plan: string,
): { isUpsell: boolean; message: string } {
  const { code, message } = parseSaleError(err);
  if (code === "FEATURE_REQUIRES_UPGRADE") {
    return { isUpsell: true, message: `Plan ${plan} requerido para envío automático. ${message}` };
  }
  return { isUpsell: false, message };
}

/** Pick the row whose `date` is today (UTC, YYYY-MM-DD), else the last row the
 *  server returned so a TZ edge or a gap never blanks a KPI card. `undefined`
 *  only when there are no rows at all. */
export function pickToday<T extends { date: string }>(rows: ReadonlyArray<T>): T | undefined {
  if (rows.length === 0) return undefined;
  const today = new Date().toISOString().slice(0, 10);
  return rows.find((r) => r.date === today) ?? rows[rows.length - 1];
}
