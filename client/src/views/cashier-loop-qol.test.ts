// W4 POS quality-of-life: hold/recall (ventas en espera) + quick-discount entry
// (monto OR %). Pure state logic the POS view drives — no DOM. We assert the
// parked snapshot is frozen (independent of the live cart), recall pops it back
// exactly once, labels don't reuse a recalled number, and a "%" discount resolves
// to clamped whole pesos against the right base.
import { describe, it, expect } from "vitest";
import {
  addToCart,
  holdSale,
  recallSale,
  nextHoldLabel,
  cloneCart,
  parseDiscountEntry,
  payableTotal,
  type CartLine,
  type Sellable,
  type HeldSale,
} from "./cashier-loop";

const prod = (id: string, price: string, stock = 99): Sellable => ({
  id,
  name: id,
  price,
  stock,
});

function buildCart(...lines: [string, string, number][]): CartLine[] {
  const cart: CartLine[] = [];
  for (const [id, price, qty] of lines) {
    for (let i = 0; i < qty; i++) addToCart(cart, prod(id, price));
  }
  return cart;
}

describe("hold / recall — ventas en espera", () => {
  it("parks a frozen snapshot independent of the live cart", () => {
    const cart = buildCart(["a", "1000", 2]);
    const { held, sale } = holdSale([], { lines: cart, globalDiscount: 0, id: "h1" });

    // Mutating the live cart after holding must not bleed into the parked sale.
    cart[0].qty = 99;
    cart.push({ product: "x", name: "x", unit_price: "500", qty: 1, stock: 5 });

    expect(held).toHaveLength(1);
    expect(sale.lines).toHaveLength(1);
    expect(sale.lines[0].qty).toBe(2);
  });

  it("recall pops the sale back exactly once", () => {
    let held: HeldSale[] = [];
    ({ held } = holdSale(held, { lines: buildCart(["a", "1000", 1]), id: "h1" }));
    ({ held } = holdSale(held, { lines: buildCart(["b", "2000", 3]), id: "h2" }));
    expect(held).toHaveLength(2);

    const r = recallSale(held, "h1");
    expect(r).not.toBeNull();
    expect(r!.sale.lines[0].product).toBe("a");
    expect(r!.held).toHaveLength(1);
    expect(r!.held[0].id).toBe("h2");

    // Already recalled → second recall finds nothing.
    expect(recallSale(r!.held, "h1")).toBeNull();
  });

  it("recalled cart is a fresh clone (editing it can't mutate the snapshot)", () => {
    const { held } = holdSale([], { lines: buildCart(["a", "1000", 2]), id: "h1" });
    const r = recallSale(held, "h1")!;
    r.sale.lines[0].qty = 50;
    // The original parked snapshot (still in `held`) is untouched.
    expect(held[0].lines[0].qty).toBe(2);
  });

  it("labels use the next free 'Espera N', never reusing a recalled number", () => {
    let held: HeldSale[] = [];
    ({ held } = holdSale(held, { lines: buildCart(["a", "1000", 1]) })); // Espera 1
    ({ held } = holdSale(held, { lines: buildCart(["b", "2000", 1]) })); // Espera 2
    expect(held.map((h) => h.label)).toEqual(["Espera 1", "Espera 2"]);

    const r = recallSale(held, held[0].id)!; // recall Espera 1
    expect(nextHoldLabel(r.held)).toBe("Espera 3"); // not "Espera 1" again
  });

  it("keeps the global discount and customer with the parked sale", () => {
    const { sale } = holdSale([], {
      lines: buildCart(["a", "1000", 1]),
      globalDiscount: 250,
      customer: { id: "c1", name: "Ana", points: 40 },
      id: "h1",
    });
    expect(sale.globalDiscount).toBe(250);
    expect(sale.customer).toEqual({ id: "c1", name: "Ana", points: 40 });
  });

  it("cloneCart deep-copies flat lines", () => {
    const cart = buildCart(["a", "1000", 1]);
    const copy = cloneCart(cart);
    copy[0].qty = 9;
    expect(cart[0].qty).toBe(1);
  });
});

describe("parseDiscountEntry — monto OR %", () => {
  it("reads a flat peso amount, dropping grouping", () => {
    expect(parseDiscountEntry("1.500", 10000)).toBe(1500);
    expect(parseDiscountEntry("2000", 10000)).toBe(2000);
  });

  it("resolves a percentage against the base to whole pesos", () => {
    expect(parseDiscountEntry("10%", 10000)).toBe(1000);
    expect(parseDiscountEntry("10 %", 10000)).toBe(1000);
    expect(parseDiscountEntry("12,5%", 10000)).toBe(1250);
  });

  it("clamps over-base entries to the base (never negative, never over)", () => {
    expect(parseDiscountEntry("200%", 5000)).toBe(5000); // can't exceed the base
    expect(parseDiscountEntry("99999", 5000)).toBe(5000);
    expect(parseDiscountEntry("0%", 5000)).toBe(0);
  });

  it("blank / garbage → 0", () => {
    expect(parseDiscountEntry("", 5000)).toBe(0);
    expect(parseDiscountEntry("   ", 5000)).toBe(0);
    expect(parseDiscountEntry("abc", 5000)).toBe(0);
    expect(parseDiscountEntry("%", 5000)).toBe(0);
  });

  it("a % entry feeds the existing payable math as plain pesos", () => {
    const cart = buildCart(["a", "10000", 1]);
    const global = parseDiscountEntry("15%", 10000); // 1500
    expect(payableTotal(cart, global)).toBe(8500);
  });
});
