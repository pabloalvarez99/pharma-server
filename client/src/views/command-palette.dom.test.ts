// @vitest-environment happy-dom
//
// Drives the real command-palette DOM mount to prove the keyboard-first
// contract: Ctrl/Cmd+K opens, typing filters, arrows move the selection (with
// wrap), Enter runs the highlighted command, Escape closes, and the `?`
// cheatsheet + `g`-chord navigation work without hijacking the keyboard while
// the operator is typing in a field.
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  openCommandPalette,
  openCheatsheet,
  installCommandPalette,
  type PaletteDeps,
} from "./command-palette";

function deps(over: Partial<PaletteDeps> = {}): PaletteDeps {
  return {
    navTargets: () => [
      { id: "pos", label: "POS", hint: "Punto de venta" },
      { id: "caja", label: "Caja", hint: "Apertura y arqueo" },
      { id: "inventory", label: "Inventario", hint: "Stock y lotes" },
    ],
    go: vi.fn(),
    focusAgent: vi.fn(),
    openCheatsheet: vi.fn(),
    ...over,
  };
}

function key(opts: KeyboardEventInit & { key: string }, target: EventTarget = document): void {
  target.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...opts }));
}

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("openCommandPalette", () => {
  it("mounts a modal dialog and focuses the search input", () => {
    openCommandPalette(deps());
    const dialog = document.querySelector(".cmdk");
    expect(dialog).not.toBeNull();
    expect(dialog!.getAttribute("aria-modal")).toBe("true");
    expect(document.activeElement).toBe(document.querySelector(".cmdk-input"));
  });

  it("filters the list as the operator types", () => {
    openCommandPalette(deps());
    const input = document.querySelector<HTMLInputElement>(".cmdk-input")!;
    input.value = "inventario";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    const titles = Array.from(document.querySelectorAll(".cmdk-item-title")).map(
      (e) => e.textContent,
    );
    expect(titles).toContain("Ir a Inventario");
    expect(titles).not.toContain("Ir a POS");
  });

  it("shows the empty state when nothing matches", () => {
    openCommandPalette(deps());
    const input = document.querySelector<HTMLInputElement>(".cmdk-input")!;
    input.value = "zzzzzz";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    expect(document.querySelector<HTMLElement>(".cmdk-empty")!.hidden).toBe(false);
    expect(document.querySelectorAll(".cmdk-item").length).toBe(0);
  });

  it("moves the selection down with ArrowDown and wraps at the ends", () => {
    openCommandPalette(deps());
    const sel = () => document.querySelector(".cmdk-item.is-active")?.getAttribute("data-i");
    expect(sel()).toBe("0");
    key({ key: "ArrowDown" });
    expect(sel()).toBe("1");
    key({ key: "ArrowUp" });
    key({ key: "ArrowUp" }); // 0 -> wrap to last
    expect(sel()).toBe(String(document.querySelectorAll(".cmdk-item").length - 1));
  });

  it("runs the highlighted command on Enter and closes", () => {
    const d = deps();
    openCommandPalette(d);
    // Filter to a deterministic top result rather than relying on list order.
    const input = document.querySelector<HTMLInputElement>(".cmdk-input")!;
    input.value = "caja";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    key({ key: "Enter" });

    expect(d.go).toHaveBeenCalledWith("caja");
    expect(document.querySelector(".cmdk")).toBeNull(); // closed
  });

  it("runs a command when its row is clicked", () => {
    const d = deps();
    openCommandPalette(d);
    const input = document.querySelector<HTMLInputElement>(".cmdk-input")!;
    input.value = "inventario"; // narrows to the single 'Ir a Inventario' row
    input.dispatchEvent(new Event("input", { bubbles: true }));
    document.querySelector<HTMLElement>(".cmdk-item")!.click();

    expect(d.go).toHaveBeenCalledWith("inventory");
    expect(document.querySelector(".cmdk")).toBeNull();
  });

  it("closes on Escape without running anything", () => {
    const d = deps();
    openCommandPalette(d);
    key({ key: "Escape" });
    expect(document.querySelector(".cmdk")).toBeNull();
    expect(d.go).not.toHaveBeenCalled();
  });

  it("traps focus: Tab is swallowed and focus stays on the input", () => {
    openCommandPalette(deps());
    const input = document.querySelector<HTMLInputElement>(".cmdk-input")!;
    const ev = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    document.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(input);
  });
});

describe("openCheatsheet", () => {
  it("lists the global shortcuts including Ctrl K and ?", () => {
    openCheatsheet();
    const sheet = document.querySelector(".cmdk-sheet");
    expect(sheet).not.toBeNull();
    const text = sheet!.textContent ?? "";
    expect(text).toContain("paleta de comandos");
    const kbds = Array.from(sheet!.querySelectorAll("kbd")).map((k) => k.textContent);
    expect(kbds).toContain("K");
    expect(kbds).toContain("?");
  });

  it("closes on Escape", () => {
    openCheatsheet();
    key({ key: "Escape" });
    expect(document.querySelector(".cmdk-sheet")).toBeNull();
  });
});

describe("installCommandPalette (global hotkeys)", () => {
  // Build a minimal shell DOM (nav buttons + agent input) the live deps read.
  function shell(): void {
    document.body.innerHTML = `
      <nav id="nav">
        <button class="nav-item" data-nav="dashboard"><span class="nav-label">Panel</span><span class="nav-hint">Resumen</span></button>
        <button class="nav-item" data-nav="pos"><span class="nav-label">POS</span><span class="nav-hint">Punto de venta</span></button>
      </nav>
      <input class="agent-ask-input" />
    `;
  }

  beforeEach(() => {
    shell();
    installCommandPalette(); // idempotent — guarded after first call
    // make sure no overlay leaked from a previous test
    document.querySelector(".cmdk-backdrop")?.remove();
  });

  it("opens the palette on Ctrl+K and toggles it closed on a second press", () => {
    key({ key: "k", ctrlKey: true });
    expect(document.querySelector(".cmdk")).not.toBeNull();
    key({ key: "k", ctrlKey: true });
    expect(document.querySelector(".cmdk")).toBeNull();
  });

  it("opens the cheatsheet on ? when not typing in a field", () => {
    key({ key: "?" });
    expect(document.querySelector(".cmdk-sheet")).not.toBeNull();
    key({ key: "Escape" });
  });

  it("navigates with the g-chord (g then p -> POS)", () => {
    const pos = document.querySelector<HTMLButtonElement>('.nav-item[data-nav="pos"]')!;
    const clicked = vi.fn();
    pos.addEventListener("click", clicked);
    key({ key: "g" });
    key({ key: "p" });
    expect(clicked).toHaveBeenCalledTimes(1);
  });

  it("does NOT hijack ? while the operator is typing in an input", () => {
    const field = document.querySelector<HTMLInputElement>(".agent-ask-input")!;
    field.focus();
    key({ key: "?" }, field);
    expect(document.querySelector(".cmdk-sheet")).toBeNull();
  });
});
