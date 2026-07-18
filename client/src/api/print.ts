// Thermal ticket printing (P0.4) — Tauri `print_ticket` command.
//
// Printer name + paper width are per-MACHINE (localStorage), not tenant
// settings: the thermal printer is plugged into one PC. When no printer is
// configured the POS falls back to `window.print()`.
import { invoke } from "@tauri-apps/api/core";
import type { Receipt } from "./pos";

/** localStorage keys — machine-local, never synced to the server. */
export const THERMAL_PRINTER_KEY = "rb.thermalPrinter";
export const THERMAL_WIDTH58_KEY = "rb.thermalWidth58";
/** Cash-drawer kick after thermal print. Default off (most PCs have no drawer). */
export const OPEN_DRAWER_KEY = "rb.openDrawer";

export interface PrintTicketInput {
  tenantName: string;
  folioOrNumber: string;
  datetime: string;
  items: {
    name: string;
    qty: number;
    unitPrice: string;
    lineTotal: string;
  }[];
  discount: string;
  total: string;
  paymentMethod: string;
  cashAmount?: string | null;
  cardAmount?: string | null;
  change?: string | null;
  cashier?: string | null;
  footerNote: string;
}

/** Read the configured Windows printer name (empty = use browser print). */
export function thermalPrinterName(): string {
  try {
    return (localStorage.getItem(THERMAL_PRINTER_KEY) ?? "").trim();
  } catch {
    return "";
  }
}

/** True when paper is 58mm (32 cols); false = 80mm (48 cols). Default 58mm. */
export function thermalWidth58(): boolean {
  try {
    const v = localStorage.getItem(THERMAL_WIDTH58_KEY);
    if (v === null) return true;
    return v !== "0" && v !== "false";
  } catch {
    return true;
  }
}

export function setThermalPrinter(name: string, width58: boolean): void {
  try {
    if (name.trim()) localStorage.setItem(THERMAL_PRINTER_KEY, name.trim());
    else localStorage.removeItem(THERMAL_PRINTER_KEY);
    localStorage.setItem(THERMAL_WIDTH58_KEY, width58 ? "1" : "0");
  } catch {
    /* private mode / quota — ignore */
  }
}

/** True when Preferencias asks to pulse the cash drawer on thermal print. Default off. */
export function openDrawerEnabled(): boolean {
  try {
    const v = localStorage.getItem(OPEN_DRAWER_KEY);
    return v === "1" || v === "true";
  } catch {
    return false;
  }
}

export function setOpenDrawer(enabled: boolean): void {
  try {
    if (enabled) localStorage.setItem(OPEN_DRAWER_KEY, "1");
    else localStorage.removeItem(OPEN_DRAWER_KEY);
  } catch {
    /* private mode / quota — ignore */
  }
}

/** Map a POS `Receipt` (snake_case wire) to the camelCase Tauri payload. */
export function receiptToPrintInput(r: Receipt): PrintTicketInput {
  return {
    tenantName: r.tenant_name,
    folioOrNumber: r.folio_or_number,
    datetime: r.datetime,
    items: r.items.map((it) => ({
      name: it.name,
      qty: it.qty,
      unitPrice: it.unit_price,
      lineTotal: it.line_total,
    })),
    discount: r.discount,
    total: r.total,
    paymentMethod: r.payment_method,
    cashAmount: r.cash_amount,
    cardAmount: r.card_amount,
    change: r.change,
    cashier: r.cashier,
    footerNote: r.footer_note,
  };
}

/** Spool a receipt to the named Windows thermal printer (ESC/POS RAW). */
export function printTicket(
  printer: string,
  width58: boolean,
  receipt: PrintTicketInput,
  openDrawer = false,
): Promise<void> {
  return invoke<void>("print_ticket", {
    printer,
    width58,
    receipt,
    openDrawer,
  });
}

/** Pulse the cash drawer via the thermal printer (ESC p). Requires a printer name. */
export function openCashDrawer(printer: string): Promise<void> {
  return invoke<void>("open_cash_drawer", { printer });
}

/**
 * Prefer thermal when configured; fall back to `window.print()`.
 * Never throws — POS must stay unblocked if the printer is offline.
 * When `rb.openDrawer` is on, the kick is appended to the same RAW job.
 */
export async function printReceiptPreferThermal(r: Receipt): Promise<"thermal" | "browser"> {
  const printer = thermalPrinterName();
  if (!printer) {
    window.print();
    return "browser";
  }
  try {
    await printTicket(
      printer,
      thermalWidth58(),
      receiptToPrintInput(r),
      openDrawerEnabled(),
    );
    return "thermal";
  } catch {
    window.print();
    return "browser";
  }
}
