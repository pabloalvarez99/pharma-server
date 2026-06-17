// @vitest-environment happy-dom
//
// Rubro-select showcase — keyboard + a11y of the LIVE configurator grid (P4).
//
// Drives the REAL `renderConfiguracion` rubro grid (not a copy) under a DOM, the
// way a mouseless operator would: arrows rove focus, Enter/Espacio selects, the
// live preview re-renders for the picked rubro. Locks the accessibility contract
// the showcase promises (docs/strategy/rubro-select-experience.md §1.4, DoD §9):
// role=radiogroup/radio + aria-checked + roving tabindex, full keyboard grid nav,
// service-rubro preview honesty, and cero dead-end (Próximamente still selectable).
import { describe, it, expect, vi } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// Mock the Tauri api layer (configuracion.ts → ../api → @tauri-apps invoke):
// getSetting feeds the grid its stored rubro; nothing else is exercised on mount.
// Hardcode the key literal ("business.vertical" = VERTICAL_KEY) to stay clear of
// vi.mock hoisting.
const h = vi.hoisted(() => ({ initial: "farmacia" }));
vi.mock("../src/api", () => ({
  getSetting: async (_url: string, key: string) =>
    key === "business.vertical" ? { value: h.initial, updated_at: null } : null,
  setSetting: async () => {},
  serverHealth: async () => ({ status: "ok", db: "ok", reachable: true }),
  seedDemo: async () => ({}),
  storedServerUrl: () => "http://127.0.0.1:8080",
  rememberServerUrl: () => {},
  SEED_ALREADY_EXISTS: "SEED_ALREADY_EXISTS",
}));

import { renderConfiguracion } from "../src/views/configuracion";

// Catalog grid order (vertical.ts RUBRO_CATALOG): used for index math (4 cols).
const ORDER = [
  "farmacia",
  "minimarket",
  "restaurant",
  "cafe",
  "tienda",
  "belleza",
  "servicios",
  "otro",
];

async function mountGrid(initial = "farmacia"): Promise<HTMLElement> {
  h.initial = initial;
  document.body.innerHTML = "";
  const host = document.createElement("div");
  document.body.appendChild(host);
  renderConfiguracion(host, "http://127.0.0.1:8080");
  await vi.waitFor(() => {
    if (!host.querySelector(".rubro-card")) throw new Error("grid not mounted");
  });
  return host;
}

const cardsOf = (host: HTMLElement) =>
  Array.from(host.querySelectorAll<HTMLButtonElement>(".rubro-card"));
const cardFor = (host: HTMLElement, v: string) =>
  host.querySelector<HTMLButtonElement>(`.rubro-card[data-vertical="${v}"]`)!;
const press = (el: Element, key: string) =>
  el.dispatchEvent(
    new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }),
  );

describe("configurator grid — ARIA radiogroup semantics", () => {
  it("renders a labelled radiogroup of role=radio cards, one per catalog rubro", async () => {
    const host = await mountGrid();
    const grid = host.querySelector(".rubro-grid")!;
    expect(grid.getAttribute("role")).toBe("radiogroup");
    expect(grid.getAttribute("aria-label")).toBeTruthy();
    const cards = cardsOf(host);
    expect(cards.map((c) => c.dataset.vertical)).toEqual(ORDER);
    for (const c of cards) expect(c.getAttribute("role")).toBe("radio");
  });

  it("reflects the stored rubro as the single checked card", async () => {
    const host = await mountGrid("minimarket");
    const checked = cardsOf(host).filter(
      (c) => c.getAttribute("aria-checked") === "true",
    );
    expect(checked).toHaveLength(1);
    expect(checked[0].dataset.vertical).toBe("minimarket");
  });

  it("roving tabindex: exactly the selected card is tab-reachable (0), rest -1", async () => {
    const host = await mountGrid("farmacia");
    const cards = cardsOf(host);
    const reachable = cards.filter((c) => c.tabIndex === 0);
    expect(reachable).toHaveLength(1);
    expect(reachable[0].dataset.vertical).toBe("farmacia");
    expect(cards.filter((c) => c.tabIndex === -1)).toHaveLength(ORDER.length - 1);
  });
});

describe("configurator grid — keyboard navigation (mouseless operator)", () => {
  it("ArrowRight/ArrowLeft rove focus one card, wrapping at the ends", async () => {
    const host = await mountGrid();
    const cards = cardsOf(host);
    cards[0].focus();
    press(cards[0], "ArrowRight");
    expect(document.activeElement).toBe(cards[1]);
    press(cards[1], "ArrowLeft");
    expect(document.activeElement).toBe(cards[0]);
    press(cards[0], "ArrowLeft"); // wrap to last
    expect(document.activeElement).toBe(cards[ORDER.length - 1]);
    press(cards[ORDER.length - 1], "ArrowRight"); // wrap to first
    expect(document.activeElement).toBe(cards[0]);
  });

  it("ArrowDown/ArrowUp jump a row (4 cols), clamped at the grid edges", async () => {
    const host = await mountGrid();
    const cards = cardsOf(host);
    cards[0].focus();
    press(cards[0], "ArrowDown"); // 0 -> 4
    expect(document.activeElement).toBe(cards[4]);
    press(cards[4], "ArrowUp"); // 4 -> 0
    expect(document.activeElement).toBe(cards[0]);
    press(cards[0], "ArrowUp"); // clamp at top
    expect(document.activeElement).toBe(cards[0]);
    cards[ORDER.length - 1].focus();
    press(cards[ORDER.length - 1], "ArrowDown"); // clamp at bottom
    expect(document.activeElement).toBe(cards[ORDER.length - 1]);
  });

  it("Home/End jump to the first/last card", async () => {
    const host = await mountGrid();
    const cards = cardsOf(host);
    cards[3].focus();
    press(cards[3], "End");
    expect(document.activeElement).toBe(cards[ORDER.length - 1]);
    press(cards[ORDER.length - 1], "Home");
    expect(document.activeElement).toBe(cards[0]);
  });

  it("arrows MOVE focus only — they never change the selection", async () => {
    const host = await mountGrid("farmacia");
    const cards = cardsOf(host);
    cards[0].focus();
    press(cards[0], "ArrowRight");
    press(cards[1], "ArrowRight");
    // focus moved to restaurant, but farmacia is still the checked rubro.
    expect(document.activeElement).toBe(cards[2]);
    expect(cardFor(host, "farmacia").getAttribute("aria-checked")).toBe("true");
    expect(cardFor(host, "restaurant").getAttribute("aria-checked")).toBe("false");
  });

  it("Enter and Espacio select the focused card (aria-checked + roving move)", async () => {
    for (const key of ["Enter", " "]) {
      const host = await mountGrid("farmacia");
      const target = cardFor(host, "minimarket");
      target.focus();
      press(target, key);
      expect(target.getAttribute("aria-checked"), `key=${key}`).toBe("true");
      expect(target.tabIndex).toBe(0);
      expect(cardFor(host, "farmacia").getAttribute("aria-checked")).toBe("false");
      const checked = cardsOf(host).filter(
        (c) => c.getAttribute("aria-checked") === "true",
      );
      expect(checked).toHaveLength(1);
    }
  });
});

describe("configurator grid — live preview tracks the picked rubro", () => {
  it("clicking a rubro re-renders the preview panel for THAT rubro", async () => {
    const host = await mountGrid("farmacia");
    const preview = host.querySelector<HTMLElement>("#cfg-vert-preview")!;
    expect(preview.getAttribute("aria-live")).toBe("polite");
    expect(preview.textContent).toContain("Tu farmacia, en regla."); // initial tagline
    cardFor(host, "minimarket").click();
    expect(preview.textContent).toContain("Tu almacén, al día.");
    expect(preview.textContent).toContain("Minimarket");
  });

  it("a service rubro previews honestly: sells without stock, hides inventario", async () => {
    const host = await mountGrid("farmacia");
    const preview = host.querySelector<HTMLElement>("#cfg-vert-preview")!;
    cardFor(host, "belleza").click();
    expect(preview.textContent).toContain("Venta de servicios sin inventario");
    // "Inventario y compras" category renders off for a service rubro.
    const off = Array.from(preview.querySelectorAll(".rubro-cat.off")).map(
      (li) => li.textContent ?? "",
    );
    expect(off.some((t) => t.includes("Inventario"))).toBe(true);
  });

  it("SII compliance note is shown for every rubro (universal boleta/factura)", async () => {
    const host = await mountGrid("belleza");
    const preview = host.querySelector<HTMLElement>("#cfg-vert-preview")!;
    expect(preview.textContent).toContain("Boleta y factura electrónica");
  });
});

describe("configurator grid — cero dead-end", () => {
  it("a Próximamente rubro is a real selectable radio, just without a demo pack", async () => {
    const host = await mountGrid("farmacia");
    const restaurant = cardFor(host, "restaurant");
    expect(restaurant.disabled).toBe(false);
    expect(restaurant.getAttribute("role")).toBe("radio");
    restaurant.focus();
    press(restaurant, "Enter");
    expect(restaurant.getAttribute("aria-checked")).toBe("true");
    // its card shows the muted "pronto" tag; with no demo pack the demo button
    // is hidden (no dead greyed control — ULTRA-PLAN §8 "no dead-end"), the
    // operator is pointed at import/manual instead — the rubro is still fully
    // chosen, a usable empty ERP.
    expect(restaurant.querySelector(".rubro-tag.soon")).not.toBeNull();
    const demoBtn = host.querySelector<HTMLButtonElement>("#cfg-demo-btn")!;
    expect(demoBtn.hidden).toBe(true);
  });

  it("the demo button is enabled for a rubro that HAS a pack (farmacia)", async () => {
    const host = await mountGrid("farmacia");
    const demoBtn = host.querySelector<HTMLButtonElement>("#cfg-demo-btn")!;
    expect(demoBtn.disabled).toBe(false);
  });
});

describe("showcase respects prefers-reduced-motion (offline, CSS-only)", () => {
  it("brand.css neutralizes rubro motion under prefers-reduced-motion", () => {
    // cwd is the client/ dir under vitest; happy-dom's import.meta.url isn't a file URL.
    const css = readFileSync(resolve(process.cwd(), "src/brand.css"), "utf8");
    expect(css).toMatch(/@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  });
});
