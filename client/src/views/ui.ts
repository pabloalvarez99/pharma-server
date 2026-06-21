// ════════════════════════════════════════════════════════════════════════════
// RutBusiness design system — produced state + component helpers (W5, Pilar C).
//
// The canonical, ADDITIVE surface for the "completo y profesional" bar
// (professional-completeness-master-plan.md §Pilar C): every list/panel renders
// its empty / loading / error state through ONE produced helper so a fresh
// install never shows a bare `<p>text</p>` that reads "de dev". Pure string
// producers — no DOM, no deps, trivially testable (see ui.test.ts).
//
// USAGE (import `from "./ui"` inside a view):
//
//   import { emptyState, errorState, loadingState } from "./ui";
//
//   // empty (a list with no rows yet) — teaches the next step, optional CTA:
//   host.innerHTML = emptyState({
//     title: "Sin boletas emitidas",
//     hint: "Emite tu primera boleta desde una venta del POS.",
//     action: { id: "go-pos", label: "Ir al POS" },   // wire after innerHTML
//   });
//
//   // error (a fetch rejected) — pass the already-humanized message:
//   host.innerHTML = errorState(asMessage(err), { retry: { id: "x-retry", label: "Reintentar" } });
//
//   // loading (before the fetch resolves) — for non-table loads; tables keep
//   // inventory.ts `tableSkeleton`/`kpiSkeleton`:
//   host.innerHTML = loadingState({ label: "Consultando folios…" });
//
// STYLING: classes are the `.ui-*` namespace (brand.css, appended block). They
// reuse the global styles.css tokens (--bg-*, --line, --text, --muted, --accent,
// --danger, --warn, --radius-card) so light/dark theme follow automatically.
// Disjoint from `.cfg-*` (ye), `.cmdk-*` (paul) and the `.rb-*`/`.rubro-*`/
// `.dash-*`/`.agent-*` blocks already in brand.css.
//
// ADOPTION: other lanes adopt these in later waves — this wave only ships the set
// + bob's own views (reports/boletas/facturas/recetas/auditoría).
// ════════════════════════════════════════════════════════════════════════════

/** Escape the five HTML-significant characters for safe interpolation into a
 *  template string. The design system's own escaper so the module is
 *  self-contained (a presentation concern lives with the presentation helpers). */
export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );
}

export type ButtonVariant = "primary" | "ghost" | "danger";

export interface ButtonOpts {
  label: string;
  variant?: ButtonVariant;
  id?: string;
  /** Native button type — `"button"` by default so a control inside a `<form>`
   *  never submits it by accident. */
  type?: "button" | "submit" | "reset";
  /** Extra class(es) appended after the base `.ui-btn ui-btn-<variant>`. */
  cls?: string;
}

/** A themed button. Variants: primary (brand CTA), ghost (secondary), danger
 *  (destructive). Label is escaped; markup is the same across every view. */
export function button(o: ButtonOpts): string {
  const variant = o.variant ?? "primary";
  const type = o.type ?? "button";
  const cls = `ui-btn ui-btn-${variant}${o.cls ? ` ${o.cls}` : ""}`;
  const id = o.id ? ` id="${escapeHtml(o.id)}"` : "";
  return `<button${id} type="${type}" class="${cls}">${escapeHtml(o.label)}</button>`;
}

export interface EmptyStateOpts {
  title: string;
  hint?: string;
  /** Trusted glyph/SVG markup for the mark; defaults to a neutral inbox icon. */
  icon?: string;
  /** Optional call-to-action. Render the result, then wire the button by `id`. */
  action?: { id: string; label: string; variant?: ButtonVariant };
}

/** A produced empty state: a soft mark, a title, optional teaching copy, and an
 *  optional CTA. Replaces bare `<p class="empty">…</p>` so "no data yet" reads as
 *  an invitation, not a dead end. Title/hint are escaped; icon is trusted. */
export function emptyState(o: EmptyStateOpts): string {
  const hint = o.hint ? `<p class="ui-empty-hint">${escapeHtml(o.hint)}</p>` : "";
  const action = o.action
    ? button({ id: o.action.id, label: o.action.label, variant: o.action.variant ?? "primary", cls: "ui-empty-action" })
    : "";
  return `<div class="ui-empty">
  <div class="ui-empty-mark" aria-hidden="true">${o.icon ?? ICON_INBOX}</div>
  <h3 class="ui-empty-title">${escapeHtml(o.title)}</h3>
  ${hint}${action}
</div>`;
}

export interface ErrorStateOpts {
  retry?: { id: string; label: string };
  /** Trusted glyph/SVG markup for the mark; defaults to an alert triangle. */
  icon?: string;
}

/** A produced error state, flagged `role="alert"` for assistive tech. Pass the
 *  already-humanized message (e.g. `asMessage(err)`); it is escaped here. An
 *  optional retry button is wired by the caller via its `id`. */
export function errorState(message: string, o: ErrorStateOpts = {}): string {
  const retry = o.retry
    ? button({ id: o.retry.id, label: o.retry.label, variant: "ghost", cls: "ui-error-retry" })
    : "";
  return `<div class="ui-error" role="alert">
  <div class="ui-error-mark" aria-hidden="true">${o.icon ?? ICON_ALERT}</div>
  <div class="ui-error-body">
    <p class="ui-error-msg">${escapeHtml(message)}</p>
    ${retry}
  </div>
</div>`;
}

export interface LoadingStateOpts {
  label?: string;
}

/** A produced loading state: a spinner + a polite status label. For non-table
 *  loads; data tables keep the inventory.ts skeletons. */
export function loadingState(o: LoadingStateOpts = {}): string {
  const label = o.label ?? "Cargando…";
  return `<div class="ui-loading" role="status" aria-live="polite">
  <span class="ui-spinner" aria-hidden="true"></span>
  <span class="ui-loading-label">${escapeHtml(label)}</span>
</div>`;
}

/** `n` shimmer placeholder lines (clamped to ≥1) for a generic loading block. */
export function skeletonLines(n: number): string {
  const count = Math.max(1, Math.floor(n));
  const lines = Array.from({ length: count }, () => `<span class="ui-sk-line"></span>`).join("");
  return `<div class="ui-skeleton" aria-hidden="true">${lines}</div>`;
}

export interface CardOpts {
  /** Escaped, rendered in the header when present. */
  title?: string;
  /** TRUSTED composed markup — the caller's responsibility to escape its parts. */
  body: string;
  /** TRUSTED markup for a right-aligned actions slot in the header. */
  actions?: string;
}

/** A themed card shell. `body`/`actions` are trusted composed markup (a view
 *  builds them with `escapeHtml` on its own data); only `title` is escaped. */
export function card(o: CardOpts): string {
  const head =
    o.title || o.actions
      ? `<div class="ui-card-head">${o.title ? `<h3 class="ui-card-title">${escapeHtml(o.title)}</h3>` : ""}${o.actions ? `<div class="ui-card-actions">${o.actions}</div>` : ""}</div>`
      : "";
  return `<div class="ui-card">${head}<div class="ui-card-body">${o.body}</div></div>`;
}

// --- default marks (inline SVG, stroke=currentColor so the token theme paints) ---
const ICON_INBOX =
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/></svg>`;
const ICON_ALERT =
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>`;
