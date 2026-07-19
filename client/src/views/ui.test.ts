// Design-system helper contract (W5, Pilar C). These are pure string producers —
// the canonical empty/loading/error/button/card surfaces every view renders so a
// fresh install never shows a bare "de dev" placeholder. Asserted on the emitted
// markup (no DOM needed): structure, escaping, and the optional slots.
import { describe, it, expect } from "vitest";
import {
  escapeHtml,
  emptyState,
  errorState,
  loadingState,
  skeletonLines,
  button,
  card,
} from "./ui";

describe("escapeHtml", () => {
  it("escapes the five HTML-significant characters", () => {
    expect(escapeHtml(`<a href="x">'y' & z</a>`)).toBe(
      "&lt;a href=&quot;x&quot;&gt;&#39;y&#39; &amp; z&lt;/a&gt;",
    );
  });
  it("leaves safe text untouched", () => {
    expect(escapeHtml("Sin registros 123")).toBe("Sin registros 123");
  });
});

describe("emptyState", () => {
  it("renders a produced empty block carrying the title", () => {
    const html = emptyState({ title: "Sin boletas" });
    expect(html).toContain("ui-empty");
    expect(html).toContain("ui-empty-title");
    expect(html).toContain("Sin boletas");
  });
  it("includes the hint copy when provided", () => {
    const html = emptyState({ title: "T", hint: "Emite tu primera boleta" });
    expect(html).toContain("ui-empty-hint");
    expect(html).toContain("Emite tu primera boleta");
  });
  it("omits the hint node when no hint is given", () => {
    expect(emptyState({ title: "T" })).not.toContain("ui-empty-hint");
  });
  it("escapes both the title and the hint", () => {
    const html = emptyState({ title: "<x>", hint: "<y>" });
    expect(html).toContain("&lt;x&gt;");
    expect(html).toContain("&lt;y&gt;");
    expect(html).not.toContain("<x>");
    expect(html).not.toContain("<y>");
  });
  it("renders an action button with the given id and label", () => {
    const html = emptyState({ title: "T", action: { id: "go-create", label: "Crear" } });
    expect(html).toContain(`id="go-create"`);
    expect(html).toContain("Crear");
    expect(html).toContain("ui-btn");
  });
  it("omits the action button by default", () => {
    expect(emptyState({ title: "T" })).not.toContain("ui-btn");
  });
});

describe("errorState", () => {
  it("renders a produced error block flagged for assistive tech", () => {
    const html = errorState("No se pudo cargar");
    expect(html).toContain("ui-error");
    expect(html).toContain(`role="alert"`);
    expect(html).toContain("No se pudo cargar");
  });
  it("escapes the message", () => {
    const html = errorState("<b>boom</b>");
    expect(html).toContain("&lt;b&gt;boom&lt;/b&gt;");
    expect(html).not.toContain("<b>boom</b>");
  });
  it("adds a retry button with the given id when requested", () => {
    const html = errorState("x", { retry: { id: "rep-retry", label: "Reintentar" } });
    expect(html).toContain("ui-error-retry");
    expect(html).toContain(`id="rep-retry"`);
    expect(html).toContain("Reintentar");
  });
  it("omits the retry button by default", () => {
    expect(errorState("x")).not.toContain("ui-error-retry");
  });
});

describe("loadingState", () => {
  it("renders a polite status with a spinner and the default label", () => {
    const html = loadingState();
    expect(html).toContain("ui-loading");
    expect(html).toContain("ui-spinner");
    expect(html).toContain(`role="status"`);
    expect(html).toContain("Cargando…");
  });
  it("uses a custom label when provided", () => {
    expect(loadingState({ label: "Consultando folios…" })).toContain("Consultando folios…");
  });
  it("escapes the label", () => {
    expect(loadingState({ label: "<x>" })).toContain("&lt;x&gt;");
  });
});

describe("skeletonLines", () => {
  it("renders the requested number of shimmer lines", () => {
    const html = skeletonLines(3);
    expect(html).toContain("ui-skeleton");
    expect((html.match(/ui-sk-line/g) ?? []).length).toBe(3);
  });
  it("clamps a non-positive count to a single line", () => {
    expect((skeletonLines(0).match(/ui-sk-line/g) ?? []).length).toBe(1);
  });
});

describe("button", () => {
  it("renders a primary, type=button control with an escaped label", () => {
    const html = button({ label: "Guardar" });
    expect(html).toContain("ui-btn");
    expect(html).toContain("ui-btn-primary");
    expect(html).toContain("Guardar");
    expect(html).toContain(`type="button"`);
  });
  it("applies the requested variant", () => {
    expect(button({ label: "x", variant: "ghost" })).toContain("ui-btn-ghost");
    expect(button({ label: "x", variant: "danger" })).toContain("ui-btn-danger");
  });
  it("sets id and type when provided", () => {
    const html = button({ label: "x", id: "save", type: "submit" });
    expect(html).toContain(`id="save"`);
    expect(html).toContain(`type="submit"`);
  });
  it("escapes the label", () => {
    expect(button({ label: "<x>" })).toContain("&lt;x&gt;");
  });
});

describe("card", () => {
  it("wraps trusted body markup in a ui-card without escaping it", () => {
    const html = card({ body: "<p>hola</p>" });
    expect(html).toContain("ui-card");
    expect(html).toContain("<p>hola</p>");
  });
  it("renders an escaped title when provided", () => {
    const html = card({ title: "<x>", body: "y" });
    expect(html).toContain("ui-card-title");
    expect(html).toContain("&lt;x&gt;");
    expect(html).not.toContain("<x>");
  });
  it("renders an actions slot when provided", () => {
    const html = card({ body: "y", actions: `<button id="a">A</button>` });
    expect(html).toContain("ui-card-actions");
    expect(html).toContain(`id="a"`);
  });
});
