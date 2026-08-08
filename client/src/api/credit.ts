// Cuenta corriente / fiado (V1) — wrappers sobre los comandos Tauri
// (client/src-tauri/src/commands/credit.rs). El CARGO lo genera el POS al vender
// con `pos_fiado`; acá se lee la cuenta y se registran abonos.
import { invoke } from "@tauri-apps/api/core";

/** Un movimiento inmutable del ledger. `amount` es STRING (Decimal). */
export interface LedgerEntry {
  id: string;
  /** `cargo` (el cliente debe) | `abono` (pagó). */
  kind: string;
  amount: string;
  order?: string | null;
  note?: string | null;
  created_at: string;
}

/** Estado de cuenta: saldo + totales + movimientos (recientes primero). */
export interface CustomerAccount {
  customer: string;
  /** `total_charged - total_paid`. Positivo = el cliente debe. */
  balance: string;
  total_charged: string;
  total_paid: string;
  entries: LedgerEntry[];
}

/** GET /api/v1/customers/{id}/cuenta (cashier+). */
export function customerAccount(serverUrl: string, id: string): Promise<CustomerAccount> {
  return invoke<CustomerAccount>("customer_account", { serverUrl, id });
}

/** POST /api/v1/customers/{id}/abono (cashier+). `amount` STRING. Rechaza en
 *  español si el abono supera la deuda o el cliente no debe nada. */
export function recordAbono(
  serverUrl: string,
  id: string,
  amount: string,
  opts: { cashSession?: string; note?: string } = {},
): Promise<LedgerEntry> {
  return invoke<LedgerEntry>("record_abono", {
    serverUrl,
    id,
    amount,
    cashSession: opts.cashSession,
    note: opts.note,
  });
}

/** Un cliente con deuda vigente. `balance` STRING (Decimal). */
export interface DebtorRow {
  customer: string;
  name: string;
  phone?: string | null;
  balance: string;
  /** Último movimiento (fiado o abono) — si la deuda está viva o dormida. */
  last_movement: string;
}

/** "¿Cuánto me deben?" — cuentas por cobrar del negocio. */
export interface DebtorsReport {
  total_por_cobrar: string;
  debtor_count: number;
  /** Deudores ordenados por saldo, mayor primero. */
  rows: DebtorRow[];
}

/** GET /api/v1/reports/por-cobrar (cashier+). */
export function debtorsReport(serverUrl: string): Promise<DebtorsReport> {
  return invoke<DebtorsReport>("debtors_report", { serverUrl });
}
