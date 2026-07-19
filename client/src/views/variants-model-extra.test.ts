// Extra pure-model tests: DTO flags, badge priority, money/barcode edge cases.
import { describe, it, expect } from "vitest";
import {
  buildNewVariantInput,
  variantsListBadgeFromDto,
  preferBarcodeLookup,
  parentWithVariantsError,
  isParentHasVariantsMessage,
  defaultVariantName,
  shouldOfferVariantsUi,
  matrixComboSuggestions,
} from "./variants-ui";
import { hasVariantsStockFlag, productFromDetail, type ProductDetail } from "../api/catalog";

const detail = (partial: Partial<ProductDetail> = {}): ProductDetail => ({
  id: "product:1",
  name: "Polera",
  slug: "polera",
  description: null,
  price: "9990",
  cost_price: null,
  stock: 0,
  category: null,
  active: true,
  laboratory: null,
  therapeutic_action: null,
  active_ingredient: null,
  prescription_type: "none",
  presentation: null,
  discount_percent: null,
  ...partial,
});

describe("hasVariantsStockFlag / productFromDetail", () => {
  it("flags parent when either count or stock sum is present", () => {
    expect(hasVariantsStockFlag({ variant_count: 0 })).toBe(true);
    expect(hasVariantsStockFlag({ variants_stock: 0 })).toBe(true);
    expect(hasVariantsStockFlag({})).toBe(false);
  });
  it("maps detail → list projection including multi-SKU fields", () => {
    const p = productFromDetail(
      detail({
        barcode: "7801",
        variants_stock: 8,
        variant_count: 2,
        parent_id: null,
      }),
    );
    expect(p.barcode).toBe("7801");
    expect(p.variants_stock).toBe(8);
    expect(p.variant_count).toBe(2);
  });
});

describe("Chile copy edge cases", () => {
  it("parent error keeps guillemets and barcode ask", () => {
    const m = parentWithVariantsError("Polera «special»");
    expect(m).toMatch(/«/);
    expect(m.toLowerCase()).toMatch(/escanea|código/);
  });
  it("domain reject with escanee accent still matches", () => {
    expect(
      isParentHasVariantsMessage(
        "el producto tiene variantes; venda por talla o escanee el código",
      ),
    ).toBe(true);
  });
  it("defaultVariantName trims parent blanks", () => {
    expect(defaultVariantName("  ", { talla: "M" })).toMatch(/Producto/);
  });
});

describe("barcode + money build edges", () => {
  it("accepts internal SKU with dash", () => {
    const r = buildNewVariantInput({ barcode: "POL-M-001", attrs: { talla: "M" } });
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.value.attrs?.talla).toBe("M");
  });
  it("rejects negative-looking stock digits only path", () => {
    const r = buildNewVariantInput({ barcode: "SKU1", stock: "-3" });
    // digits strip may yield 3 or fail — stock must be ≥ 0
    if (r.ok) expect((r.value.stock ?? 0) >= 0).toBe(true);
  });
  it("preferBarcodeLookup rejects phrasey names", () => {
    expect(preferBarcodeLookup("polera basica m negra")).toBe(false);
    expect(preferBarcodeLookup("7801234567890")).toBe(true);
  });
});

describe("multi-rubro + matrix honesty", () => {
  it("service rubro never offers variants UI", () => {
    expect(shouldOfferVariantsUi(false)).toBe(false);
  });
  it("empty matrix when no dims", () => {
    expect(matrixComboSuggestions([], [])).toEqual([]);
  });
  it("badge dto empty when no signals", () => {
    expect(variantsListBadgeFromDto({ variant_count: null, variants_stock: null })).toBe("");
  });
});
