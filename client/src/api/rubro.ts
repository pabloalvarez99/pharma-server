// Rubro pack wrapper (client/src-tauri/src/commands/rubro.rs →
// server `GET /api/v1/rubro-pack`, `domain::rubro::RubroPack`).
//
// The server is the source of truth for per-rubro rules; the client keeps its
// local constants in `vertical.ts` as an OFFLINE FALLBACK only.
import { invoke } from "@tauri-apps/api/core";

/** Optional modules a rubro turns on (mirrors `RubroFeatures` server-side). */
export interface PackFeatures {
  recetas: boolean;
  lotes: boolean;
  physical_stock: boolean;
  clinical: boolean;
  /** Agent chat is home (feria). Older servers omit → treat as false. */
  agent_home?: boolean;
  barcode?: boolean;
  printer?: boolean;
  dte?: boolean;
  informal_ok?: boolean;
}

/** One extra product attribute the rubro declares (`product.attrs[key]`). */
export interface PackAttrField {
  key: string;
  label: string;
  kind: string; // "text" | "number" | "money" | "date" | "bool"
}

export interface PackVocab {
  item: string; // "Producto" | "Servicio" | "Plato"
  catalog: string; // "Inventario" | "Carta" | "Servicios"
}

/** Declarative per-rubro pack served by the server. Extra fields are ignored
 *  by older clients (forward-compatible). */
export interface RubroPack {
  rubro: string;
  label: string;
  tagline: string;
  accent: string;
  features: PackFeatures;
  vocab: PackVocab;
  attrs: PackAttrField[];
  seed_vertical: string | null;
  coming_soon: string[];
}

/** GET /api/v1/rubro-pack (Bearer, any role). */
export function rubroPack(serverUrl: string): Promise<RubroPack> {
  return invoke<RubroPack>("rubro_pack", { serverUrl });
}
