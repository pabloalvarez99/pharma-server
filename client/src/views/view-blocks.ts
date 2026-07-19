// Shared view building-blocks — the canonical home for the HTML block helpers
// that used to live at the bottom of inventory.ts (16 views imported them from
// there). Pure string producers, no DOM; the only exception is
// `attachRutAdvisory`, which wires a live RUT check onto an input.
//
// inventory.ts re-exports everything here so existing `from "./inventory"`
// call sites keep working; new code should import from "./view-blocks".

import { isValidRut, canonicalRut, formatRut } from "../format";
import { classifyFetchError, type EmptyCopy } from "./stock-helpers";

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );
}

export function asMessage(err: unknown): string {
  return typeof err === "string" ? err : "No se pudo cargar la información.";
}

export function kpiCard(label: string, value: string, sub: string, tone = ""): string {
  return `
    <div class="kpi-card ${tone ? `kpi-${tone}` : ""}">
      <span class="kpi-label">${escapeHtml(label)}</span>
      <strong class="kpi-value">${value}</strong>
      <span class="kpi-sub muted">${escapeHtml(sub)}</span>
    </div>
  `;
}

export function kpiSkeleton(n = 4): string {
  return Array.from({ length: n })
    .map(() => `<div class="kpi-card skel"><span class="sk sk-sm"></span><span class="sk sk-lg"></span><span class="sk sk-sm"></span></div>`)
    .join("");
}

export function tableSkeleton(rows = 6): string {
  return `<div class="table-skel">${Array.from({ length: rows })
    .map(() => `<div class="sk sk-row"></div>`)
    .join("")}</div>`;
}

export function errorCard(err: unknown): string {
  return `<div class="kpi-card kpi-danger kpi-span"><span class="kpi-label">Error</span><strong class="kpi-value sm">${escapeHtml(asMessage(err))}</strong></div>`;
}

/** Centered empty-state block (reuses the global `.caja-empty` chrome) — never a
 *  blank screen. Pass `ctaId` to render the call-to-action button; the caller
 *  wires its click. Shared by inventory / compras / gastos. */
export function emptyStateHtml(c: EmptyCopy, ctaId?: string): string {
  const cta =
    c.cta && ctaId
      ? `<button type="button" class="btn-primary empty-cta" id="${escapeHtml(ctaId)}"><span class="btn-label">${escapeHtml(c.cta)}</span></button>`
      : "";
  return `
    <div class="caja-empty">
      <div class="caja-empty-mark">●</div>
      <h3>${escapeHtml(c.title)}</h3>
      <p class="muted">${escapeHtml(c.hint)}</p>
      ${cta}
    </div>
  `;
}

/** Operator-facing error block. A permission / offline failure gets the friendly
 *  centered `.caja-empty` treatment with an actionable hint; an unclassified
 *  error keeps the raw message in the red `.view-error` band so nothing real is
 *  hidden. `resource` customizes the "sin acceso" line ("las compras"). */
export function errorStateHtml(err: unknown, resource?: string): string {
  const c = classifyFetchError(err, resource);
  if (c.kind === "generic") {
    return `<div class="view-error">${escapeHtml(c.hint)}</div>`;
  }
  return `
    <div class="caja-empty">
      <div class="caja-empty-mark">●</div>
      <h3>${escapeHtml(c.title)}</h3>
      <p class="muted">${escapeHtml(c.hint)}</p>
    </div>
  `;
}

/** Wire a live, **advisory** mód-11 check onto an optional RUT input + its hint
 *  element: green echo when valid, amber warning when the verifier doesn't
 *  match (never blocks — the field is optional registry data), and a blur that
 *  rewrites a valid RUT to the pretty `NN.NNN.NNN-D` form. Used by the customer
 *  and supplier forms; the DTE emisor/receptor use their own *blocking* check.
 *  Returns a `canonical()` getter the caller uses on save to store the SII
 *  `NNNNNNNN-D` form for valid RUTs (raw passthrough otherwise). */
export function attachRutAdvisory(
  input: HTMLInputElement,
  hint: HTMLElement,
): { canonical: () => string } {
  const check = (): void => {
    const raw = input.value.trim();
    if (!raw) {
      hint.hidden = true;
      hint.className = "field-hint";
      return;
    }
    if (isValidRut(raw)) {
      hint.hidden = false;
      hint.className = "field-hint ok";
      hint.textContent = `RUT válido — ${formatRut(raw)}`;
    } else {
      hint.hidden = false;
      hint.className = "field-hint err";
      hint.textContent = "El dígito verificador no calza; revísalo (puedes guardar igual).";
    }
  };
  input.addEventListener("input", check);
  input.addEventListener("blur", () => {
    if (isValidRut(input.value)) input.value = formatRut(input.value);
  });
  check();
  return {
    canonical: () => {
      const raw = input.value.trim();
      return isValidRut(raw) ? canonicalRut(raw) : raw;
    },
  };
}
