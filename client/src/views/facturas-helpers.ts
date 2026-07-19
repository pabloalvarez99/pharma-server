// Pure tax math behind the Facturas emit preview — extracted so the client↔server
// IVA parity (the totals the cashier sees vs the montos the server stamps on the
// DTE) is regression-tested without a DOM. Mirrors `crates/dte/src/emit.rs`
// EXACTLY: per-line `monto = trunc(cantidad * precio_unitario)`, accumulated into
// the afecto/exento bases, then ONE IVA split over the aggregate afecto (the IVA
// absorbs the rounding, neto + IVA == afecto). Splitting per line and summing
// would drift by a peso on multi-line documents — the SII would reject the eco.
import { desgloseIva } from "../format";

/** Minimal shape the totals math reads off a factura item row (a row may still
 *  be half-typed in the live form, so fields are raw strings). */
export interface FacturaTotalsItem {
  cantidad: string;
  precio: string;
  exento: boolean;
}

export interface FacturaTotals {
  neto: number;
  iva: number;
  exento: number;
  total: number;
}

/** Line amount in integer CLP: `trunc(cantidad * precio_unitario)`, matching the
 *  server's `(it.cantidad * it.precio_unitario).trunc()`. Non-numeric / non-positive
 *  qty / negative price yield `null` so the caller skips a still-invalid row
 *  (the preview tolerates partial input instead of reading "NaN"). */
export function lineMonto(cantidad: string, precio: string): number | null {
  const qty = Number(cantidad);
  const price = Number(precio);
  if (!Number.isFinite(qty) || !Number.isFinite(price) || qty <= 0 || price < 0) {
    return null;
  }
  return Math.trunc(qty * price);
}

/** Neto/IVA/exento/total for a set of factura items, mirroring the server's
 *  `build_documento` desglose: sum the per-line trunc'd montos into afecto/exento,
 *  then split IVA ONCE over the aggregate afecto. Invalid/blank rows are skipped. */
export function facturaTotals(items: readonly FacturaTotalsItem[]): FacturaTotals {
  let afecto = 0;
  let exento = 0;
  for (const it of items) {
    const monto = lineMonto(it.cantidad, it.precio);
    if (monto === null) continue;
    if (it.exento) exento += monto;
    else afecto += monto;
  }
  const { neto, iva } = desgloseIva(afecto);
  return { neto, iva, exento, total: afecto + exento };
}
