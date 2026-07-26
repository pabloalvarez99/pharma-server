// Sucursales + stock por sucursal + transferencias
// (client/src-tauri/src/commands/sucursales.rs).
//
// Multi-sucursal OPERATIVO (V2): el stock deja de ser un número global y pasa a
// vivir POR local. `branch === null` = casa matriz (el negocio de un solo local
// vive siempre ahí y no ve ninguna diferencia).
import { invoke } from "@tauri-apps/api/core";

/** Una sucursal del negocio (`/api/v1/sucursales`). */
export interface Sucursal {
  id: string;
  name: string;
  code: string | null;
  address: string | null;
  comuna: string | null;
  phone: string | null;
  active: boolean;
}

/** Una caja física (`/api/v1/cajas`), opcionalmente asignada a una sucursal. */
export interface Caja {
  id: string;
  branch: string | null;
  name: string;
  code: string | null;
  active: boolean;
}

/** On-hand de un producto en una sucursal. `branch === null` = casa matriz. */
export interface StockSucursal {
  product: string;
  product_name: string | null;
  branch: string | null;
  branch_name: string | null;
  stock: number;
  updated_at: string;
}

/** Fila del reporte: un producto, su desglose por local y el total (que por
 *  invariante es el stock global del producto). */
export interface StockSucursalReporte {
  product: string;
  product_name: string;
  by_branch: { branch: string | null; branch_name: string | null; stock: number }[];
  total: number;
}

/** Resultado de una transferencia aplicada. */
export interface TransferenciaResult {
  product: string;
  product_name: string;
  from_branch: string | null;
  to_branch: string | null;
  qty: number;
  from_stock: number;
  to_stock: number;
  movement_out: string;
  movement_in: string;
}

/** GET /api/v1/sucursales (Bearer). */
export function listSucursales(
  serverUrl: string,
  active?: boolean,
): Promise<Sucursal[]> {
  return invoke<Sucursal[]>("sucursales", { serverUrl, active });
}

/** GET /api/v1/cajas (Bearer). `branch` filtra las cajas de ese local. */
export function listCajas(
  serverUrl: string,
  branch?: string,
  active?: boolean,
): Promise<Caja[]> {
  return invoke<Caja[]>("cajas", { serverUrl, branch, active });
}

/** GET /api/v1/stock/sucursales (Bearer). `branch` acepta `"none"` para la
 *  casa matriz. */
export function stockPorSucursal(
  serverUrl: string,
  opts: { product?: string; branch?: string; nonZero?: boolean } = {},
): Promise<StockSucursal[]> {
  return invoke<StockSucursal[]>("stock_por_sucursal", {
    serverUrl,
    product: opts.product,
    branch: opts.branch,
    nonZero: opts.nonZero,
  });
}

/** GET /api/v1/stock/sucursales/reporte (Bearer). */
export function stockPorSucursalReporte(
  serverUrl: string,
  opts: { branch?: string; nonZero?: boolean } = {},
): Promise<StockSucursalReporte[]> {
  return invoke<StockSucursalReporte[]>("stock_por_sucursal_reporte", {
    serverUrl,
    branch: opts.branch,
    nonZero: opts.nonZero,
  });
}

/** POST /api/v1/stock/transferencias (Bearer, admin+). `fromBranch`/`toBranch`
 *  ausentes o `"none"` = casa matriz. Rechaza con `INSUFFICIENT_STOCK|…` si el
 *  origen no tiene tanto. */
export function transferirStock(
  serverUrl: string,
  input: {
    product: string;
    fromBranch?: string;
    toBranch?: string;
    qty: number;
    notes?: string;
  },
): Promise<TransferenciaResult> {
  return invoke<TransferenciaResult>("transferir_stock", {
    serverUrl,
    product: input.product,
    fromBranch: input.fromBranch,
    toBranch: input.toBranch,
    qty: input.qty,
    notes: input.notes,
  });
}

// --- sucursal activa (estado del shell) ------------------------------------

const ACTIVE_BRANCH_KEY = "sucursal_activa";
/** Valor sentinela de la casa matriz — el mismo token que entiende el server. */
export const CASA_MATRIZ = "none";

/** Sucursal activa del operador. `"none"` = casa matriz. Vive en
 *  `localStorage` para que sobreviva al refresh de la PWA y al reinicio de la
 *  app; es una preferencia de terminal, no del usuario, así que NO viaja al
 *  servidor ni se sincroniza entre dispositivos: la tablet del local 2 se queda
 *  en el local 2. */
export function sucursalActiva(): string {
  try {
    return localStorage.getItem(ACTIVE_BRANCH_KEY) || CASA_MATRIZ;
  } catch {
    return CASA_MATRIZ;
  }
}

/** Cambia la sucursal activa. Emite `sucursal-cambiada` para que las vistas
 *  montadas se refresquen sin que el shell tenga que conocerlas. */
export function setSucursalActiva(id: string): void {
  try {
    localStorage.setItem(ACTIVE_BRANCH_KEY, id || CASA_MATRIZ);
  } catch {
    /* modo privado / storage lleno: la sesión sigue, sin persistir */
  }
  window.dispatchEvent(
    new CustomEvent("sucursal-cambiada", { detail: { branch: id || CASA_MATRIZ } }),
  );
}

/** La sucursal activa como la espera el POS: `undefined` en casa matriz (el
 *  server la deduce de la caja o cae a casa matriz). */
export function sucursalParaVenta(): string | undefined {
  const b = sucursalActiva();
  return b === CASA_MATRIZ ? undefined : b;
}
