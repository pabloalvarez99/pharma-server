import { describe, it, expect } from "vitest";
import {
  variantsParentBanner,
  variantsSectionTitle,
  parentWithVariantsError,
  isParentHasVariantsMessage,
  variantAttrsLabel,
  preferBarcodeLookup,
  parentStockLabel,
  sumVariantStock,
  variantsListBadge,
  variantsStockListBadge,
  variantsListBadgeFromDto,
  variantStockCellLabel,
  variantRowAriaLabel,
  variantsLoadingLabel,
  variantsLoadError,
  matrixComboSuggestions,
  variantEditBlockedHint,
  variantFormKeyboardHint,
  variantChildNote,
  addVariantButtonLabel,
  addVariantModalTitle,
  hasVariantsToggleHint,
  hasVariantsToggleLabel,
  parentCreatedOpenVariantsToast,
  variantsEmptyHint,
  plainOutOfStockError,
  shouldOfferVariantsUi,
  variantAttrFieldsFromPack,
  isVariantAttrKey,
  defaultVariantAttrFields,
  variantFormAttrFields,
  defaultVariantName,
  toVariantTableRow,
  posVariantsSearchHint,
  buildNewVariantInput,
  parentStockWhenHasVariants,
} from "./variants-ui";
import type { PackAttrField } from "../api/rubro";

const TIENDA: PackAttrField[] = [
  { key: "talla", label: "Talla", kind: "text" },
  { key: "color", label: "Color", kind: "text" },
  { key: "sku", label: "SKU", kind: "text" },
];

const FARMA: PackAttrField[] = [
  { key: "active_ingredient", label: "Principio activo", kind: "text" },
  { key: "laboratory", label: "Laboratorio", kind: "text" },
];

describe("variantsParentBanner", () => {
  it("Spanish copy with count for multi-SKU parent", () => {
    expect(variantsParentBanner(2)).toMatch(/2 variantes/i);
    expect(variantsParentBanner(2)).toMatch(/código de barras/i);
    expect(variantsParentBanner(2)).toMatch(/padre/i);
    expect(variantsParentBanner(1)).toMatch(/1 variante/i);
  });
  it("empty when no children", () => {
    expect(variantsParentBanner(0)).toBe("");
    expect(variantsParentBanner(-3)).toBe("");
  });
});

describe("variantsSectionTitle", () => {
  it("includes count when > 0", () => {
    expect(variantsSectionTitle(3)).toMatch(/3 variantes/i);
    expect(variantsSectionTitle(1)).toMatch(/1 variante/i);
  });
  it("generic when empty", () => {
    expect(variantsSectionTitle(0)).toMatch(/variantes/i);
  });
});

describe("parentWithVariantsError", () => {
  it("names the product and asks for barcode scan", () => {
    const m = parentWithVariantsError("Polera básica");
    expect(m).toContain("Polera básica");
    expect(m.toLowerCase()).toMatch(/variantes/);
    expect(m.toLowerCase()).toMatch(/escanea|código/);
  });
  it("falls back when name blank", () => {
    expect(parentWithVariantsError("  ")).toMatch(/este producto/i);
  });
});

describe("isParentHasVariantsMessage", () => {
  it("matches domain sales Spanish reject", () => {
    expect(
      isParentHasVariantsMessage(
        "el producto 'Polera' tiene variantes; venda por talla/SKU o escanee el código de barras de la variante",
      ),
    ).toBe(true);
    expect(isParentHasVariantsMessage("stock insuficiente")).toBe(false);
  });
});

describe("variantAttrsLabel", () => {
  it("prefers talla/color/sku order", () => {
    expect(variantAttrsLabel({ talla: "M", color: "Negro", sku: "X1" })).toBe("M · Negro · X1");
  });
  it("empty bag → empty string", () => {
    expect(variantAttrsLabel(null)).toBe("");
    expect(variantAttrsLabel({})).toBe("");
  });
  it("falls back to other keys", () => {
    expect(variantAttrsLabel({ material: "algodón" })).toMatch(/algodón/);
  });
});

describe("preferBarcodeLookup", () => {
  it("EAN-13 style codes prefer barcode path", () => {
    expect(preferBarcodeLookup("7804999700013")).toBe(true);
    expect(preferBarcodeLookup("7804999700020")).toBe(true);
  });
  it("short names do not", () => {
    expect(preferBarcodeLookup("para")).toBe(false);
    expect(preferBarcodeLookup("ab")).toBe(false);
  });
  it("internal SKU without spaces can prefer barcode", () => {
    expect(preferBarcodeLookup("POL-M-001")).toBe(true);
  });
  it("spaces look like name search", () => {
    expect(preferBarcodeLookup("polera basica")).toBe(false);
  });
});

describe("stock helpers", () => {
  it("sumVariantStock adds children", () => {
    expect(sumVariantStock([{ stock: 2 }, { stock: 5 }, { stock: 0 }])).toBe(7);
    expect(sumVariantStock([])).toBe(0);
  });
  it("parentStockLabel shows sum", () => {
    expect(parentStockLabel([{ stock: 3 }, { stock: 1 }])).toMatch(/4 u/);
    expect(parentStockLabel([])).toMatch(/variantes/i);
  });
  it("variantsListBadge", () => {
    expect(variantsListBadge(0)).toBe("");
    expect(variantsListBadge(1)).toBe("1 variante");
    expect(variantsListBadge(4)).toBe("4 variantes");
  });
  it("variantsStockListBadge uses units not child count", () => {
    expect(variantsStockListBadge(12)).toMatch(/Multi-SKU/);
    expect(variantsStockListBadge(12)).toMatch(/12 u/);
  });
  it("variantsListBadgeFromDto prefers count over stock sum", () => {
    expect(variantsListBadgeFromDto({ variant_count: 3, variants_stock: 99 })).toBe("3 variantes");
    expect(variantsListBadgeFromDto({ variants_stock: 5 })).toMatch(/5 u/);
    expect(variantsListBadgeFromDto({})).toBe("");
  });
  it("variantStockCellLabel Agotado when 0", () => {
    expect(variantStockCellLabel(0)).toEqual({ text: "Agotado", out: true });
    expect(variantStockCellLabel(4)).toEqual({ text: "4", out: false });
  });
  it("variantRowAriaLabel includes name and stock state", () => {
    expect(variantRowAriaLabel({ name: "Polera M", stock: 0, barcode: "780" })).toMatch(/agotado/i);
    expect(variantRowAriaLabel({ name: "Polera M", stock: 2, attrsLabel: "M" })).toMatch(/stock 2/i);
  });
  it("loading/error/edit/keyboard hints in Spanish Chile", () => {
    expect(variantsLoadingLabel()).toMatch(/cargando/i);
    expect(variantsLoadError()).toMatch(/variantes/i);
    expect(variantsLoadError("timeout")).toMatch(/timeout/i);
    expect(variantEditBlockedHint()).toMatch(/editar|desactivar/i);
    expect(variantFormKeyboardHint()).toMatch(/enter|esc/i);
  });
  it("matrixComboSuggestions cartesian + missing flag", () => {
    const combos = matrixComboSuggestions(["S", "M"], ["Negro", "Blanco"], [{ talla: "S", color: "Negro" }]);
    expect(combos).toHaveLength(4);
    expect(combos.find((c) => c.label === "S · Negro")?.missing).toBe(false);
    expect(combos.find((c) => c.label === "M · Blanco")?.missing).toBe(true);
  });
  it("matrixComboSuggestions caps and dedupes", () => {
    const manyT = Array.from({ length: 20 }, (_, i) => `T${i}`);
    const manyC = Array.from({ length: 20 }, (_, i) => `C${i}`);
    expect(matrixComboSuggestions(manyT, manyC).length).toBeLessThanOrEqual(24);
    expect(matrixComboSuggestions(["M", "m", ""], ["Rojo"]).map((c) => c.label)).toEqual(["M · Rojo"]);
  });
});

describe("operator copy", () => {
  it("child note / CTA / modal / toggle / toast in Spanish", () => {
    expect(variantChildNote()).toMatch(/código de barras/i);
    expect(addVariantButtonLabel()).toMatch(/agregar variante/i);
    expect(addVariantModalTitle("Polera")).toContain("Polera");
    expect(hasVariantsToggleLabel()).toMatch(/variantes/i);
    expect(hasVariantsToggleHint()).toMatch(/código de barras|talla/i);
    expect(parentCreatedOpenVariantsToast("Polera")).toMatch(/Polera/);
    expect(variantsEmptyHint()).toMatch(/variantes/i);
    expect(plainOutOfStockError("Aspirina")).toMatch(/sin stock/i);
  });
});

describe("multi-rubro honesty", () => {
  it("only physicalStock offers variants UI", () => {
    expect(shouldOfferVariantsUi(true)).toBe(true);
    expect(shouldOfferVariantsUi(false)).toBe(false);
  });
  it("variantAttrFieldsFromPack drops clinical keys", () => {
    const mixed = [...TIENDA, ...FARMA];
    const keys = variantAttrFieldsFromPack(mixed).map((f) => f.key);
    expect(keys).toEqual(["talla", "color", "sku"]);
    expect(keys.every((k) => !isVariantAttrKey("laboratory") || k !== "laboratory")).toBe(true);
    expect(isVariantAttrKey("laboratory")).toBe(false);
    expect(isVariantAttrKey("talla")).toBe(true);
  });
  it("variantFormAttrFields falls back to defaults", () => {
    expect(variantFormAttrFields([])).toEqual(defaultVariantAttrFields());
    expect(variantFormAttrFields(null).map((f) => f.key)).toEqual(["talla", "color", "sku"]);
    expect(variantFormAttrFields(TIENDA).map((f) => f.key)).toEqual(["talla", "color", "sku"]);
  });
  it("farmacia pack alone → defaults for modal (no clinical as variant dims)", () => {
    expect(variantFormAttrFields(FARMA).map((f) => f.key)).toEqual(["talla", "color", "sku"]);
  });
});

describe("defaultVariantName / table row", () => {
  it("joins parent + attrs", () => {
    expect(defaultVariantName("Polera", { talla: "M", color: "Negro" })).toBe(
      "Polera — M · Negro",
    );
    expect(defaultVariantName("Polera", {})).toBe("Polera");
  });
  it("toVariantTableRow maps fields", () => {
    const row = toVariantTableRow({
      id: "p:1",
      name: "Polera — M",
      price: "9990",
      stock: 4,
      attrs: { talla: "M", barcode: "780111" },
    });
    expect(row.attrsLabel).toBe("M");
    expect(row.barcode).toBe("780111");
    expect(row.stock).toBe(4);
    expect(row.active).toBe(true);
  });
});

describe("posVariantsSearchHint", () => {
  it("mentions barcode", () => {
    expect(posVariantsSearchHint("Producto")).toMatch(/código de barras/i);
    expect(posVariantsSearchHint("Servicio")).toMatch(/servicio/i);
  });
});

describe("buildNewVariantInput (barcode-first)", () => {
  it("rejects empty barcode", () => {
    const r = buildNewVariantInput({ barcode: "" });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/código de barras/i);
  });
  it("rejects spaces in barcode", () => {
    const r = buildNewVariantInput({ barcode: "780 123" });
    expect(r.ok).toBe(false);
  });
  it("rejects short barcode", () => {
    const r = buildNewVariantInput({ barcode: "ab" });
    expect(r.ok).toBe(false);
  });
  it("accepts barcode + attrs + stock", () => {
    const r = buildNewVariantInput({
      barcode: "7804999700013",
      stock: "5",
      price: "1.990",
      attrs: { talla: "M", color: "Negro", empty: "" },
    });
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.value.barcode).toBe("7804999700013");
      expect(r.value.stock).toBe(5);
      expect(r.value.price).toBe("1990");
      expect(r.value.attrs).toEqual({ talla: "M", color: "Negro" });
    }
  });
  it("optional name omitted when blank", () => {
    const r = buildNewVariantInput({ barcode: "SKU-001", name: "  " });
    expect(r.ok).toBe(true);
    if (r.ok) expect(r.value.name).toBeUndefined();
  });
  it("invalid money → Spanish error", () => {
    const r = buildNewVariantInput({ barcode: "SKU-001", price: "abc" });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(/precio/i);
  });
});

describe("parentStockWhenHasVariants", () => {
  it("forces 0 when hasVariants", () => {
    expect(parentStockWhenHasVariants(true, true, "12")).toBe(0);
  });
  it("parses stock when plain product", () => {
    expect(parentStockWhenHasVariants(false, true, "12")).toBe(12);
    expect(parentStockWhenHasVariants(false, true, "")).toBeUndefined();
  });
  it("service rubro never sends stock", () => {
    expect(parentStockWhenHasVariants(true, false, "9")).toBeUndefined();
  });
});
