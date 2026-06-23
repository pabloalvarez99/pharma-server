// Configuration hub — pure model + selectors (no DOM, no Tauri). The view
// (configuracion.ts) is a thin renderer over this: the section catalog drives
// the side nav, `searchConfig` powers the in-settings search, the save-state
// machine drives per-card feedback, and the validators gate every write. Kept
// DOM-free so all of it is unit-tested without happy-dom. Spanish labels are the
// product surface — the operator NEVER sees a raw `admin_setting` key.
import { isValidRut, canonicalRut } from "../format";

export type SectionId =
  | "negocio"
  | "sii"
  | "licencia"
  | "agente"
  | "preferencias"
  | "usuarios"
  | "sucursales"
  | "respaldo";

export interface ConfigField {
  /** Human label shown to the operator — never the raw setting key. */
  label: string;
  /** Extra search synonyms beyond the label (also accent-insensitive). */
  keywords?: readonly string[];
}

export interface ConfigSection {
  id: SectionId;
  /** Side-nav title. */
  label: string;
  /** One-line description under the section heading. */
  blurb: string;
  /** Icon id resolved by the view (see `sectionIconSvg`). */
  icon: string;
  /** Searchable fields living in this section. */
  fields: readonly ConfigField[];
  /** Honest "próximamente / hoy por CLI" mount — backend lands in a later wave. */
  placeholder?: boolean;
}

// The catalog. Order = nav order. `negocio` is the landing section. Placeholder
// sections are still listed (and searchable) so the hub reads as complete and
// the owner sees the roadmap — never a dead-end (ADR-0005: no dark patterns).
export const CONFIG_SECTIONS: readonly ConfigSection[] = [
  {
    id: "negocio",
    label: "Negocio",
    blurb: "Identidad de tu negocio: rubro, nombre y datos tributarios.",
    icon: "store",
    fields: [
      { label: "Rubro del negocio", keywords: ["vertical", "giro comercial", "tipo de negocio"] },
      { label: "Nombre del negocio", keywords: ["razón social comercial", "marca"] },
      { label: "RUT empresa", keywords: ["rol único tributario"] },
      { label: "Razón social" },
      { label: "Giro" },
      { label: "Dirección", keywords: ["domicilio", "comuna", "ciudad"] },
      { label: "Datos demo", keywords: ["catálogo de ejemplo", "seed", "prueba"] },
    ],
  },
  {
    id: "sii",
    label: "Facturación SII",
    blurb: "Boleta y factura electrónica: ambiente, folios y certificados.",
    icon: "receipt",
    fields: [
      { label: "Ambiente SII", keywords: ["sandbox", "certificación", "producción"] },
      { label: "Folios CAF", keywords: ["caf", "folios disponibles", "rango"] },
      { label: "Certificado digital", keywords: ["cert", "firma electrónica", "pfx"] },
    ],
  },
  {
    id: "licencia",
    label: "Licencia y plan",
    blurb: "Tu plan actual, estado y funcionalidades habilitadas.",
    icon: "key",
    fields: [
      { label: "Plan actual", keywords: ["tier", "free", "pro", "business", "enterprise"] },
      { label: "Estado de la licencia", keywords: ["vencimiento", "asientos"] },
      { label: "Funcionalidades habilitadas", keywords: ["features", "entitlements"] },
    ],
  },
  {
    id: "agente",
    label: "Agente",
    blurb: "Tu agente de negocio y la federación con otros nodos.",
    icon: "robot",
    fields: [
      { label: "Federación B2B", keywords: ["ecosistema", "cotizaciones", "órdenes entre nodos"] },
      { label: "Fidelidad de clientes", keywords: ["puntos", "loyalty", "premios"] },
    ],
  },
  {
    id: "preferencias",
    label: "Preferencias",
    blurb: "Apariencia, idioma, conexión al servidor y telemetría.",
    icon: "sliders",
    fields: [
      { label: "Tema", keywords: ["apariencia", "oscuro", "claro", "modo"] },
      { label: "Idioma", keywords: ["lenguaje", "español"] },
      { label: "Conexión al servidor", keywords: ["url", "ip", "servidor"] },
      { label: "Telemetría anónima", keywords: ["estadísticas", "uso", "opt-in", "privacidad"] },
    ],
  },
  {
    id: "usuarios",
    label: "Usuarios y roles",
    blurb: "Cajeros, químicos y administradores con sus permisos.",
    icon: "users",
    fields: [
      { label: "Usuarios", keywords: ["cuentas", "personas"] },
      { label: "Roles", keywords: ["permisos", "cajero", "químico", "admin", "dueño"] },
    ],
  },
  {
    id: "sucursales",
    label: "Sucursales y cajas",
    blurb: "Locales, bodegas y puntos de caja de tu negocio.",
    icon: "building",
    placeholder: true,
    fields: [
      { label: "Sucursales", keywords: ["locales", "tiendas", "multi-tenant"] },
      { label: "Cajas", keywords: ["puntos de venta", "terminales"] },
    ],
  },
  {
    id: "respaldo",
    label: "Respaldo",
    blurb: "Copias de seguridad programadas y restauración guiada.",
    icon: "shield",
    placeholder: true,
    fields: [
      { label: "Respaldo automático", keywords: ["backup", "copia de seguridad", "programado"] },
      { label: "Restaurar", keywords: ["restore", "recuperar datos"] },
    ],
  },
] as const;

/** The section the hub opens on. */
export function defaultSection(): SectionId {
  return CONFIG_SECTIONS[0].id;
}

/** Resolve a requested section id to a real one, falling back to the default
 *  when the request is unknown (e.g. a stale deep-link or a filtered-away tab). */
export function resolveSection(
  requested: string | null | undefined,
  sections: readonly ConfigSection[] = CONFIG_SECTIONS,
): SectionId {
  const hit = sections.find((s) => s.id === requested);
  return hit ? hit.id : (sections[0]?.id ?? defaultSection());
}

/** Accent- and case-insensitive normalisation so "facturacion" matches
 *  "Facturación" and "telemetria" matches "Telemetría". */
export function norm(s: string): string {
  return s
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .trim();
}

export interface FieldHit {
  section: SectionId;
  label: string;
}

export interface ConfigSearchResult {
  /** Matching sections in catalog order. Empty query → every section. */
  sectionIds: SectionId[];
  /** Field labels that matched, flattened, in catalog order. */
  fieldHits: FieldHit[];
  /** True when a non-blank query is active (drives the "N de M" affordance). */
  filtered: boolean;
}

// Search ranks a section IN when its label/blurb matches, OR any of its fields
// (label or keyword) matches. Field-level hits are surfaced separately so the
// view can offer "jump to field" chips. A blank query returns the full catalog
// and no hits (nav shows everything, no filter chrome).
export function searchConfig(
  query: string,
  sections: readonly ConfigSection[] = CONFIG_SECTIONS,
): ConfigSearchResult {
  const q = norm(query);
  if (!q) {
    return { sectionIds: sections.map((s) => s.id), fieldHits: [], filtered: false };
  }
  const sectionIds: SectionId[] = [];
  const fieldHits: FieldHit[] = [];
  for (const sec of sections) {
    const fieldMatches = sec.fields.filter(
      (f) =>
        norm(f.label).includes(q) ||
        (f.keywords ?? []).some((k) => norm(k).includes(q)),
    );
    const sectionMatches =
      norm(sec.label).includes(q) || norm(sec.blurb).includes(q);
    if (sectionMatches || fieldMatches.length > 0) {
      sectionIds.push(sec.id);
      for (const f of fieldMatches) fieldHits.push({ section: sec.id, label: f.label });
    }
  }
  return { sectionIds, fieldHits, filtered: true };
}

// --- Save-state machine -----------------------------------------------------
// One tiny state per save control. The view maps `status` → CSS class and shows
// `message`; keeping the transitions here makes the feedback testable and
// uniform across every card (no ad-hoc `statusEl.textContent =` divergence).

export type SaveStatus = "idle" | "saving" | "ok" | "error";

export interface SaveState {
  status: SaveStatus;
  message: string | null;
}

export const SAVE_IDLE: SaveState = { status: "idle", message: null };

export function toSaving(): SaveState {
  return { status: "saving", message: "Guardando…" };
}

export function toSaved(message = "Guardado"): SaveState {
  return { status: "ok", message };
}

export function toFailed(message: string): SaveState {
  return { status: "error", message };
}

/** CSS suffix for a save state (`cfg-status cfg-status-<x>`). Idle → no suffix. */
export function saveStatusClass(state: SaveState): string {
  switch (state.status) {
    case "ok":
      return "cfg-status cfg-status-ok";
    case "error":
      return "cfg-status cfg-status-err";
    case "saving":
      return "cfg-status cfg-status-pending";
    default:
      return "cfg-status";
  }
}

// --- Field validators -------------------------------------------------------
// Pure, message-bearing. The view shows `message` inline and only writes when
// `ok`. Errors are Spanish (user-facing); `value` carries the normalised value
// to persist (canonical RUT, truncated integer) so the DOM layer never re-parses.

export interface FieldResult<T> {
  ok: boolean;
  value?: T;
  message: string | null;
}

export function validateRequiredText(raw: string, label: string): FieldResult<string> {
  const v = raw.trim();
  if (!v) return { ok: false, message: `Completa el campo "${label}".` };
  return { ok: true, value: v, message: null };
}

/** Business RUT for the SII emitter — a wrong one breaks every DTE. Validates
 *  mód-11 and returns the canonical `NNNNNNNN-D` form to persist. */
export function validateBusinessRut(raw: string): FieldResult<string> {
  const v = raw.trim();
  if (!v) return { ok: false, message: "Ingresa el RUT de la empresa." };
  if (!isValidRut(v)) {
    return {
      ok: false,
      message: "RUT inválido: revisa el dígito verificador (mód-11).",
    };
  }
  return { ok: true, value: canonicalRut(v), message: null };
}

export function validateNonNegativeInt(raw: string, label: string): FieldResult<number> {
  const v = raw.trim();
  if (!v) return { ok: true, value: 0, message: null };
  const n = Number(v);
  if (!Number.isFinite(n) || n < 0) {
    return { ok: false, message: `${label}: ingresa un número válido (0 o más).` };
  }
  return { ok: true, value: Math.trunc(n), message: null };
}

/** Optional SII activity code — empty is allowed, but if present it must be a
 *  non-negative integer. `value` is `undefined` when the operator left it blank. */
export function validateActeco(raw: string): FieldResult<number | undefined> {
  const v = raw.trim();
  if (!v) return { ok: true, value: undefined, message: null };
  const n = Number(v);
  if (!Number.isInteger(n) || n < 0) {
    return { ok: false, message: "El código acteco debe ser un número entero." };
  }
  return { ok: true, value: n, message: null };
}
