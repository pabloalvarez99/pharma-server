// PHASE 2 perf lane (marvin) — the INVENTORY render budget at 50k SKUs.
//
// Backend stock is already fast (perf-001: stock_stats p99 2.7s → 1.7ms). This
// file guards the CLIENT side: that the views don't re-introduce the cliff in
// the DOM. Audit result of the three inventory feeds:
//   • Productos  → server-capped at PAGE_LIMIT 60 (listProducts limit). Bounded.
//   • Compras/Gastos (marvin scope) → server-capped 60/100. Bounded.
//   • Vencimientos (near-expiry) → NO server limit. UNBOUNDED.
//   • Rotación   (stock-rotation) → NO server limit. UNBOUNDED.
// The two unbounded feeds drove `host.innerHTML = rows.map(...).join("")` over
// the whole catalog → at 50k SKUs the browser parses + lays out 50k <tr> in one
// frame (multi-hundred-ms jank). Fix = `capRows`: both feeds arrive pre-ordered
// by urgency (FEFO) / ABC, so the top-N window is exactly what the operator acts
// on; the long tail goes to export. These journeys prove the cap bounds the row
// count handed to innerHTML, preserves the important head, and that building the
// capped string stays inside a one-frame budget while the uncapped 50k build is
// measurably larger. Both verticals exercised (fármacos + minimarket perecibles).
import { describe, it, expect } from "vitest";
import {
  capRows,
  LIST_RENDER_CAP,
  nearExpiryView,
  rotacionRows,
} from "./stock-helpers";

// Row-string builders mirroring the real (module-private) nearRow / rotacionRow
// template size, so the measured build cost is representative of the view.
const nearRowHtml = (r: { product_name: string; batch_code: string; days_to_expiry: number; stock: number; tone: string; label: string }): string =>
  `<tr data-id="p" class="inv-row" tabindex="0"><td>${r.product_name}</td><td>${r.batch_code}</td><td>01/01/2026</td><td class="num">${r.stock}</td><td class="num">${r.days_to_expiry}</td><td><span class="pill pill-${r.tone}">${r.label}</span></td></tr>`;

function makeNearFeed(n: number, vertical: "pharmacy" | "minimarket" = "pharmacy") {
  const name = vertical === "pharmacy" ? "Paracetamol" : "Leche 1L";
  return Array.from({ length: n }, (_, i) => ({
    product_id: `p${i}`,
    product_name: `${name} ${i}`,
    batch_code: `B-${i}`,
    // Shuffle urgency so the FEFO sort actually has work to do; a deterministic
    // spread from very-overdue to far-future.
    days_to_expiry: ((i * 7919) % 400) - 100,
    expired: false,
    stock: (i % 50) + 1,
  }));
}

function makeRotationFeed(n: number) {
  return Array.from({ length: n }, (_, i) => ({
    product_id: `p${i}`,
    product_name: `SKU ${i}`,
    qty_sold: (i * 104729) % 5000,
    current_stock: i % 200,
  }));
}

// --------------------------------------------------------------------------
// Journey 1 — small lists pass through untouched (no cap, no footer change).
//   A real pharmacy with a few hundred lotes must see ALL of them, verbatim.
// --------------------------------------------------------------------------
describe("journey · capRows — bajo presupuesto NO trunca", () => {
  it("una lista dentro del budget se devuelve intacta", () => {
    const rows = makeNearFeed(150);
    const c = capRows(rows);
    expect(c.truncated).toBe(false);
    expect(c.total).toBe(150);
    expect(c.rows).toBe(rows); // same reference — zero copy, zero mutation
  });

  it("exactamente en el límite no trunca", () => {
    const c = capRows(makeNearFeed(LIST_RENDER_CAP));
    expect(c.truncated).toBe(false);
    expect(c.rows.length).toBe(LIST_RENDER_CAP);
  });
});

// --------------------------------------------------------------------------
// Journey 2 — 50k SKUs: the cap bounds the rows handed to innerHTML.
// --------------------------------------------------------------------------
describe("journey · capRows — 50k SKUs se acotan al budget", () => {
  it("trunca a LIST_RENDER_CAP y reporta el total real", () => {
    const c = capRows(makeNearFeed(50_000));
    expect(c.rows.length).toBe(LIST_RENDER_CAP);
    expect(c.total).toBe(50_000);
    expect(c.truncated).toBe(true);
  });

  it("no muta el arreglo de entrada", () => {
    const rows = makeNearFeed(50_000);
    const len = rows.length;
    capRows(rows);
    expect(rows.length).toBe(len);
  });
});

// --------------------------------------------------------------------------
// Journey 3 — Vencimientos: the cap keeps the MOST URGENT lotes (FEFO head).
//   Truncating must never drop a caducado in favour of a far-future lote.
// --------------------------------------------------------------------------
describe("journey · vencimientos — el cap conserva lo más urgente (FEFO)", () => {
  it("tras ordenar por urgencia, el top-N son los días más bajos del catálogo", () => {
    const feed = makeNearFeed(50_000);
    const ordered = nearExpiryView(feed);
    const { rows } = capRows(ordered);
    // Head is the lowest days_to_expiry across ALL 50k, ascending.
    const allDays = feed.map((r) => r.days_to_expiry).sort((a, b) => a - b);
    expect(rows.map((r) => r.days_to_expiry)).toEqual(allDays.slice(0, LIST_RENDER_CAP));
    // The most overdue lote of the whole catalog survives the cap.
    expect(rows[0].days_to_expiry).toBe(allDays[0]);
    expect(rows[0].tone).toBe("danger");
  });
});

// --------------------------------------------------------------------------
// Journey 4 — Rotación: the cap keeps the A-class movers (ABC head).
// --------------------------------------------------------------------------
describe("journey · rotación — el cap conserva el top por rotación (ABC)", () => {
  it("el top-N son los más vendidos del catálogo, orden desc", () => {
    const feed = makeRotationFeed(50_000);
    const ranked = rotacionRows(feed);
    const { rows, total, truncated } = capRows(ranked);
    expect(truncated).toBe(true);
    expect(total).toBe(50_000);
    expect(rows.length).toBe(LIST_RENDER_CAP);
    const topSold = feed.map((r) => r.qty_sold).sort((a, b) => b - a).slice(0, LIST_RENDER_CAP);
    expect(rows.map((r) => r.qty_sold)).toEqual(topSold);
    // Highest-selling product of all 50k is row 0 and never a C.
    expect(rows[0].qty_sold).toBe(topSold[0]);
  });
});

// --------------------------------------------------------------------------
// Journey 5 — render budget: building the CAPPED string is one-frame cheap,
//   the UNCAPPED 50k build is measurably (orders of magnitude) larger. This is
//   the antes/después evidence for the DOM cliff the cap removes.
// --------------------------------------------------------------------------
describe("journey · presupuesto de render — capped << uncapped @50k", () => {
  it("el string capeado se construye dentro de un frame y queda acotado", () => {
    const feed = makeNearFeed(50_000);
    const ordered = nearExpiryView(feed);

    // ANTES: full catalog → the innerHTML the browser would have to parse.
    const t0 = performance.now();
    const uncappedHtml = ordered.map(nearRowHtml).join("");
    const uncappedMs = performance.now() - t0;

    // DESPUÉS: capped → what we actually hand to innerHTML now.
    const { rows } = capRows(ordered);
    const t1 = performance.now();
    const cappedHtml = rows.map(nearRowHtml).join("");
    const cappedMs = performance.now() - t1;

    // Row count handed to the DOM drops from 50k to the cap — this is the cliff.
    expect(rows.length).toBe(LIST_RENDER_CAP);
    expect(uncappedHtml.length).toBeGreaterThan(cappedHtml.length * 100);
    // Capped string build is trivially under a 16ms frame budget. (Generous
    // ceiling for slow CI; in practice sub-millisecond.)
    expect(cappedMs).toBeLessThan(16);

    // Visibility for the run log (not an assertion — perf numbers vary by host).
    // eslint-disable-next-line no-console
    console.info(
      `[inv-perf] near-expiry @50k — uncapped build ${uncappedMs.toFixed(1)}ms / ${uncappedHtml.length} chars · capped ${cappedMs.toFixed(2)}ms / ${cappedHtml.length} chars`,
    );
  });
});

// --------------------------------------------------------------------------
// Journey 6 — MULTI-RUBRO: a 50k-SKU minimarket (perecibles, no fármacos) hits
//   the exact same cap path; the budget logic is rubro-agnostic.
// --------------------------------------------------------------------------
describe("journey · multi-rubro — minimarket 50k perecibles capea igual", () => {
  it("leche/pan caducados se acotan al budget como los fármacos", () => {
    const feed = makeNearFeed(50_000, "minimarket");
    const { rows, total, truncated } = capRows(nearExpiryView(feed));
    expect(truncated).toBe(true);
    expect(total).toBe(50_000);
    expect(rows.length).toBe(LIST_RENDER_CAP);
    expect(rows[0].product_name.startsWith("Leche 1L")).toBe(true);
  });
});
