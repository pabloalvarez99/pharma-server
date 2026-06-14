// User-journey tests for the cashier daily loop. Each `describe` is one operator
// scenario run end-to-end against the PURE state logic the POS / caja /
// devoluciones views drive (cashier-loop.ts) — no clicks, no DOM: we mutate the
// same cart/arqueo/refund state the views mutate and assert the money + stock at
// every step, in BOTH verticals (pharmacy + minimarket) where it matters.
import { describe, it, expect } from "vitest";
import {
  addToCart,
  changeQty,
  cartTotal,
  expectedCash,
  discrepancy,
  deriveTipo,
  validateRefund,
  type CartLine,
  type Sellable,
  type RefundDraftLine,
} from "./cashier-loop";
import { clp, parseCash, effectiveTender, vuelto, quickCashAmounts } from "../format";

// Believable seed-shaped products for both verticals (mirrors `seed-demo`):
const PHARMACY: Record<string, Sellable> = {
  paracetamol: { id: "product:para", name: "Paracetamol 500mg", price: "1290", stock: 40 },
  ibuprofeno: { id: "product:ibu", name: "Ibuprofeno 400mg", price: "2490", stock: 3 },
  agotado: { id: "product:nil", name: "Amoxicilina (agotado)", price: "3990", stock: 0 },
};
const MINIMARKET: Record<string, Sellable> = {
  pan: { id: "product:pan", name: "Pan amasado", price: "1500", stock: 25 },
  leche: { id: "product:lec", name: "Leche 1L", price: "1190", stock: 12 },
};

describe("journey 1 · pharmacy cash sale with vuelto", () => {
  it("open caja → sell 2 lines → pay cash → boleta total + change", () => {
    // Open caja with 20.000 float.
    const opening = 20000;

    // Sell: 2× Paracetamol, 1× Ibuprofeno.
    const cart: CartLine[] = [];
    addToCart(cart, PHARMACY.paracetamol);
    addToCart(cart, PHARMACY.paracetamol);
    addToCart(cart, PHARMACY.ibuprofeno);
    expect(cart).toHaveLength(2);
    expect(cart[0].qty).toBe(2);

    const total = cartTotal(cart);
    expect(total).toBe(1290 * 2 + 2490); // 5070
    expect(clp(total)).toBe("$5.070");

    // Cashier tenders 10.000 → vuelto 4.930, server gets the tendered amount.
    const tendered = parseCash("$10.000");
    expect(tendered).toBe(10000);
    const v = vuelto(tendered, total);
    expect(v).toEqual({ kind: "ok", amount: 4930 });
    expect(effectiveTender(tendered, total)).toBe(10000);

    // The opening float is untouched by the math above (sanity for arqueo step).
    expect(opening).toBe(20000);
  });
});

describe("journey 2 · minimarket sale, exact cash, no vuelto", () => {
  it("sells abarrotes with exact tender → vuelto kind none", () => {
    const cart: CartLine[] = [];
    addToCart(cart, MINIMARKET.pan);
    addToCart(cart, MINIMARKET.leche);
    addToCart(cart, MINIMARKET.leche);
    const total = cartTotal(cart);
    expect(total).toBe(1500 + 1190 * 2); // 3880

    // Quick-cash chips: exact first, then the next round bills.
    const chips = quickCashAmounts(total);
    expect(chips[0]).toBe(3880);
    expect(chips).toContain(4000);
    expect(chips.length).toBeLessThanOrEqual(4);

    // Exact tender → no change due.
    expect(vuelto(3880, total)).toEqual({ kind: "ok", amount: 0 });
    expect(effectiveTender(3880, total)).toBe(3880);
  });
});

describe("journey 3 · stock guards (edge: stock 0 and stock cap)", () => {
  it("cannot add an out-of-stock product", () => {
    const cart: CartLine[] = [];
    addToCart(cart, PHARMACY.agotado);
    expect(cart).toHaveLength(0);
    expect(cartTotal(cart)).toBe(0);
  });

  it("never carts more units than the shelf holds", () => {
    const cart: CartLine[] = [];
    // Ibuprofeno has stock 3 — try to add 5.
    for (let i = 0; i < 5; i++) addToCart(cart, PHARMACY.ibuprofeno);
    expect(cart[0].qty).toBe(3);
    // changeQty also clamps to stock.
    changeQty(cart, PHARMACY.ibuprofeno.id, +10);
    expect(cart[0].qty).toBe(3);
  });

  it("decrementing to zero removes the line, returning to an empty cart", () => {
    const cart: CartLine[] = [];
    addToCart(cart, MINIMARKET.pan);
    changeQty(cart, MINIMARKET.pan.id, -1);
    expect(cart).toHaveLength(0);
    expect(cartTotal(cart)).toBe(0);
  });
});

describe("journey 4 · short tender never yields a negative vuelto", () => {
  it("under-payment shows a shortfall, and tender floors to the total", () => {
    const cart: CartLine[] = [];
    addToCart(cart, PHARMACY.paracetamol); // 1290
    const total = cartTotal(cart);

    // Cashier types less than due.
    const short = parseCash("1000");
    expect(vuelto(short, total)).toEqual({ kind: "short", amount: 290 });
    // A short tender is never sent as-is — it floors to the total so the server
    // balance check can't be tricked into a negative change.
    expect(effectiveTender(short, total)).toBe(total);

    // A stray minus sign can't produce a negative tender either.
    expect(parseCash("-5000")).toBe(5000);
  });
});

describe("journey 5 · partial refund over a prior sale", () => {
  it("returns 1 of 2 units → Parcial, correct money, restock stays off", () => {
    // Boleta lines from journey 1's sale (no product id on boleta lines).
    const lines: RefundDraftLine[] = [
      { name: "Paracetamol 500mg", sold: 2, qty: 1, unit_price: "1290" },
      { name: "Ibuprofeno 400mg", sold: 1, qty: 0, unit_price: "2490" },
    ];
    expect(deriveTipo(lines)).toBe("parcial");

    const r = validateRefund(lines, { restock: true });
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.value.tipo).toBe("parcial");
      expect(r.value.total).toBe(1290);
      expect(r.value.items).toHaveLength(1);
      // restock requested but boleta line has no product id → stays false.
      expect(r.value.items[0].restock).toBe(false);
    }
  });

  it("rejects returning more than was sold, in Spanish", () => {
    const lines: RefundDraftLine[] = [
      { name: "Pan amasado", sold: 2, qty: 3, unit_price: "1500" },
    ];
    const r = validateRefund(lines);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toContain("más de lo vendido");
  });

  it("rejects an empty refund (no quantities picked)", () => {
    const lines: RefundDraftLine[] = [
      { name: "Leche 1L", sold: 3, qty: 0, unit_price: "1190" },
    ];
    const r = validateRefund(lines);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toContain("al menos una cantidad");
  });
});

describe("journey 6 · full refund derives Total", () => {
  it("every sold unit returned → Total, restock on when product known", () => {
    const lines: RefundDraftLine[] = [
      { name: "Pan amasado", sold: 2, qty: 2, unit_price: "1500", product: "product:pan" },
    ];
    expect(deriveTipo(lines)).toBe("total");
    const r = validateRefund(lines, { restock: true });
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.value.tipo).toBe("total");
      expect(r.value.total).toBe(3000);
      expect(r.value.items[0].restock).toBe(true); // product id present
    }
  });
});

describe("journey 7 · arqueo + cierre (balanced, over, short)", () => {
  // A shift: opened with 20.000, took 5.070 + 3.880 in cash sales, one 2.000
  // ingreso (vuelto change fund), 1.000 egreso (petty cash).
  const arqueo = {
    opening_cash: "20000",
    cash_sales: "8950",
    movements_in: "2000",
    movements_out: "1000",
  };

  it("computes expected drawer cash", () => {
    expect(expectedCash(arqueo)).toBe(20000 + 8950 + 2000 - 1000); // 29950
  });

  it("counted == expected → Cuadra", () => {
    const d = discrepancy(29950, expectedCash(arqueo));
    expect(d).toEqual({ tone: "ok", label: "Cuadra", amount: 0 });
  });

  it("counted > expected → Sobrante (positive amount, never signed)", () => {
    const d = discrepancy(30950, expectedCash(arqueo));
    expect(d.tone).toBe("warn");
    expect(d.label).toBe("Sobrante");
    expect(d.amount).toBe(1000);
  });

  it("counted < expected → Faltante", () => {
    const d = discrepancy(28000, expectedCash(arqueo));
    expect(d.tone).toBe("danger");
    expect(d.label).toBe("Faltante");
    expect(d.amount).toBe(1950);
  });
});

describe("journey 8 · garbage money inputs never corrupt totals", () => {
  it("non-numeric price floors to 0; NaN never reaches the total", () => {
    const cart: CartLine[] = [
      { product: "x", name: "corrupto", unit_price: "no-es-numero", qty: 2, stock: 9 },
    ];
    expect(cartTotal(cart)).toBe(0);
    expect(clp(cartTotal(cart))).toBe("$0");
  });

  it("empty / garbage tender parses to 0, shows no vuelto", () => {
    expect(parseCash("")).toBe(0);
    expect(parseCash("abc")).toBe(0);
    expect(vuelto(0, 5000)).toEqual({ kind: "none", amount: 0 });
  });
});
