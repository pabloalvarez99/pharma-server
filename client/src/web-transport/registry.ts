// Command registry: the 73 Tauri commands mapped to fetch handlers, plus the
// desktop-only degradations. Grouped per domain mirroring
// client/src-tauri/src/commands/*.rs for reviewability.

import type { CommandHandler } from "./core";
import { authCommands } from "./commands/auth";
import { catalogCommands } from "./commands/catalog";
import { posCommands } from "./commands/pos";
import { cashCommands } from "./commands/cash";
import { customerCommands } from "./commands/customers";
import { purchaseCommands } from "./commands/purchases";
import { dteCommands } from "./commands/dte";
import { miscCommands } from "./commands/misc";
import { sucursalCommands } from "./commands/sucursales";

/** Error for commands with no pure-HTTP equivalent (ESC/POS printing, cash
 *  drawer, updater). Views already catch it and fall back (e.g. POS printing
 *  falls back to window.print()) — the view never dies. */
export const DESKTOP_ONLY_ERROR = "Disponible en la app de escritorio";

const desktopOnly: CommandHandler = async () => {
  throw DESKTOP_ONLY_ERROR;
};

export const registry: Record<string, CommandHandler> = {
  ...authCommands,
  ...catalogCommands,
  ...posCommands,
  ...cashCommands,
  ...customerCommands,
  ...purchaseCommands,
  ...dteCommands,
  ...sucursalCommands,
  ...miscCommands,
  // Desktop-only surface (commands/print.rs → escpos.rs spooling RAW to a
  // Windows printer; no browser equivalent).
  print_ticket: desktopOnly,
  open_cash_drawer: desktopOnly,
};
