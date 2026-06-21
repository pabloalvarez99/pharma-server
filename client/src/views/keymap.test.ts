import { describe, it, expect } from "vitest";
import {
  normalize,
  scoreCommand,
  filterCommands,
  nextIndex,
  resolveGoto,
  SHORTCUTS,
  type Command,
} from "./keymap";

const noop = () => {};

function cmd(partial: Partial<Command> & Pick<Command, "id" | "title">): Command {
  return { section: "Navegación", run: noop, ...partial };
}

describe("normalize (es-CL, accent-insensitive)", () => {
  it("lowercases and strips Spanish diacritics", () => {
    expect(normalize("Configuración")).toBe("configuracion");
    expect(normalize("Café")).toBe("cafe");
    expect(normalize("AUDITORÍA")).toBe("auditoria");
  });

  it("collapses surrounding whitespace", () => {
    expect(normalize("  POS  ")).toBe("pos");
  });
});

describe("scoreCommand", () => {
  const pos = cmd({ id: "pos", title: "Ir a POS", keywords: ["vender", "boleta"] });

  it("returns -1 when nothing matches", () => {
    expect(scoreCommand(pos, "xyzzy")).toBe(-1);
  });

  it("matches an empty query (neutral score >= 0)", () => {
    expect(scoreCommand(pos, "")).toBeGreaterThanOrEqual(0);
    expect(scoreCommand(pos, "   ")).toBeGreaterThanOrEqual(0);
  });

  it("scores a title prefix higher than a keyword-only match", () => {
    const titlePrefix = scoreCommand(cmd({ id: "a", title: "Vender ahora" }), "vender");
    const keywordOnly = scoreCommand(pos, "vender"); // 'vender' only in keywords
    expect(titlePrefix).toBeGreaterThan(keywordOnly);
  });

  it("is accent-insensitive both ways", () => {
    const c = cmd({ id: "cfg", title: "Configuración" });
    expect(scoreCommand(c, "configuracion")).toBeGreaterThanOrEqual(0);
    expect(scoreCommand(cmd({ id: "x", title: "Auditoria" }), "auditoría")).toBeGreaterThanOrEqual(0);
  });

  it("requires every whitespace-separated token to match (AND)", () => {
    const c = cmd({ id: "pos", title: "Ir a POS" });
    expect(scoreCommand(c, "ir pos")).toBeGreaterThanOrEqual(0);
    expect(scoreCommand(c, "ir caja")).toBe(-1); // 'caja' absent → excluded
  });
});

describe("filterCommands", () => {
  const all: Command[] = [
    cmd({ id: "pos", title: "Ir a POS", keywords: ["vender"] }),
    cmd({ id: "caja", title: "Ir a Caja", keywords: ["arqueo"] }),
    cmd({ id: "cfg", title: "Ir a Configuración" }),
    cmd({ id: "venta", title: "Nueva venta", section: "Acciones", keywords: ["pos"] }),
  ];

  it("returns every command in original order for an empty query", () => {
    expect(filterCommands(all, "").map((c) => c.id)).toEqual(["pos", "caja", "cfg", "venta"]);
  });

  it("excludes commands that do not match", () => {
    const ids = filterCommands(all, "caja").map((c) => c.id);
    expect(ids).toEqual(["caja"]);
  });

  it("finds an accented title with an unaccented query", () => {
    const ids = filterCommands(all, "configuracion").map((c) => c.id);
    expect(ids).toContain("cfg");
  });

  it("ranks a title match above a keyword-only match", () => {
    // 'venta' is in title of 'venta' and nowhere in 'pos'; 'pos' keyword on 'venta'.
    const ids = filterCommands(all, "venta").map((c) => c.id);
    expect(ids[0]).toBe("venta");
  });

  it("is stable: equal scores keep input order", () => {
    const ties: Command[] = [
      cmd({ id: "b", title: "Item" }),
      cmd({ id: "a", title: "Item" }),
    ];
    expect(filterCommands(ties, "item").map((c) => c.id)).toEqual(["b", "a"]);
  });
});

describe("nextIndex (keyboard wrap)", () => {
  it("moves forward and wraps past the end", () => {
    expect(nextIndex(0, 3, 1)).toBe(1);
    expect(nextIndex(2, 3, 1)).toBe(0);
  });

  it("moves backward and wraps before the start", () => {
    expect(nextIndex(0, 3, -1)).toBe(2);
    expect(nextIndex(1, 3, -1)).toBe(0);
  });

  it("returns -1 for an empty list", () => {
    expect(nextIndex(0, 0, 1)).toBe(-1);
  });
});

describe("resolveGoto (g-chord nav)", () => {
  it("maps known mnemonic keys to a nav id, case-insensitively", () => {
    expect(resolveGoto("p")).toBe("pos");
    expect(resolveGoto("P")).toBe("pos");
    expect(resolveGoto("d")).toBe("dashboard");
  });

  it("returns null for an unmapped key", () => {
    expect(resolveGoto("z")).toBeNull();
  });
});

describe("SHORTCUTS catalog", () => {
  it("documents the palette and cheatsheet keys for the discoverable overlay", () => {
    const blob = SHORTCUTS.map((s) => `${s.keys.join("")} ${s.label}`)
      .join(" ")
      .toLowerCase();
    expect(blob).toContain("k"); // Ctrl/Cmd K opens the palette
    expect(blob).toContain("?"); // ? opens this very cheatsheet
    expect(SHORTCUTS.length).toBeGreaterThan(0);
  });
});
