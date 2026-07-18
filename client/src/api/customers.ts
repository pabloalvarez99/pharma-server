// Customers / clientes wrappers (client/src-tauri/src/commands/customers.rs).
import { invoke } from "@tauri-apps/api/core";

/** A customer search result (`CustomerDto`). */
export interface Customer {
  id: string;
  name: string;
  rut: string | null;
  phone: string | null;
  email: string | null;
  loyalty_points: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** Customer detail w/ lifetime aggregates (`CustomerDetailDto`). `total_spent`
 *  is a STRING (Decimal). Served by feat/customers-loyalty-history. */
export interface CustomerDetail {
  id: string;
  name: string;
  rut: string | null;
  phone: string | null;
  email: string | null;
  loyalty_points: number;
  total_spent: string;
  visit_count: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** One purchase-history row (`CustomerOrderDto`). `total` is a STRING (Decimal). */
export interface CustomerOrder {
  id: string;
  status: string;
  payment_method: string;
  total: string;
  items_count: number;
  created_at: string;
}

/** Sentinel the customer commands reject with when the server lacks the
 *  `/customers/*` surface (404). Matches `CUSTOMERS_MISSING` in
 *  commands/customers.rs — the Clientes view shows an upgrade note instead of a
 *  hard error. */
export const CUSTOMERS_MODULE_MISSING = "CUSTOMERS_MODULE_MISSING";

/** GET /api/v1/customers/search?q= (Bearer). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when the endpoint is not deployed (404). */
export function customerSearch(
  serverUrl: string,
  q: string,
): Promise<Customer[]> {
  return invoke<Customer[]>("customer_search", { serverUrl, q });
}

/** GET /api/v1/customers/{id} (Bearer). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when not deployed (404). */
export function customerDetail(
  serverUrl: string,
  id: string,
): Promise<CustomerDetail> {
  return invoke<CustomerDetail>("customer_detail", { serverUrl, id });
}

/** GET /api/v1/customers/{id}/history?limit=N (Bearer). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when not deployed (404). */
export function customerHistory(
  serverUrl: string,
  id: string,
  limit?: number,
): Promise<CustomerOrder[]> {
  return invoke<CustomerOrder[]>("customer_history", { serverUrl, id, limit });
}

/** Fields for creating / editing a customer. `name` is required on create; on
 *  edit every field is optional (only the ones set are sent). */
export interface CustomerInput {
  name?: string;
  rut?: string;
  phone?: string;
  email?: string;
  active?: boolean;
}

/** POST /api/v1/clientes (Bearer, cashier+) — register a new customer. Empty
 *  optional fields are dropped server-side (stored null). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when the module is not deployed (404). */
export function createCustomer(
  serverUrl: string,
  name: string,
  rut?: string,
  phone?: string,
  email?: string,
): Promise<Customer> {
  return invoke<Customer>("create_customer", {
    serverUrl,
    name,
    rut,
    phone,
    email,
  });
}

/** PATCH /api/v1/clientes/{id} (Bearer, cashier+) — edit a customer. Only the
 *  provided fields are forwarded; `active` toggles activar/desactivar. Rejects
 *  with {@link CUSTOMERS_MODULE_MISSING} when not deployed (404). */
export function updateCustomer(
  serverUrl: string,
  id: string,
  patch: CustomerInput,
): Promise<Customer> {
  return invoke<Customer>("update_customer", {
    serverUrl,
    id,
    name: patch.name,
    rut: patch.rut,
    phone: patch.phone,
    email: patch.email,
    active: patch.active,
  });
}
