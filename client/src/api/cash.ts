// Caja / cash register wrappers (client/src-tauri/src/commands/cash.rs).
import { invoke } from "@tauri-apps/api/core";

/** A cash register session (`/cash-sessions`). Money fields are STRINGS
 *  (Decimal). The closing/discrepancy fields are null while `status === "open"`. */
export interface CashSession {
  id: string;
  user: string;
  register_name: string;
  opening_cash: string;
  opening_notes: string | null;
  closing_cash_counted: string | null;
  closing_cash_expected: string | null;
  discrepancia: string | null;
  closing_notes: string | null;
  opened_at: string;
  closed_at: string | null;
  status: string; // "open" | "closed"
  /** Caja física y sucursal donde se abrió (V2). Ausentes en sesiones previas
   *  a la migración 0041 y en cajas sueltas de un negocio de un solo local. */
  register?: string | null;
  branch?: string | null;
}

/** Close summary / arqueo payload (`CloseSummary`). Money fields are STRINGS. */
export interface CashCloseSummary {
  session: CashSession;
  cash_sales: string;
  movements_in: string;
  movements_out: string;
}

/** GET /api/v1/cash-sessions (Bearer). `status`/`limit` optional filters. */
export function cashSessions(
  serverUrl: string,
  status?: string,
  limit?: number,
): Promise<CashSession[]> {
  return invoke<CashSession[]>("cash_sessions", { serverUrl, status, limit });
}

/** POST /api/v1/cash-sessions (Bearer) — open a register. `openingCash` STRING.
 *  `register` (caja física, `register:<key>`) manda sobre `branch`: la caja ya
 *  sabe en qué local está. `branch` es el fallback de la caja suelta. De acá sale
 *  la sucursal que hereda la venta. */
export function openCashSession(
  serverUrl: string,
  registerName: string,
  openingCash: string,
  notes?: string,
  register?: string,
  branch?: string,
): Promise<CashSession> {
  return invoke<CashSession>("open_cash_session", {
    serverUrl,
    registerName,
    openingCash,
    notes,
    register,
    branch,
  });
}

/** GET /api/v1/cash-sessions/{id}/arqueo (Bearer) — non-mutating close preview. */
export function cashArqueo(
  serverUrl: string,
  id: string,
): Promise<CashCloseSummary> {
  return invoke<CashCloseSummary>("cash_arqueo", { serverUrl, id });
}

/** POST /api/v1/cash-sessions/{id}/close (Bearer). `closingCashCounted` STRING. */
export function closeCashSession(
  serverUrl: string,
  id: string,
  closingCashCounted: string,
  notes?: string,
): Promise<CashCloseSummary> {
  return invoke<CashCloseSummary>("close_cash_session", {
    serverUrl,
    id,
    closingCashCounted,
    notes,
  });
}
