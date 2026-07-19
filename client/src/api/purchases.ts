// Purchasing wrappers: purchase orders, suppliers, payments
// (client/src-tauri/src/commands/purchases.rs).
import { invoke } from "@tauri-apps/api/core";

/** Header-only projection of a purchase order (`PurchaseOrderDto`). `total`
 *  is a STRING (Decimal). `items` is omitted — list returns headers only. */
export interface PurchaseOrder {
  id: string;
  supplier: string;
  status: string; // "draft" | "sent" | "received" | "partial" | "cancelled"
  currency: string;
  total: string;
  notes: string | null;
  external_ref: string | null;
  created_at: string;
  updated_at: string;
}

/** GET /api/v1/purchase-orders (Bearer, cashier+). `status` / `limit` optional. */
export function listPurchaseOrders(
  serverUrl: string,
  status?: string,
  limit?: number,
): Promise<PurchaseOrder[]> {
  return invoke<PurchaseOrder[]>("list_purchase_orders", { serverUrl, status, limit });
}

/** One line of a purchase order (`PurchaseOrderItemDto`). `unit_cost`/`subtotal`
 *  are STRINGS (Decimal). `qty_received` is the cumulative quantity already
 *  received against this line — `quantity - qty_received` is what's left. */
export interface PurchaseOrderItem {
  id: string;
  product: string | null;
  product_name: string;
  quantity: number;
  qty_received: number;
  unit_cost: string;
  subtotal: string;
}

/** Full purchase order WITH line items — the `GET /purchase-orders/{id}` shape.
 *  Same header as {@link PurchaseOrder} plus the populated `items`. */
export interface PurchaseOrderDetail {
  id: string;
  supplier: string;
  status: string;
  currency: string;
  total: string;
  notes: string | null;
  external_ref: string | null;
  items: PurchaseOrderItem[];
  created_at: string;
  updated_at: string;
}

/** One line for {@link createPurchaseOrder}. Field names are snake_case: serde
 *  deserializes the array elements directly (no camelCase conversion — only the
 *  top-level command args are converted). `product` omitted ⇒ free-text line;
 *  `unit_cost` is a STRING (Decimal). */
export interface NewPurchaseOrderItem {
  product?: string;
  product_name: string;
  quantity: number;
  unit_cost: string;
}

/** One line for {@link receivePurchaseOrder}. snake_case for the same reason as
 *  {@link NewPurchaseOrderItem}. `po_line_id` is the {@link PurchaseOrderItem} id. */
export interface ReceiveLine {
  po_line_id: string;
  qty_received: number;
}

/** GET /api/v1/purchase-orders/{id} (Bearer, cashier+) — full PO with items. */
export function getPurchaseOrder(serverUrl: string, id: string): Promise<PurchaseOrderDetail> {
  return invoke<PurchaseOrderDetail>("get_purchase_order", { serverUrl, id });
}

/** POST /api/v1/purchase-orders (Bearer, admin+) — create a draft PO. The server
 *  computes per-line subtotals + header total and defaults `currency` to CLP. */
export function createPurchaseOrder(
  serverUrl: string,
  supplier: string,
  items: NewPurchaseOrderItem[],
  currency?: string,
  notes?: string,
  externalRef?: string,
): Promise<PurchaseOrder> {
  return invoke<PurchaseOrder>("create_purchase_order", {
    serverUrl,
    supplier,
    items,
    currency,
    notes,
    externalRef,
  });
}

/** POST /api/v1/purchase-orders/{id}/send (Bearer, admin+) — issue a draft PO to
 *  the supplier (draft → sent). Only legal from `draft` (any other status → 409).
 *  Sending is the bridge that makes a PO receivable: a draft cannot be received
 *  directly (409), so the operator must send it first. Returns the updated PO. */
export function sendPurchaseOrder(serverUrl: string, id: string): Promise<PurchaseOrder> {
  return invoke<PurchaseOrder>("send_purchase_order", { serverUrl, id });
}

/** POST /api/v1/purchase-orders/{id}/receive (Bearer, admin+) — goods receipt.
 *  Bumps stock, recomputes weighted-average cost, advances each line's
 *  `qty_received`. Only legal from `sent`/`approved`/`partial` (a draft → 409). */
export function receivePurchaseOrder(
  serverUrl: string,
  id: string,
  lines: ReceiveLine[],
  notes?: string,
): Promise<PurchaseOrder> {
  return invoke<PurchaseOrder>("receive_purchase_order", { serverUrl, id, lines, notes });
}

/** One recorded supplier payment against a PO (`PurchasePaymentDto`). Money
 *  (`amount`) is a STRING (Decimal); dates are RFC3339. */
export interface PurchasePayment {
  id: string;
  purchase_order: string;
  amount: string;
  currency: string;
  payment_method: string; // "cash" | "bank" | "card" | "transfer"
  cash_session: string | null;
  reference: string | null;
  note: string | null;
  paid_at: string;
  created_at: string;
}

/** Accounts-payable rollup of a PO + its payments (`PurchasePaymentSummary`).
 *  `total`/`paid`/`balance` are STRING Decimals. */
export interface PurchasePaymentSummary {
  purchase_order: string;
  status: string;
  total: string;
  paid: string;
  balance: string;
  fully_paid: boolean;
  payments: PurchasePayment[];
}

/** GET /api/v1/purchase-orders/{id}/payments (Bearer, cashier+) — AP summary. */
export function getPoPayments(serverUrl: string, id: string): Promise<PurchasePaymentSummary> {
  return invoke<PurchasePaymentSummary>("get_po_payments", { serverUrl, id });
}

/** POST /api/v1/purchase-orders/{id}/payments (Bearer, admin+) — record a
 *  supplier payment. `amount` is a Decimal string; `cashSession` is required by
 *  the server when paying `cash` with an open drawer. */
export function createPoPayment(
  serverUrl: string,
  id: string,
  args: { amount: string; paymentMethod?: string; cashSession?: string; reference?: string },
): Promise<PurchasePayment> {
  return invoke<PurchasePayment>("create_po_payment", {
    serverUrl,
    id,
    amount: args.amount,
    paymentMethod: args.paymentMethod,
    cashSession: args.cashSession,
    reference: args.reference,
  });
}

/** A supplier (`domain::purchasing::model::SupplierDto`). No money fields. */
export interface Supplier {
  id: string;
  name: string;
  rut: string | null;
  contact_name: string | null;
  contact_email: string | null;
  contact_phone: string | null;
  default_invoice_format: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** GET /api/v1/suppliers (Bearer, cashier+). `search`/`limit` optional. */
export function listSuppliers(
  serverUrl: string,
  search?: string,
  limit?: number,
): Promise<Supplier[]> {
  return invoke<Supplier[]>("list_suppliers", { serverUrl, search, limit });
}

/** POST /api/v1/suppliers (Bearer) — register a supplier. `name` required;
 *  contact fields optional (empty dropped server-side). */
export function createSupplier(
  serverUrl: string,
  name: string,
  rut?: string,
  contactName?: string,
  contactEmail?: string,
  contactPhone?: string,
): Promise<Supplier> {
  return invoke<Supplier>("create_supplier", {
    serverUrl,
    name,
    rut,
    contactName,
    contactEmail,
    contactPhone,
  });
}
