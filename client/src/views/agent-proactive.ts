// Proactive agent cards — the agent that SPEAKS FIRST. Where the ask-bar waits
// to be asked, this surfaces the few things the owner should act on the moment
// they open Panel: a register left open from yesterday, lots about to expire,
// stock running out. It is the north star made *proactive* (1 RUT = 1 agente
// que avisa), built ENTIRELY from read endpoints the dashboard already has — no
// new server calls, no writes.
//
// Anti-dark-pattern (CLAUDE.md / ADR-0005), enforced here, not just intended:
//   - capped at MAX_CARDS so it never becomes a wall of nags;
//   - every card is DISMISSIBLE, and a dismissal is remembered FOR THE DAY (a
//     genuinely new alert tomorrow still surfaces — we hide noise, not signal);
//   - multi-rubro: a service rubro (no stock) never sees expiry/stock cards; a
//     non-perishable rubro never sees expiry. Gated off `featuresForRubro`, the
//     same capability source the rest of the app reads.
//
// All logic here is pure + DOM-free (the dashboard owns the mount), so the
// selection / ordering / dismissal rules are unit-tested without a browser.
import type { CashSession, InventorySummary, NearExpiryRow } from "../api";
import { featuresForRubro, type Rubro } from "../vertical";

/** Hard cap so the panel never turns into a nag wall (anti-dark-pattern). */
export const MAX_CARDS = 3;
/** "Esta semana" window for the near-expiry card. */
export const NEAR_EXPIRY_DAYS = 7;

export type Severity = "danger" | "warn";
/** Glyph id the DOM layer maps to an inline SVG (kept out of the pure model). */
export type ProactiveIcon = "drawer" | "expiry" | "stock";

/** What a card's button does: jump to a view, or hand the agent a question (so
 *  the proactive card and the conversational ask-bar reinforce each other). */
export type ProactiveAction =
  | { kind: "nav"; nav: string; label: string }
  | { kind: "ask"; question: string; label: string };

export interface ProactiveCard {
  /** Stable id (independent of count) so a dismissal sticks across re-renders. */
  id: string;
  severity: Severity;
  icon: ProactiveIcon;
  title: string;
  body: string;
  action: ProactiveAction;
}

export interface ProactiveInput {
  /** Decides which cards even apply (service rubros skip stock/expiry). */
  rubro: Rubro;
  /** Open cash sessions (`cashSessions(url, "open")`); null = fetch failed. */
  openSessions: CashSession[] | null;
  /** Near-expiry rows (`nearExpiry(url, NEAR_EXPIRY_DAYS)`); null = N/A or failed. */
  expiry: NearExpiryRow[] | null;
  /** Inventory KPIs (`inventorySummary`); null = N/A or failed. */
  inventory: InventorySummary | null;
  /** Injectable clock for deterministic tests. */
  now?: Date;
}

const SEVERITY_RANK: Record<Severity, number> = { danger: 0, warn: 1 };

/** Local calendar day (YYYY-MM-DD) — the owner reasons in their wall clock, not
 *  UTC, so "de ayer" must use the local date. */
export function localDay(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Count distinct products in a set of expiry rows (one product may have several
 *  lots; the owner cares about products, not rows). */
function distinctProducts(rows: NearExpiryRow[]): number {
  return new Set(rows.map((r) => r.product_id)).size;
}

/** Build the ordered, capped list of proactive cards from already-fetched data.
 *  Pure: the dashboard fetches, this decides. Cards are emitted in priority
 *  order, stable-sorted so `danger` floats above `warn`, then capped. A null
 *  input (failed/skipped fetch) simply contributes no card — never an error. */
export function buildProactiveCards(input: ProactiveInput): ProactiveCard[] {
  const now = input.now ?? new Date();
  const today = localDay(now);
  const f = featuresForRubro(input.rubro);
  const cards: ProactiveCard[] = [];

  // 1. Register left open from a previous day — money uncuadrado. Universal
  //    (every rubro runs caja), highest priority.
  if (input.openSessions) {
    const stale = input.openSessions.filter((s) => {
      if (s.status !== "open") return false;
      const t = new Date(s.opened_at);
      if (Number.isNaN(t.getTime())) return false;
      return localDay(t) < today;
    });
    if (stale.length > 0) {
      cards.push({
        id: "caja-stale",
        severity: "danger",
        icon: "drawer",
        title: "Caja sin cerrar",
        body:
          stale.length === 1
            ? "Tienes una caja abierta de un día anterior. Ciérrala para cuadrar el efectivo."
            : `Tienes ${stale.length} cajas abiertas de días anteriores. Ciérralas para cuadrar el efectivo.`,
        action: { kind: "nav", nav: "caja", label: "Revisar caja" },
      });
    }
  }

  // 2 & 3. Expiry — only for rubros that track lots/perishables.
  if (f.lotes && input.expiry) {
    const expired = input.expiry.filter((r) => r.expired);
    const expiredN = distinctProducts(expired);
    if (expiredN > 0) {
      cards.push({
        id: "expiry-expired",
        severity: "danger",
        icon: "expiry",
        title: "Productos vencidos",
        body:
          expiredN === 1
            ? "Hay 1 producto con lotes ya vencidos. Retíralo de la venta."
            : `Hay ${expiredN} productos con lotes ya vencidos. Retíralos de la venta.`,
        action: { kind: "nav", nav: "inventory", label: "Ver inventario" },
      });
    }

    const soon = input.expiry.filter((r) => !r.expired && r.days_to_expiry <= NEAR_EXPIRY_DAYS);
    const soonN = distinctProducts(soon);
    if (soonN > 0) {
      cards.push({
        id: "expiry-soon",
        severity: "warn",
        icon: "expiry",
        title: "Por vencer esta semana",
        body:
          soonN === 1
            ? "1 producto se vence dentro de 7 días. Conviene priorizarlo o liquidarlo."
            : `${soonN} productos se vencen dentro de 7 días. Conviene priorizarlos o liquidarlos.`,
        // Hand it to the agent — showcases that you can just ask for the detail.
        action: { kind: "ask", question: "¿Qué se está por vencer?", label: "Preguntar al agente" },
      });
    }
  }

  // 4. Stock health — only for rubros with physical stock.
  if (f.physicalStock && input.inventory) {
    const { out_of_stock, low_stock } = input.inventory;
    if (out_of_stock > 0) {
      cards.push({
        id: "stock-out",
        severity: "danger",
        icon: "stock",
        title: "Productos agotados",
        body:
          out_of_stock === 1
            ? "1 producto está agotado. Repón stock para no perder ventas."
            : `${out_of_stock} productos están agotados. Repón stock para no perder ventas.`,
        action: { kind: "nav", nav: "inventory", label: "Reponer stock" },
      });
    } else if (low_stock > 0) {
      // Only when nothing is fully out — agotado already implies "revisa stock".
      cards.push({
        id: "stock-low",
        severity: "warn",
        icon: "stock",
        title: "Stock bajo",
        body:
          low_stock === 1
            ? "1 producto está por debajo de su mínimo. Considera reponerlo."
            : `${low_stock} productos están por debajo de su mínimo. Considera reponerlos.`,
        action: { kind: "nav", nav: "inventory", label: "Revisar stock" },
      });
    }
  }

  // Stable sort by severity (danger first); insertion order breaks ties so the
  // priority above is preserved within a severity band. Then cap.
  return cards
    .map((c, i) => ({ c, i }))
    .sort((a, b) => SEVERITY_RANK[a.c.severity] - SEVERITY_RANK[b.c.severity] || a.i - b.i)
    .slice(0, MAX_CARDS)
    .map((x) => x.c);
}

// --- Dismissal (per-day) ----------------------------------------------------
//
// A dismissal silences a card FOR TODAY only: hide the noise the owner already
// saw, but never bury a genuinely new alert tomorrow. Keys are date-stamped and
// pruned to today on write, so storage can't grow without bound.

/** Minimal storage shape (subset of Web Storage) so dismissal is testable
 *  without a real localStorage. Mirrors onboarding-ux's `KeyStore`. */
export interface KeyStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export const DISMISS_STORE_KEY = "pharma:agent-cards-dismissed";

/** Date-stamped dismissal key for a card. */
export function dismissKey(cardId: string, now: Date = new Date()): string {
  return `${cardId}:${localDay(now)}`;
}

function readKeys(store: KeyStore): string[] {
  let raw: string | null;
  try {
    raw = store.getItem(DISMISS_STORE_KEY);
  } catch {
    return [];
  }
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((k): k is string => typeof k === "string") : [];
  } catch {
    return [];
  }
}

/** True when this card was dismissed earlier TODAY (a prior day's dismissal does
 *  not count — the alert re-surfaces). */
export function isDismissed(store: KeyStore, cardId: string, now: Date = new Date()): boolean {
  return readKeys(store).includes(dismissKey(cardId, now));
}

/** Record a dismissal for today, pruning any keys not from today (bounded). */
export function markDismissed(store: KeyStore, cardId: string, now: Date = new Date()): void {
  const today = localDay(now);
  const next = new Set(readKeys(store).filter((k) => k.endsWith(`:${today}`)));
  next.add(dismissKey(cardId, now));
  try {
    store.setItem(DISMISS_STORE_KEY, JSON.stringify([...next]));
  } catch {
    /* storage unavailable — the card just reappears, no crash */
  }
}

/** Drop cards the owner already dismissed today. */
export function filterDismissed(
  cards: ProactiveCard[],
  store: KeyStore,
  now: Date = new Date(),
): ProactiveCard[] {
  return cards.filter((c) => !isDismissed(store, c.id, now));
}

// --- Agent onboarding intro (one-time coach mark) ---------------------------
//
// The other half of the lane: the owner must DISCOVER they can talk to their
// business. A one-time, dismissible intro on the ask-bar teaches it; once seen
// it never shows again (a persistent flag, not per-day). Zero dark patterns:
// it's a hint, not a modal, and dismissing it is permanent.

export const AGENT_INTRO_KEY = "pharma:agent-intro-seen";

/** Show the "háblale a tu negocio" intro until the owner has dismissed it once. */
export function shouldShowAgentIntro(store: KeyStore): boolean {
  try {
    return store.getItem(AGENT_INTRO_KEY) !== "1";
  } catch {
    return false; // storage blocked → don't risk nagging every launch
  }
}

/** Permanently mark the agent intro as seen. No-throw. */
export function dismissAgentIntro(store: KeyStore): void {
  try {
    store.setItem(AGENT_INTRO_KEY, "1");
  } catch {
    /* noop */
  }
}
