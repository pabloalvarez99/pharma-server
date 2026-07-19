// Per-rubro render tests for the cashier views (POS picker + devoluciones copy).
// The cash-loop MATH is covered in cashier-loop.test.ts; here we assert the
// VISUAL gate driven by the rubro signal (featuresForRubro().physicalStock):
// a service rubro (peluquería, oficios) must never show "Stock 0 · agotado" — it
// sells the item as a servicio — while a physical rubro keeps the stock line.
// Out-of-stock physical cards stay clickable (multi-SKU parent shell is often 0).
import { describe, it, expect } from "vitest";
import { resultCard, posSearchPlaceholder, friendlyPosError } from "./pos";
import { devolucionesSubtitle, restockControlHtml } from "./devoluciones";
import { featuresForRubro } from "../vertical";
import type { Product } from "../api";

const base: Omit<Product, "stock"> = {
  id: "product:x",
  name: "Corte de pelo",
  price: "12000",
  active: true,
  laboratory: null,
  active_ingredient: null,
};
const product = (stock: number): Product => ({ ...base, stock });

describe("POS picker card — physical rubro (trackStock=true)", () => {
  it("shows the stock count and stays sellable when in stock", () => {
    const html = resultCard(product(7), true);
    expect(html).toContain("Stock:");
    expect(html).toContain(">7<");
    expect(html).not.toContain("agotado");
    expect(html).not.toContain("disabled");
    expect(html).not.toContain("Servicio");
  });

  it("marks out-of-stock as Agotado but stays clickable (parent multi-SKU / sin stock)", () => {
    const html = resultCard(product(0), true);
    expect(html).toMatch(/agotado/i);
    expect(html).toMatch(/variantes/i);
    expect(html).not.toContain("disabled");
    expect(html).toContain("is-out");
  });
});

describe("POS picker card — service rubro (trackStock=false)", () => {
  it("reads 'Servicio', never shows stock/agotado, and stays sellable", () => {
    // Even a zero-stock product (services seed with stock 0) must be sellable.
    const html = resultCard(product(0), false);
    expect(html).toContain("Servicio");
    expect(html).toContain("is-service");
    expect(html).toContain("12.000"); // price still shown
    expect(html).not.toContain("Stock:");
    expect(html).not.toContain("agotado");
    expect(html).not.toContain("disabled");
  });
});

describe("rubro signal drives the stock gate", () => {
  it("service rubros are physicalStock:false → card hides stock", () => {
    for (const r of ["belleza", "servicios"]) {
      const track = featuresForRubro(r).physicalStock;
      expect(track).toBe(false);
      expect(resultCard(product(0), track)).toContain("Servicio");
    }
  });

  it("physical rubros are physicalStock:true → card shows stock", () => {
    for (const r of ["farmacia", "minimarket", "cafe", "tienda", "otro"]) {
      const track = featuresForRubro(r).physicalStock;
      expect(track).toBe(true);
      expect(resultCard(product(3), track)).toContain("Stock:");
    }
  });
});

describe("devoluciones — stock copy gated by rubro", () => {
  it("physical rubro mentions restocking aparte", () => {
    expect(devolucionesSubtitle(true)).toContain("stock se reabastece");
    const ctl = restockControlHtml(true);
    expect(ctl).toContain("Reingresar al stock");
    expect(ctl).toContain("dev-f-restock");
  });

  it("service rubro drops every stock reference", () => {
    expect(devolucionesSubtitle(false).toLowerCase()).not.toContain("stock");
    expect(restockControlHtml(false)).toBe("");
  });
});

describe("POS copy — pack vocab + professional errors", () => {
  it("search placeholder uses the pack item word", () => {
    expect(posSearchPlaceholder("Servicio", false)).toMatch(/servicio/i);
    expect(posSearchPlaceholder("Producto", true)).toMatch(/producto|código de barras/i);
  });

  it("service card can show custom item label", () => {
    expect(resultCard(product(0), false, "Servicio")).toContain("Servicio");
  });

  it("friendlyPosError never surfaces stack-like English", () => {
    expect(friendlyPosError("Error: boom\n    at foo.js:1")).toMatch(/no se pudo/i);
    expect(friendlyPosError("timeout waiting")).toMatch(/tiempo/i);
  });
});
