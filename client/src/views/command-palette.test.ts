import { describe, it, expect, vi } from "vitest";
import { buildCommands, renderResults, type PaletteDeps } from "./command-palette";
import type { Command } from "./keymap";

function fakeDeps(over: Partial<PaletteDeps> = {}): PaletteDeps {
  return {
    navTargets: () => [
      { id: "pos", label: "POS", hint: "Punto de venta" },
      { id: "caja", label: "Caja", hint: "Apertura y arqueo" },
    ],
    go: vi.fn(),
    focusAgent: vi.fn(),
    openCheatsheet: vi.fn(),
    ...over,
  };
}

describe("buildCommands", () => {
  it("turns each nav target into a Navegación command that navigates on run", () => {
    const deps = fakeDeps();
    const cmds = buildCommands(deps);
    const nav = cmds.filter((c) => c.section === "Navegación");

    expect(nav.map((c) => c.title)).toEqual(["Ir a POS", "Ir a Caja"]);
    nav[0].run();
    expect(deps.go).toHaveBeenCalledWith("pos");
  });

  it("carries the nav hint as the command subtitle", () => {
    const cmds = buildCommands(fakeDeps());
    const pos = cmds.find((c) => c.title === "Ir a POS")!;
    expect(pos.subtitle).toBe("Punto de venta");
  });

  it("includes a 'Nueva venta' action that opens the POS", () => {
    const deps = fakeDeps();
    const venta = buildCommands(deps).find((c) => c.title === "Nueva venta")!;
    expect(venta.section).toBe("Acciones");
    venta.run();
    expect(deps.go).toHaveBeenCalledWith("pos");
  });

  it("includes the agent command that focuses the ask-bar (the north star)", () => {
    const deps = fakeDeps();
    const agent = buildCommands(deps).find((c) => c.section === "Agente")!;
    expect(agent.title.toLowerCase()).toContain("negocio");
    agent.run();
    expect(deps.focusAgent).toHaveBeenCalledTimes(1);
  });

  it("includes a help command that opens the keyboard cheatsheet", () => {
    const deps = fakeDeps();
    const help = buildCommands(deps).find((c) => c.section === "Ayuda")!;
    help.run();
    expect(deps.openCheatsheet).toHaveBeenCalledTimes(1);
  });
});

describe("renderResults", () => {
  const cmds: Command[] = [
    { id: "a", title: "Ir a POS", section: "Navegación", run: () => {} },
    { id: "b", title: "Ir a Caja", subtitle: "Apertura", section: "Navegación", run: () => {} },
  ];

  it("renders one option per command", () => {
    const html = renderResults(cmds, 0);
    expect(html.match(/role="option"/g)?.length).toBe(2);
  });

  it("marks exactly the selected option with aria-selected", () => {
    const html = renderResults(cmds, 1);
    expect(html).toContain('data-i="1"');
    // The selected row is the one flagged selected.
    const selectedTrue = html.match(/aria-selected="true"/g)?.length ?? 0;
    expect(selectedTrue).toBe(1);
    // And it is the second item (id=cmdk-opt-1).
    expect(html).toMatch(/id="cmdk-opt-1"[^>]*aria-selected="true"|aria-selected="true"[^>]*id="cmdk-opt-1"/);
  });

  it("escapes HTML in titles (no injection from data)", () => {
    const evil: Command[] = [
      { id: "x", title: "<img src=x onerror=alert(1)>", section: "Acciones", run: () => {} },
    ];
    const html = renderResults(evil, 0);
    expect(html).not.toContain("<img src=x");
    expect(html).toContain("&lt;img");
  });

  it("returns an empty string when there are no commands", () => {
    expect(renderResults([], 0)).toBe("");
  });
});
