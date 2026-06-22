// @vitest-environment happy-dom
//
// Centro de Configuración — HUB e2e (W6, follow-up of ye's Pilar A lane).
//
// The W5 scaffold's `it.todo`s are now REAL DOM assertions: ye's config hub
// (`renderConfiguracion`) has landed in feature/erp-parity, so this drives the
// actual renderer under a DOM — nav, human labels, in-settings search, the
// save-state machine, inline field validation, the rubro vitrina mount, and the
// produced empty/loading/error states. The rubro grid's deep keyboard/a11y
// contract lives in `rubro-configurator.dom.test.ts`; here we only assert it
// MOUNTS inside the hub (no duplication).
//
// Produced-state note (professional-completeness §Pilar A/C): the contract is
// "every list/panel shows a produced empty/loading/error state, never a bare
// `<p>text</p>` that reads 'de dev'." bob's own views (reports/boletas/…) render
// these through the ui.ts `.ui-*` helpers; ye's config renderer satisfies the
// SAME contract with the shared inventory `tableSkeleton` (loading) + `.view-error`
// (error). Both are produced — we assert the contract, not one specific class set.
//
// Sections NOT yet wired in this base (usuarios/sucursales CRUD, in-UI cert/CAF
// upload, scheduled backup+restore) render an HONEST "próximamente" mount with a
// CLI path — never a fake or dead-end (ADR-0005, no dark patterns). Those are
// asserted as honest placeholders here and kept as `it.todo` for the wave ye's
// `feat/config-users-api` / `feat/config-branches-sii` lanes land the real CRUD.
import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the Tauri api layer (configuracion.ts → ../api → @tauri-apps invoke). The
// hoisted store lets a test steer the stored rubro and force a section load to
// fail (error-state coverage), and records writes so we can assert a save was —
// or was NOT (blocked by validation) — persisted. Setting-key literals are
// hardcoded (VERTICAL_KEY = "business.vertical") to stay clear of vi.mock hoisting.
const h = vi.hoisted(() => ({
  vertical: "farmacia",
  failVertical: false,
  setCalls: [] as Array<{ key: string; value: string }>,
}));
vi.mock("../src/api", () => ({
  getSetting: async (_url: string, key: string) => {
    // The real api layer (Tauri invoke) rejects with a string message; asMessage
    // passes strings through verbatim, so this mirrors a backend error surfacing.
    if (h.failVertical && key === "business.vertical") {
      throw "Servidor no disponible";
    }
    if (key === "business.vertical") return { value: h.vertical, updated_at: null };
    return null; // every other setting absent → forms render their defaults
  },
  setSetting: async (_url: string, key: string, value: string) => {
    h.setCalls.push({ key, value });
  },
  serverHealth: async () => ({ status: "ok", db: "ok", reachable: true }),
  seedDemo: async () => ({ products_created: 0, batches_created: 0, movements_emitted: 0, wiped: 0 }),
  storedServerUrl: () => "http://127.0.0.1:8080",
  rememberServerUrl: () => {},
  licenseStatus: async () => ({
    tier: "free",
    status: "active",
    license_id: "lic_test",
    features: [],
    expires_at: null,
    key_id: "k1",
    seat_count: 1,
  }),
  dteCafStatus: async () => ({ cafs: [] }),
  SEED_ALREADY_EXISTS: "SEED_ALREADY_EXISTS",
}));

import { renderConfiguracion } from "../src/views/configuracion";
import { CONFIG_SECTIONS } from "../src/views/config-center";

const SERVER = "http://127.0.0.1:8080";

function mountHub(): HTMLElement {
  document.body.innerHTML = "";
  const host = document.createElement("div");
  document.body.appendChild(host);
  renderConfiguracion(host, SERVER);
  return host;
}

/** Wait until a selector resolves (a section's async load painted its content). */
async function waitForEl<T extends Element>(host: HTMLElement, sel: string): Promise<T> {
  return vi.waitFor(() => {
    const el = host.querySelector<T>(sel);
    if (!el) throw new Error(`not mounted yet: ${sel}`);
    return el;
  });
}

const navItems = (host: HTMLElement) =>
  Array.from(host.querySelectorAll<HTMLButtonElement>(".cfg-nav-item"));
const visibleSections = (host: HTMLElement) =>
  navItems(host).filter((b) => !b.hidden).map((b) => b.dataset.section);
const goSection = (host: HTMLElement, id: string) =>
  host.querySelector<HTMLButtonElement>(`.cfg-nav-item[data-section="${id}"]`)!.click();
const type = (input: HTMLInputElement, value: string) => {
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
};

beforeEach(() => {
  h.vertical = "farmacia";
  h.failVertical = false;
  h.setCalls = [];
  document.body.innerHTML = "";
});

describe("config hub — lateral nav", () => {
  it("renders one nav item per catalog section, in catalog order", () => {
    const host = mountHub();
    const nav = host.querySelector<HTMLElement>("#cfg-nav")!;
    expect(nav.getAttribute("aria-label")).toBeTruthy();
    expect(navItems(host).map((b) => b.dataset.section)).toEqual(
      CONFIG_SECTIONS.map((s) => s.id),
    );
    // the flagship sections AND the honest "pronto" mounts are all listed — the
    // operator sees the whole roadmap, never a hidden/dead area.
    for (const id of [
      "negocio", "sii", "licencia", "agente", "preferencias",
      "usuarios", "sucursales", "respaldo",
    ]) {
      expect(navItems(host).some((b) => b.dataset.section === id)).toBe(true);
    }
  });

  it("nav labels are human Spanish, never a raw admin_setting key (cero 'de dev')", () => {
    const host = mountHub();
    const labels = Array.from(host.querySelectorAll(".cfg-nav-label")).map(
      (e) => (e.textContent ?? "").trim(),
    );
    expect(labels.length).toBe(CONFIG_SECTIONS.length);
    for (const t of labels) {
      expect(t.length).toBeGreaterThan(0);
      expect(t).not.toMatch(/[._]|admin_setting|cfg-/);
    }
  });
});

describe("config hub — in-settings search", () => {
  it("filters the nav down to matching sections", () => {
    const host = mountHub();
    const search = host.querySelector<HTMLInputElement>("#cfg-search")!;

    // blank query → the whole catalog is visible (no filter chrome)
    expect(visibleSections(host).length).toBe(CONFIG_SECTIONS.length);

    // a section-label keyword folds the nav to Facturación SII
    type(search, "facturacion");
    expect(visibleSections(host)).toContain("sii");
    expect(visibleSections(host)).not.toContain("negocio");
    expect(visibleSections(host)).toHaveLength(1);
  });

  it("surfaces a jump-to-field chip when a field (not the section) matches", () => {
    const host = mountHub();
    const search = host.querySelector<HTMLInputElement>("#cfg-search")!;
    const hits = host.querySelector<HTMLElement>("#cfg-search-hits")!;

    type(search, "rut"); // "RUT empresa" lives inside Negocio
    expect(visibleSections(host)).toContain("negocio");
    expect(hits.hidden).toBe(false);
    expect(
      Array.from(hits.querySelectorAll(".cfg-hit-chip")).some((c) =>
        /rut/i.test(c.textContent ?? ""),
      ),
    ).toBe(true);
  });

  it("a query that matches nothing shows an explicit 'sin resultados' (never a blank dead panel)", () => {
    const host = mountHub();
    const search = host.querySelector<HTMLInputElement>("#cfg-search")!;
    const hits = host.querySelector<HTMLElement>("#cfg-search-hits")!;

    type(search, "zzzznotarealsetting");
    expect(visibleSections(host)).toEqual([]);
    expect(hits.hidden).toBe(false);
    expect(hits.textContent ?? "").toMatch(/sin resultados/i);
  });
});

describe("config hub — save-state machine (produced feedback)", () => {
  it("saves a setting and surfaces a produced success state, then persists it", async () => {
    const host = mountHub();
    goSection(host, "sii");
    const status = await waitForEl<HTMLElement>(host, "#cfg-sii-status");
    const saveBtn = await waitForEl<HTMLButtonElement>(host, "#cfg-sii-save");

    expect(status.hidden).toBe(true); // idle: no feedback yet
    saveBtn.click();

    await vi.waitFor(() => {
      expect(status.hidden).toBe(false);
      expect(status.className).toContain("cfg-status-ok");
    });
    expect(status.textContent ?? "").toMatch(/guardado/i);
    // the write actually went through (sandbox is the default env)
    expect(h.setCalls.some((c) => c.value === "sandbox")).toBe(true);
  });
});

describe("config hub — inline validation blocks a bad write", () => {
  it("echoes a readable mód-11 error inline and refuses to persist an invalid RUT", async () => {
    const host = mountHub(); // Negocio is the landing section → emisor form mounts
    const rut = await waitForEl<HTMLInputElement>(host, "#cfg-em-rut");
    const hint = host.querySelector<HTMLElement>("#cfg-em-rut-hint")!;

    // live mód-11 echo: a wrong check digit flags the field with a readable message
    type(rut, "76123456-5"); // real DV is 0 → 5 is wrong
    expect(hint.hidden).toBe(false);
    expect(hint.className).toContain("err");
    expect(hint.textContent ?? "").toMatch(/d[ií]gito verificador/i);
    expect(rut.classList.contains("invalid")).toBe(true);

    // fill the other required fields so the save's only blocker is the RUT
    for (const [name, val] of [
      ["razon_social", "Mi Empresa SpA"],
      ["giro", "Comercio"],
      ["direccion", "Av. Principal 123"],
      ["comuna", "Coquimbo"],
    ] as const) {
      const el = host.querySelector<HTMLInputElement>(`#cfg-em-${name}`)!;
      el.value = val;
    }

    const before = h.setCalls.length;
    host.querySelector<HTMLButtonElement>("#cfg-em-save")!.click();
    const status = host.querySelector<HTMLElement>("#cfg-em-status")!;
    expect(status.hidden).toBe(false);
    expect(status.className).toContain("cfg-status-err");
    expect(status.textContent ?? "").toMatch(/d[ií]gito verificador/i);
    expect(h.setCalls.length).toBe(before); // write was BLOCKED

    // a valid RUT clears the inline error (the gate opens)
    type(rut, "11.111.111-1");
    expect(hint.className).toContain("ok");
    expect(rut.classList.contains("invalid")).toBe(false);
  });
});

describe("config hub — rubro vitrina mounts (deep a11y in rubro-configurator.dom.test)", () => {
  it("mounts the rubro radiogroup + live preview inside the Negocio section", async () => {
    const host = mountHub();
    const grid = await waitForEl<HTMLElement>(host, ".rubro-grid");
    expect(grid.getAttribute("role")).toBe("radiogroup");
    expect(host.querySelector("#cfg-vert-preview")).not.toBeNull();
    expect(host.querySelectorAll(".rubro-card").length).toBeGreaterThan(0);
  });
});

describe("config hub — produced empty/loading/error states", () => {
  it("shows a produced LOADING skeleton (never bare 'Cargando…' text) before data resolves", () => {
    const host = mountHub();
    const vert = host.querySelector<HTMLElement>("#cfg-vertical")!;
    // synchronous: the loader paints a skeleton, then awaits — assert before resolve
    expect(vert.querySelector(".table-skel")).not.toBeNull();
    expect(vert.querySelector(".sk-row")).not.toBeNull();
    expect((vert.textContent ?? "").trim()).toBe(""); // skeleton carries no dev text
  });

  it("renders a produced ERROR state (humanized, never a crash) when a section load fails", async () => {
    h.failVertical = true;
    const host = mountHub();
    const err = await waitForEl<HTMLElement>(host, "#cfg-vertical .view-error");
    const msg = (err.textContent ?? "").trim();
    expect(msg).toContain("Servidor no disponible"); // humanized backend message
    expect(msg).not.toMatch(/\[object|undefined|\bError:/); // never a raw dump/stack
    expect(msg.length).toBeGreaterThan(0);
  });
});

describe("config hub — honest 'próximamente' mounts (cero dead-end, ADR-0005)", () => {
  it("placeholder sections render an honest roadmap + CLI path, never blank or fake", () => {
    const host = mountHub();
    const panel = host.querySelector<HTMLElement>("#cfg-panel")!;
    for (const id of ["usuarios", "sucursales", "respaldo"]) {
      goSection(host, id);
      expect(panel.querySelector(".cfg-soon-chip")?.textContent ?? "").toMatch(/próximamente/i);
      expect((panel.textContent ?? "").trim().length).toBeGreaterThan(0);
    }
    // every placeholder tab also flags itself "pronto" in the nav
    expect(host.querySelectorAll(".cfg-nav-soon").length).toBe(
      CONFIG_SECTIONS.filter((s) => s.placeholder).length,
    );
  });

  it("SII surfaces certificate/CAF upload as an honest 'próximamente' card, not a fake uploader", () => {
    const host = mountHub();
    goSection(host, "sii");
    const soon = host.querySelector<HTMLElement>("#cfg-panel .cfg-card-soon");
    expect(soon).not.toBeNull();
    expect(soon!.textContent ?? "").toMatch(/certificado|caf/i);
    expect(soon!.querySelector(".cfg-soon-chip")?.textContent ?? "").toMatch(/próximamente/i);
  });

  // --- becomes real when ye's CRUD lanes land in feature/erp-parity -----------
  // (feat/config-users-api, feat/config-branches-sii). Kept as todos — never
  // asserting against the soon-to-be-replaced placeholder, never going red.
  it.todo("Usuarios: create/edit/delete a cashier with a role, surfaced in a produced table");
  it.todo("Sucursales y cajas: add a branch + caja, list them, switch the active one");
  it.todo("SII: upload a .pfx certificate and CAF files from the hub with progress + validation");
  it.todo("Respaldo: run a backup now, schedule one, and restore via a guided step flow");
});
