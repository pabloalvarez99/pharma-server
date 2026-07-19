// DTE / boleta electrónica SII wrappers (client/src-tauri/src/commands/dte.rs).
import { invoke } from "@tauri-apps/api/core";

/** A DTE row (`crates/api/src/v1/dte.rs::DteDto`). `monto_total` is a STRING
 *  (Decimal); `tipo` is the SII code (39 = boleta). `has_xml` flags whether the
 *  signed XML can be downloaded via {@link dteXml}. */
export interface Dte {
  id: string;
  tipo: number;
  folio: number;
  fecha_emision: string;
  rut_emisor: string;
  rut_receptor: string;
  razon_social_receptor: string;
  monto_total: string;
  estado: string; // "draft" | "signed" | "sent" | "accepted" | "rejected" | "cancelled"
  track_id: number | null;
  sii_glosa: string | null;
  order_id: string | null;
  has_xml: boolean;
}

/** One active CAF + remaining folios (`GET /dte/caf-status` row). */
export interface CafInfo {
  id: string;
  folio_desde: number;
  folio_hasta: number;
  next_folio: number;
  restantes: number;
}

/** `GET /dte/caf-status` payload — total folios left for the DTE type. */
export interface CafStatus {
  tipo: number;
  folios_restantes: number;
  cafs: CafInfo[];
}

/** Filters for {@link listDtes}. Dates are `YYYY-MM-DD`. */
export interface DteFilters {
  estado?: string;
  tipo?: number;
  from?: string;
  to?: string;
  limit?: number;
}

/** GET /api/v1/dte (Bearer) — boletas/DTEs del tenant, newest first. */
export function listDtes(serverUrl: string, filters: DteFilters = {}): Promise<Dte[]> {
  return invoke<Dte[]>("list_dtes", {
    serverUrl,
    estado: filters.estado,
    tipo: filters.tipo,
    from: filters.from,
    to: filters.to,
    limit: filters.limit,
  });
}

/** GET /api/v1/dte/caf-status?tipo=N (Bearer) — folios restantes (default 39). */
export function dteCafStatus(serverUrl: string, tipo?: number): Promise<CafStatus> {
  return invoke<CafStatus>("dte_caf_status", { serverUrl, tipo });
}

/** GET /api/v1/dte/{id}/xml (Bearer) — signed XML text for a Blob download. */
export function dteXml(serverUrl: string, id: string): Promise<string> {
  return invoke<string>("dte_xml", { serverUrl, id });
}

/** Receptor completo — obligatorio en factura/notas/guía (33/56/61/52). */
export interface DocReceptor {
  rut: string;
  razon_social: string;
  giro: string;
  direccion: string;
  comuna: string;
}

/** Línea de documento. Precios IVA-incluido; el server calcula neto/IVA. */
export interface DocItem {
  nombre: string;
  cantidad: string;
  precio_unitario: string;
  exento?: boolean;
}

/** Referencia al documento original (obligatoria en notas 56/61). */
export interface DocReferencia {
  tipo_doc_ref: string;
  folio_ref: string;
  fecha_ref: string; // YYYY-MM-DD
  cod_ref?: number; // 1 anula, 2 corrige texto, 3 corrige montos
  razon_ref?: string;
}

/** POST /api/v1/dte/documentos (Bearer, admin+) — emit + sign factura (33),
 *  nota débito (56), nota crédito (61) o guía (52). Rejects `"CODE|message"`
 *  — use `parseSaleError` from `./pos`. */
export function emitDocumento(
  serverUrl: string,
  args: {
    tipo: number;
    certPassphrase: string;
    receptor: DocReceptor;
    items: DocItem[];
    referencias?: DocReferencia[];
    indTraslado?: number;
    orderId?: string;
  },
): Promise<Dte> {
  return invoke<Dte>("emit_documento", {
    serverUrl,
    tipo: args.tipo,
    certPassphrase: args.certPassphrase,
    receptor: args.receptor,
    items: args.items,
    referencias: args.referencias,
    indTraslado: args.indTraslado,
    orderId: args.orderId,
  });
}

/** GET /api/v1/dte/libro-ventas?period=YYYY-MM (Bearer, admin+) — monthly
 *  sales book XML (unsigned, accountant review / manual upload). */
export function dteLibroVentas(serverUrl: string, period: string): Promise<string> {
  return invoke<string>("dte_libro_ventas", { serverUrl, period });
}

/** POST /api/v1/dte/libro-ventas/signed (Bearer, admin+) — monthly sales book
 *  signed with the company cert (EnvioLibro), ready for the SII portal. */
export function dteLibroVentasSigned(
  serverUrl: string,
  period: string,
  certPassphrase: string,
): Promise<string> {
  return invoke<string>("dte_libro_ventas_signed", { serverUrl, period, certPassphrase });
}

/** POST /api/v1/dte/boletas (Bearer, cashier+) — emit + sign the boleta of a
 *  paid POS order. Rejects with `"CODE|message"` — use `parseSaleError`. */
export function emitBoleta(
  serverUrl: string,
  orderId: string,
  certPassphrase: string,
  receptorRut?: string,
  razonSocialReceptor?: string,
): Promise<Dte> {
  return invoke<Dte>("emit_boleta", {
    serverUrl,
    orderId,
    certPassphrase,
    receptorRut,
    razonSocialReceptor,
  });
}

/** POST /api/v1/dte/{id}/send (Bearer, admin+) — upload to SII. Tier-gated:
 *  rejects `"FEATURE_REQUIRES_UPGRADE|msg"` on Free — use `parseSaleError`. */
export function sendDte(serverUrl: string, id: string): Promise<Dte> {
  return invoke<Dte>("send_dte", { serverUrl, id });
}

/** Poll result: the refreshed DTE + the normalized SII verdict of this query. */
export interface DtePollResult extends Dte {
  sii_estado: string; // aceptado|aceptado_con_reparos|rechazado|recibido|en_proceso|error
}

/** POST /api/v1/dte/{id}/poll (Bearer, admin+) — refresh the SII verdict. */
export function pollDte(serverUrl: string, id: string): Promise<DtePollResult> {
  return invoke<DtePollResult>("poll_dte", { serverUrl, id });
}

/** POST /api/v1/dte/{id}/cancel (Bearer, admin+) — anular pre-envío. */
export function cancelDte(serverUrl: string, id: string, reason: string): Promise<Dte> {
  return invoke<Dte>("cancel_dte", { serverUrl, id, reason });
}
