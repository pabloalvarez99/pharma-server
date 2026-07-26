// Libro de compras + resumen IVA (F29) — wrappers sobre los comandos Tauri
// (client/src-tauri/src/commands/compliance.rs). El libro se arma con las OC
// recepcionadas del período; mientras el operador no capture la factura real del
// proveedor, neto/IVA se derivan del total (19%) y la fila queda `declared:false`.
import { invoke } from "@tauri-apps/api/core";

/** Una fila del libro: un documento de proveedor. Money STRING. */
export interface PurchaseBookRow {
  purchase_order: string;
  /** Tipo DTE: 33 factura afecta, 34 exenta, 61 NC, 56 ND. */
  tipo: number;
  folio?: string | null;
  supplier_name: string;
  supplier_rut?: string | null;
  date: string;
  neto: string;
  iva: string;
  total: string;
  /** `false` = derivado del total; falta capturar la factura real. */
  declared: boolean;
}

export interface PurchaseBook {
  period: string;
  rows: PurchaseBookRow[];
  total_neto: string;
  total_iva: string;
  total: string;
  pending_declaration: number;
}

/** Débito (ventas) − crédito (compras) = IVA a pagar del período. */
export interface IvaSummary {
  period: string;
  iva_debito: string;
  iva_credito: string;
  /** Positivo = a pagar; negativo = remanente a favor. */
  iva_a_pagar: string;
  ventas_neto: string;
  compras_neto: string;
}

/** GET /api/v1/reports/libro-compras (admin+). Sin `period` → mes en curso. */
export function libroCompras(serverUrl: string, period?: string): Promise<PurchaseBook> {
  return invoke<PurchaseBook>("libro_compras", { serverUrl, period });
}

/** GET /api/v1/reports/iva (admin+). Sin `period` → mes en curso. */
export function ivaSummary(serverUrl: string, period?: string): Promise<IvaSummary> {
  return invoke<IvaSummary>("iva_summary", { serverUrl, period });
}

/** PATCH /api/v1/purchase-orders/{id}/factura (admin+) — capturar el documento
 *  del proveedor. Devuelve el libro del período ya actualizado. */
export function setPoInvoice(
  serverUrl: string,
  id: string,
  invoice: {
    folio?: string;
    date?: string;
    tipo?: number;
    neto?: string;
    iva?: string;
    total?: string;
  },
): Promise<PurchaseBook> {
  return invoke<PurchaseBook>("set_po_invoice", { serverUrl, id, ...invoice });
}
