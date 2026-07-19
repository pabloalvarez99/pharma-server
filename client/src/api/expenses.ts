// Expenses (gastos / caja chica) wrappers (client/src-tauri/src/commands/expenses.rs).
import { invoke } from "@tauri-apps/api/core";

/** An expense / egreso (`ExpenseDto`). `amount` is a STRING (Decimal).
 *  `cash_session`/`supplier`/`note`/`created_by` are null when unset. */
export interface Expense {
  id: string;
  category: string;
  description: string;
  amount: string;
  payment_method: string; // "cash" | "bank" | "card" | "transfer"
  cash_session: string | null;
  supplier: string | null;
  note: string | null;
  created_by: string | null;
  incurred_at: string;
  created_at: string;
}

/** Fields the Gastos form sends to create an expense (`NewExpense`). `amount` is
 *  a STRING; `paymentMethod` defaults to `cash` server-side when omitted. */
export interface NewExpense {
  category: string;
  description: string;
  amount: string;
  paymentMethod?: string;
  note?: string;
  incurredAt?: string; // RFC3339
}

/** GET /api/v1/expenses (Bearer, cashier+). `category`/`paymentMethod`/`limit`
 *  optional filters. */
export function listExpenses(
  serverUrl: string,
  category?: string,
  paymentMethod?: string,
  limit?: number,
): Promise<Expense[]> {
  return invoke<Expense[]>("list_expenses", { serverUrl, category, paymentMethod, limit });
}

/** POST /api/v1/expenses (Bearer, cashier+) — record an expense. `amount` STRING. */
export function createExpense(serverUrl: string, e: NewExpense): Promise<Expense> {
  return invoke<Expense>("create_expense", {
    serverUrl,
    category: e.category,
    description: e.description,
    amount: e.amount,
    paymentMethod: e.paymentMethod,
    note: e.note,
    incurredAt: e.incurredAt,
  });
}
