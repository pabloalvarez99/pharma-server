// POS wrappers: sales, receipts, refunds/devoluciones
// (client/src-tauri/src/commands/pos.rs).
import { invoke } from "@tauri-apps/api/core";

/** One cart line for `pos_sale`. `unit_price` is a STRING per server contract. */
export interface PosItem {
  product: string;
  product_name: string;
  quantity: number;
  unit_price: string;
}

/** POS payment methods the server accepts on the counter. `pos_mixed` is a
 *  split tender (cash + card) — for it `posSale` sends both `cashAmount` and
 *  `cardAmount` and the server only requires their sum to cover the total. */
/** `pos_fiado` = venta a cuenta corriente (fiado): exige `customer`, NO mueve
 *  caja y genera un cargo en el ledger del cliente (server-side). */
export type PaymentMethod =
  | "pos_cash"
  | "pos_debit"
  | "pos_credit"
  | "pos_mixed"
  | "pos_fiado";

/** A POS sale error surfaced from the Tauri layer as `"CODE|message"`. */
export interface SaleError {
  code: string; // "" when the server sent no envelope
  message: string;
}

/** Split the `"CODE|message"` string the `pos_sale` command rejects with. */
export function parseSaleError(err: unknown): SaleError {
  const raw = typeof err === "string" ? err : "Error inesperado al cobrar.";
  const i = raw.indexOf("|");
  if (i === -1) return { code: "", message: raw };
  return { code: raw.slice(0, i), message: raw.slice(i + 1) };
}

/** A low-stock alert the server attaches to a sale response (`LowStockAlert`). */
export interface LowStockAlert {
  product: string;
  product_name: string;
  stock: number;
  threshold: number;
}

/** Raw shape of the `pos_sale` JSON we actually read (the server returns more). */
interface RawSaleResponse {
  order?: { id?: string };
  loyalty_points_awarded?: number;
  low_stock_alerts?: LowStockAlert[];
}

/** Narrowed result of a successful sale: the order id (for the receipt fetch),
 *  loyalty points awarded, and any low-stock alerts to surface afterwards. */
export interface PosSaleResult {
  orderId: string;
  loyaltyPointsAwarded: number;
  lowStockAlerts: LowStockAlert[];
}

/** POST /api/v1/pos/sale (Bearer + fresh Idempotency-Key minted in Rust).
 *  `customer` is an optional record id — when present the server links the sale
 *  and awards loyalty points. `branch` es la sucursal activa: el stock se
 *  descuenta de ESE local (V2); ausente = el server usa la sucursal de la caja
 *  abierta y, si no hay, la casa matriz. Rejects with a `"CODE|message"` string
 *  — use {@link parseSaleError}. */
export async function posSale(
  serverUrl: string,
  items: PosItem[],
  paymentMethod: PaymentMethod,
  cashAmount?: string,
  cardAmount?: string,
  customer?: string,
  discount?: string,
  branch?: string,
): Promise<PosSaleResult> {
  const res = await invoke<RawSaleResponse>("pos_sale", {
    serverUrl,
    items,
    paymentMethod,
    cashAmount,
    cardAmount,
    customer,
    discount,
    branch,
  });
  return {
    orderId: res?.order?.id ?? "",
    loyaltyPointsAwarded: res?.loyalty_points_awarded ?? 0,
    lowStockAlerts: Array.isArray(res?.low_stock_alerts) ? res.low_stock_alerts : [],
  };
}

/** One printable receipt line (`ReceiptItem`). Money fields are STRINGS. */
export interface ReceiptItem {
  name: string;
  qty: number;
  unit_price: string;
  line_total: string;
}

/** Printable boleta for a completed sale (`ReceiptDto`). Money is STRING;
 *  `cash_amount`/`card_amount`/`change` are null on tenders they don't apply to. */
export interface Receipt {
  order_id: string;
  folio_or_number: string;
  datetime: string;
  tenant_name: string;
  items: ReceiptItem[];
  subtotal: string;
  discount: string;
  total: string;
  payment_method: string;
  cash_amount: string | null;
  card_amount: string | null;
  change: string | null;
  loyalty_points_awarded: number;
  cashier: string | null;
  footer_note: string;
}

/** GET /api/v1/orders/{id}/receipt (Bearer) — boleta for a completed sale. */
export function getReceipt(serverUrl: string, id: string): Promise<Receipt> {
  return invoke<Receipt>("get_receipt", { serverUrl, id });
}

// --- devoluciones / returns ------------------------------------------------

/** A refund/devolución header (`DevolucionDto`). `total_devuelto` is a STRING
 *  (Decimal). `order` is the linked sale (null for a standalone refund). */
export interface Devolucion {
  id: string;
  order: string | null;
  tipo: string;
  motivo: string;
  notas: string | null;
  total_devuelto: string;
  metodo_reembolso: string | null;
  procesado_por: string | null;
  created_at: string;
}

/** One refund line for {@link createRefund}. snake_case (serde deserializes the
 *  array elements directly — no camelCase rename, same as `PosItem`).
 *  `unit_price` STRING; `restock` returns the unit to stock — the server REQUIRES
 *  `product` to be set when `restock` is true, so receipt-driven lines (which
 *  carry no product id) must send `restock: false`. */
export interface RefundItem {
  product?: string;
  product_name: string;
  quantity: number;
  unit_price: string;
  restock: boolean;
}

/** Narrowed result of a successful refund: the devolución header + whether the
 *  linked order was flipped to `refunded`. */
export interface RefundResult {
  devolucion: Devolucion;
  orderMarkedRefunded: boolean;
}

/** POST /api/v1/pos/returns (Bearer, cashier+) — create a refund. `tipo` is the
 *  return MOTIVO and must be a value the `devolucion.tipo` schema accepts
 *  (`venta` | `cancelacion` | `garantia` | `error`); defaults to `venta` (a
 *  normal return against a sale). It is NOT the total/parcial scope — that is a
 *  presentational distinction, never persisted on this field. Sending an invalid
 *  tipo (e.g. `total`/`parcial`) made the server reject every return with a 500
 *  (BUG-paul-001). Rejects with a Spanish string (e.g. over-refund / 403). */
export async function createRefund(
  serverUrl: string,
  motivo: string,
  items: RefundItem[],
  opts: { order?: string; tipo?: string; notas?: string; metodoReembolso?: string } = {},
): Promise<RefundResult> {
  const res = await invoke<{ devolucion: Devolucion; order_marked_refunded?: boolean }>(
    "create_refund",
    {
      serverUrl,
      order: opts.order,
      tipo: opts.tipo ?? "venta",
      motivo,
      notas: opts.notas,
      items,
      metodoReembolso: opts.metodoReembolso,
    },
  );
  return {
    devolucion: res.devolucion,
    orderMarkedRefunded: res.order_marked_refunded ?? false,
  };
}

/** GET /api/v1/returns (Bearer). Optional `order` / `tipo` / `limit` filters. */
export function listRefunds(
  serverUrl: string,
  order?: string,
  tipo?: string,
  limit?: number,
): Promise<Devolucion[]> {
  return invoke<Devolucion[]>("list_refunds", { serverUrl, order, tipo, limit });
}
