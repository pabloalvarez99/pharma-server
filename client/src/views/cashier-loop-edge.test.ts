// Deep edge-case journeys for the cashier daily loop — the "weird" real-counter
// situations beyond the happy path in cashier-loop.test.ts: garbage tender, round
// vs sub-1000 quick-cash, exact-cover splits, discount-then-refund money, FEFO-
// style restock gating, and the multi-tender overpay-on-cash invariant. These
// LOCK behaviour a live cashier can hit so it can't silently regress. Pure logic,
// no DOM — same state the pos/caja/devoluciones views drive. Both verticals.
import { describe, it, expect } from "vitest";
import {
  addToCart,
  changeQty,
  cartTotal,
  orderDiscount,
  payableTotal,
  splitPayment,
  remainingOnCard,
  cashTenderShort,
  validateRefund,
  deriveTipo,
  expectedCash,
  discrepancy,
  nextDrawerName,
  type CartLine,
  type Sellable,
  type RefundDraftLine,
} from "./cashier-loop";
import {
  parseCash,
  vuelto,
  effectiveTender,
  quickCashAmounts,
  clp,
} from "../format";

const PARA: Sellable = { id: "product:para", name: "Paracetamol 500mg", price: "1290", stock: 40 };
const IBU: Sellable = { id: "product:ibu", name: "Ibuprofeno 400mg", price: "2490", stock: 3 };
const PAN: Sellable = { id: "product:pan", name: "Pan amasado", price: "1500", stock: 25 };

describe("edge · quick-cash chips at awkward totals", () => {
  it("a total that is already a round 1k bill yields the exact chip only once", () => {
    // round=5000, ceil1k=5000 dedup → just [5000, 10000].
    expect(quickCashAmounts(5000)).toEqual([5000, 10000]);
  });

  it("a sub-1000 total still offers the next round bills", () => {
    expect(quickCashAmounts(990)).toEqual([990, 1000, 5000, 10000]);
  });

  it("chips are always distinct, positive and ascending (never confuse the cashier)", () => {
    for (const t of [1, 50, 499, 1290, 3880, 5070, 9999, 12345, 49999, 100001]) {
      const chips = quickCashAmounts(t);
      expect(chips.length).toBeLessThanOrEqual(4);
      expect(new Set(chips).size).toBe(chips.length); // distinct
      for (let i = 1; i < chips.length; i++) expect(chips[i]).toBeGreaterThan(chips[i - 1]);
      expect(chips[0]).toBe(t); // exact amount always first
      expect(chips.every((c) => c > 0)).toBe(true);
    }
  });

  it("a non-positive total has no chips", () => {
    expect(quickCashAmounts(0)).toEqual([]);
    expect(quickCashAmounts(-100)).toEqual([]);
  });
});

describe("edge · garbage / hostile tender input never breaks the till", () => {
  it("CL-formatted cash with dots + sign parses to the digits, never negative", () => {
    expect(parseCash("$10.000")).toBe(10000);
    expect(parseCash("-5.000")).toBe(5000); // stray minus stripped
    expect(parseCash("1.234.567")).toBe(1234567);
    expect(parseCash("  ")).toBe(0);
    expect(parseCash("abc")).toBe(0);
    expect(parseCash("0")).toBe(0);
  });

  it("an exact tender shows zero vuelto, not 'none'", () => {
    expect(vuelto(5070, 5070)).toEqual({ kind: "ok", amount: 0 });
    expect(effectiveTender(5070, 5070)).toBe(5070);
  });

  it("a one-peso short tender is flagged short and floored up to the total", () => {
    expect(vuelto(5069, 5070)).toEqual({ kind: "short", amount: 1 });
    expect(effectiveTender(5069, 5070)).toBe(5070); // never underpay the server
  });

  it("an enormous fat-finger tender still computes a sane vuelto", () => {
    const huge = parseCash("999999999"); // ~1e9
    const v = vuelto(huge, 1290);
    expect(v.kind).toBe("ok");
    expect(v.amount).toBe(huge - 1290);
  });
});

describe("edge · split tender (Mixto) covers the awkward cases", () => {
  it("an empty-cart Mixto (total 0) can never be charged", () => {
    // pos.ts chargeEnabled() gates Mixto on splitPayment().ok; total 0 → not ok.
    const s = splitPayment({ cash: 0, card: 0 }, 0);
    expect(s.ok).toBe(false);
  });

  it("exact split to the peso settles with no vuelto", () => {
    const s = splitPayment({ cash: 1780, card: 2000 }, 3780);
    expect(s).toEqual({ ok: true, tendered: 3780, change: 0, short: 0 });
  });

  it("overpayment in a split falls entirely on the cash side as vuelto", () => {
    // Invariant (cashier-loop comment): you can't over-charge a card, so the
    // change = (cash+card) − total regardless of which side overpaid.
    const s = splitPayment({ cash: 5000, card: 2000 }, 3780);
    expect(s.ok).toBe(true);
    expect(s.change).toBe(3220);
    expect(clp(s.change)).toBe("$3.220");
  });

  it("both tenders garbage → short by the whole total, no negative anything", () => {
    const s = splitPayment({ cash: NaN, card: -999 }, 3780);
    expect(s.tendered).toBe(0);
    expect(s.ok).toBe(false);
    expect(s.short).toBe(3780);
    expect(s.change).toBe(0);
  });
});

describe("edge · discount then refund — the money must stay consistent", () => {
  it("a refund is computed on the LINE prices, not the discounted net (server refunds gross lines)", () => {
    // Sale: 2× Paracetamol (gross 2580) with a 580 line discount → paid 2000.
    const cart: CartLine[] = [];
    addToCart(cart, PARA);
    addToCart(cart, PARA);
    cart[0].discount = 580;
    expect(payableTotal(cart)).toBe(2000);

    // Boleta lines carry the per-unit price; the refund draft returns 1 unit.
    const lines: RefundDraftLine[] = [
      { name: PARA.name, sold: 2, qty: 1, unit_price: "1290" },
    ];
    const r = validateRefund(lines);
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.value.total).toBe(1290); // one unit at list price
  });

  it("global discount capped so the payable can never go negative, even absurd", () => {
    const cart: CartLine[] = [];
    addToCart(cart, PAN); // 1500
    expect(orderDiscount(cart, 1_000_000)).toBe(1500);
    expect(payableTotal(cart, 1_000_000)).toBe(0);
  });
});

describe("edge · FEFO-style restock gating on returns", () => {
  it("restock sticks only when the line identifies a product (boleta lines don't)", () => {
    const boletaLine: RefundDraftLine = { name: PAN.name, sold: 1, qty: 1, unit_price: "1500" };
    const r1 = validateRefund([boletaLine], { restock: true });
    expect(r1.ok && r1.value.items[0].restock).toBe(false); // no product id → no restock

    const knownLine: RefundDraftLine = { ...boletaLine, product: "product:pan" };
    const r2 = validateRefund([knownLine], { restock: true });
    expect(r2.ok && r2.value.items[0].restock).toBe(true);

    const r3 = validateRefund([knownLine], { restock: false });
    expect(r3.ok && r3.value.items[0].restock).toBe(false); // toggle off wins
  });

  it("a non-integer or negative return qty is rejected in Spanish (number input fat-finger)", () => {
    const r1 = validateRefund([{ name: PAN.name, sold: 3, qty: 1.5, unit_price: "1500" }]);
    expect(r1.ok).toBe(false);
    if (!r1.ok) expect(r1.error).toContain("enteros");

    const r2 = validateRefund([{ name: PAN.name, sold: 3, qty: -1, unit_price: "1500" }]);
    expect(r2.ok).toBe(false);
  });

  it("full return across every line derives Total; a single partial line drops it to Parcial", () => {
    const full: RefundDraftLine[] = [
      { name: PARA.name, sold: 2, qty: 2, unit_price: "1290" },
      { name: IBU.name, sold: 1, qty: 1, unit_price: "2490" },
    ];
    expect(deriveTipo(full)).toBe("total");
    full[1].qty = 0;
    expect(deriveTipo(full)).toBe("parcial");
  });
});

describe("edge · stock cap survives keyboard hammering", () => {
  it("holding + on a low-stock line never exceeds the shelf, decrement past 0 drops it", () => {
    const cart: CartLine[] = [];
    for (let i = 0; i < 10; i++) addToCart(cart, IBU); // stock 3
    expect(cart[0].qty).toBe(3);
    changeQty(cart, IBU.id, +50);
    expect(cart[0].qty).toBe(3); // clamped
    changeQty(cart, IBU.id, -3);
    expect(cart).toHaveLength(0); // line removed at 0
    expect(cartTotal(cart)).toBe(0);
  });
});

describe("edge · arqueo with movements and a zero-float open", () => {
  it("opening with no float still reconciles cash sales + ingresos − egresos", () => {
    const exp = expectedCash({ opening_cash: "0", cash_sales: "8950", movements_in: "0", movements_out: "1000" });
    expect(exp).toBe(7950);
    expect(discrepancy(7950, exp)).toEqual({ tone: "ok", label: "Cuadra", amount: 0 });
    expect(discrepancy(7000, exp).label).toBe("Faltante");
    expect(discrepancy(8000, exp).label).toBe("Sobrante");
  });

  it("garbage string amounts in the arqueo floor to 0, never NaN", () => {
    const exp = expectedCash({ opening_cash: "abc", cash_sales: "", movements_in: "x", movements_out: "y" });
    expect(exp).toBe(0);
  });
});

describe("edge · multi-caja drawer naming under gaps and custom names", () => {
  it("a closed middle drawer leaves the next name at max+1, not a reused number", () => {
    expect(nextDrawerName(["caja-1", "caja-3"])).toBe("caja-4");
  });

  it("whitespace around a drawer name is tolerated when picking the next number", () => {
    expect(nextDrawerName([" caja-2 "])).toBe("caja-3");
  });

  it("only custom names fall back to count+1 so a second drawer never collides", () => {
    expect(nextDrawerName(["Mostrador"])).toBe("caja-2");
  });
});

describe("edge · mixto remainder + cash short (cashier UX)", () => {
  it("remainingOnCard fills the card leg when cash is partial", () => {
    expect(remainingOnCard(3000, 7500)).toBe(4500);
    expect(remainingOnCard(7500, 7500)).toBe(0);
    expect(remainingOnCard(9000, 7500)).toBe(0);
    expect(remainingOnCard(-5, 1000)).toBe(1000); // floors negatives
  });

  it("cashTenderShort: blank/0 is exact-path, partial typed amount is short", () => {
    expect(cashTenderShort(0, 5000)).toBe(false);
    expect(cashTenderShort(5000, 5000)).toBe(false);
    expect(cashTenderShort(4999, 5000)).toBe(true);
    expect(cashTenderShort(10000, 5000)).toBe(false);
  });

  it("splitPayment + remainingOnCard stay consistent for a common mix", () => {
    const total = 12990;
    const cash = 5000;
    const card = remainingOnCard(cash, total);
    const s = splitPayment({ cash, card }, total);
    expect(s.ok).toBe(true);
    expect(s.change).toBe(0);
    expect(s.tendered).toBe(total);
  });
});
