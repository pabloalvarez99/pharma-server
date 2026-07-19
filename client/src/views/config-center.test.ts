import { describe, it, expect } from "vitest";
import {
  CONFIG_SECTIONS,
  defaultSection,
  resolveSection,
  norm,
  searchConfig,
  SAVE_IDLE,
  toSaving,
  toSaved,
  toFailed,
  saveStatusClass,
  validateRequiredText,
  validateBusinessRut,
  validateNonNegativeInt,
  validateActeco,
} from "./config-center";

describe("section catalog", () => {
  it("ids are unique (nav has no duplicate tabs)", () => {
    const ids = CONFIG_SECTIONS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("every section carries human label + blurb + at least one field", () => {
    for (const s of CONFIG_SECTIONS) {
      expect(s.label.length).toBeGreaterThan(0);
      expect(s.blurb.length).toBeGreaterThan(0);
      expect(s.fields.length).toBeGreaterThan(0);
    }
  });

  it("no field label is a raw admin_setting key (no dotted/underscored tech ids)", () => {
    for (const s of CONFIG_SECTIONS) {
      for (const f of s.fields) {
        expect(f.label).not.toMatch(/[._]|admin_setting|cfg-/);
      }
    }
  });

  it("the flagship sections are present", () => {
    const ids = CONFIG_SECTIONS.map((s) => s.id);
    for (const id of ["negocio", "sii", "licencia", "agente", "preferencias"]) {
      expect(ids).toContain(id);
    }
  });
});

describe("section navigation", () => {
  it("default section is the first in the catalog (Negocio)", () => {
    expect(defaultSection()).toBe("negocio");
    expect(CONFIG_SECTIONS[0].id).toBe("negocio");
  });

  it("resolves a known id to itself", () => {
    expect(resolveSection("licencia")).toBe("licencia");
  });

  it("falls back to default for unknown / null / stale ids", () => {
    expect(resolveSection("does-not-exist")).toBe("negocio");
    expect(resolveSection(null)).toBe("negocio");
    expect(resolveSection(undefined)).toBe("negocio");
  });

  it("falls back to the first AVAILABLE section when the catalog is filtered", () => {
    const filtered = CONFIG_SECTIONS.filter((s) => s.id !== "negocio");
    // "negocio" was filtered away → resolve to the new first (sii).
    expect(resolveSection("negocio", filtered)).toBe("sii");
  });
});

describe("norm (accent/case folding)", () => {
  it("strips diacritics and lowercases", () => {
    expect(norm("Facturación")).toBe("facturacion");
    expect(norm("  Telemetría  ")).toBe("telemetria");
    expect(norm("RUT")).toBe("rut");
  });
});

describe("searchConfig", () => {
  it("blank query returns the full catalog, unfiltered, no hits", () => {
    const r = searchConfig("   ");
    expect(r.filtered).toBe(false);
    expect(r.sectionIds.length).toBe(CONFIG_SECTIONS.length);
    expect(r.fieldHits).toHaveLength(0);
  });

  it("matches a section by an accent-insensitive field label", () => {
    const r = searchConfig("facturacion");
    expect(r.filtered).toBe(true);
    expect(r.sectionIds).toContain("sii");
  });

  it("matches by field keyword/synonym, not just the visible label", () => {
    // "backup" is a keyword of the Respaldo section's field, not its label.
    const r = searchConfig("backup");
    expect(r.sectionIds).toContain("respaldo");
    expect(r.fieldHits.some((h) => h.section === "respaldo")).toBe(true);
  });

  it("finds the RUT field inside Negocio and records the field hit", () => {
    const r = searchConfig("rut");
    expect(r.sectionIds).toContain("negocio");
    expect(r.fieldHits.some((h) => h.section === "negocio")).toBe(true);
  });

  it("returns no sections for a query that matches nothing", () => {
    const r = searchConfig("zzzznotarealsetting");
    expect(r.sectionIds).toHaveLength(0);
    expect(r.fieldHits).toHaveLength(0);
    expect(r.filtered).toBe(true);
  });

  it("keeps results in catalog order", () => {
    // "u" appears across many sections; order must match the catalog.
    const r = searchConfig("u");
    const order = CONFIG_SECTIONS.map((s) => s.id).filter((id) => r.sectionIds.includes(id));
    expect(r.sectionIds).toEqual(order);
  });
});

describe("save-state machine", () => {
  it("idle is inert", () => {
    expect(SAVE_IDLE.status).toBe("idle");
    expect(SAVE_IDLE.message).toBeNull();
    expect(saveStatusClass(SAVE_IDLE)).toBe("cfg-status");
  });

  it("transitions carry the right status + css class", () => {
    expect(toSaving().status).toBe("saving");
    expect(saveStatusClass(toSaving())).toBe("cfg-status cfg-status-pending");
    expect(saveStatusClass(toSaved())).toBe("cfg-status cfg-status-ok");
    expect(saveStatusClass(toFailed("boom"))).toBe("cfg-status cfg-status-err");
  });

  it("saved defaults to 'Guardado' but takes a custom message", () => {
    expect(toSaved().message).toBe("Guardado");
    expect(toSaved("Rubro guardado").message).toBe("Rubro guardado");
  });

  it("failed surfaces the server's message verbatim", () => {
    expect(toFailed("No autorizado").message).toBe("No autorizado");
  });
});

describe("field validators", () => {
  it("required text rejects blank, accepts trimmed", () => {
    expect(validateRequiredText("   ", "Giro").ok).toBe(false);
    const ok = validateRequiredText("  Farmacia  ", "Giro");
    expect(ok.ok).toBe(true);
    expect(ok.value).toBe("Farmacia");
  });

  it("business RUT validates mód-11 and canonicalises", () => {
    const bad = validateBusinessRut("76123456-5"); // real DV is 0 → 5 is wrong
    expect(bad.ok).toBe(false);
    expect(bad.message).toMatch(/dígito verificador/);
    const blank = validateBusinessRut("  ");
    expect(blank.ok).toBe(false);
    // A valid RUT round-trips to canonical NNNNNNNN-D form.
    const good = validateBusinessRut("11.111.111-1");
    expect(good.ok).toBe(true);
    expect(good.value).toBe("11111111-1");
  });

  it("non-negative int: blank → 0, truncates, rejects negatives/garbage", () => {
    expect(validateNonNegativeInt("", "Puntos").value).toBe(0);
    expect(validateNonNegativeInt("3.9", "Puntos").value).toBe(3);
    expect(validateNonNegativeInt("-1", "Puntos").ok).toBe(false);
    expect(validateNonNegativeInt("abc", "Puntos").ok).toBe(false);
  });

  it("acteco: optional (blank ok → undefined), integer-only when present", () => {
    const blank = validateActeco("");
    expect(blank.ok).toBe(true);
    expect(blank.value).toBeUndefined();
    expect(validateActeco("477301").value).toBe(477301);
    expect(validateActeco("12.5").ok).toBe(false);
    expect(validateActeco("-3").ok).toBe(false);
  });
});
