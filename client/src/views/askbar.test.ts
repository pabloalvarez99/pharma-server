import { describe, it, expect } from "vitest";
import {
  answerState,
  errorState,
  responseState,
  doneState,
  isProposal,
  suggestedQuestions,
  renderAskOutput,
  UNKNOWN_INTENT,
  FALLBACK_TEXT,
  FALLBACK_DONE_TEXT,
  type AskState,
} from "./askbar";
import type { AssistAnswer, AgentProposal } from "../api";

const PROPOSAL: AgentProposal = {
  action: "gasto.crear",
  resumen: "Vas a registrar un gasto de $5.000 (nafta).",
  confirm_token: "tok_abc123",
};

describe("answerState", () => {
  it("flags a recognised intent as understood", () => {
    const ans: AssistAnswer = { intent: "ventas_hoy", text: "Hoy vendiste $42.000." };
    const s = answerState("¿cuánto vendí hoy?", ans);
    expect(s).toMatchObject({ kind: "answer", understood: true, intent: "ventas_hoy" });
    if (s.kind === "answer") expect(s.text).toBe("Hoy vendiste $42.000.");
  });

  it("marks desconocido as NOT understood (no error)", () => {
    const ans: AssistAnswer = { intent: UNKNOWN_INTENT, text: "No te entendí bien." };
    const s = answerState("bla bla", ans);
    expect(s.kind).toBe("answer"); // never an error
    if (s.kind === "answer") {
      expect(s.understood).toBe(false);
      expect(s.text).toBe("No te entendí bien.");
    }
  });

  it("falls back when the server sends empty prose", () => {
    const ans: AssistAnswer = { intent: "ventas_hoy", text: "   " };
    const s = answerState("q", ans);
    if (s.kind === "answer") expect(s.text).toBe(FALLBACK_TEXT);
  });

  it("defaults a missing intent to desconocido", () => {
    const ans = { text: "algo" } as unknown as AssistAnswer;
    const s = answerState("q", ans);
    if (s.kind === "answer") {
      expect(s.intent).toBe(UNKNOWN_INTENT);
      expect(s.understood).toBe(false);
    }
  });
});

describe("errorState", () => {
  it("uses the server's Spanish string verbatim", () => {
    const s = errorState("q", "Permiso denegado para esta operación.");
    expect(s).toEqual({
      kind: "error",
      question: "q",
      message: "Permiso denegado para esta operación.",
    });
  });

  it("has a friendly fallback for non-string / empty rejects", () => {
    const s = errorState("q", new Error("boom"));
    if (s.kind === "error") expect(s.message.length).toBeGreaterThan(0);
  });
});

describe("suggestedQuestions", () => {
  it("offers stock + vencimiento prompts to a product rubro with lotes", () => {
    const qs = suggestedQuestions("farmacia");
    expect(qs).toContain("¿Qué se está por vencer?");
    expect(qs.some((q) => q.toLowerCase().includes("agotando"))).toBe(true);
  });

  it("never offers vencimiento/stock prompts to a service rubro", () => {
    const qs = suggestedQuestions("belleza");
    expect(qs.some((q) => q.includes("vencer"))).toBe(false);
    expect(qs.some((q) => q.toLowerCase().includes("agotando"))).toBe(false);
    expect(qs.some((q) => q.toLowerCase().includes("servicios"))).toBe(true);
  });

  it("always includes the core sales/caja prompts for any rubro", () => {
    for (const r of ["otro", "tienda", "servicios"] as const) {
      const qs = suggestedQuestions(r);
      expect(qs).toContain("¿Cuánto vendí hoy?");
      expect(qs).toContain("¿Cuánto hay en caja?");
    }
  });
});

describe("renderAskOutput", () => {
  it("renders suggestion chips when idle", () => {
    const html = renderAskOutput({ kind: "idle" }, "otro");
    expect(html).toContain("agent-chip");
    expect(html).toContain("¿Cuánto vendí hoy?");
  });

  it("shows a thinking indicator while loading", () => {
    const html = renderAskOutput({ kind: "loading", question: "q" }, "otro");
    expect(html).toContain("agent-thinking");
    expect(html).toContain("Pensando");
  });

  it("renders an understood answer as a plain bubble (no nudge chips)", () => {
    const s: AskState = {
      kind: "answer",
      question: "q",
      text: "Hoy vendiste $10.000.",
      intent: "ventas_hoy",
      understood: true,
    };
    const html = renderAskOutput(s, "otro");
    expect(html).toContain("Hoy vendiste $10.000.");
    expect(html).not.toContain("agent-chip");
  });

  it("re-offers example chips on a not-understood answer", () => {
    const s: AskState = {
      kind: "answer",
      question: "q",
      text: "No te entendí.",
      intent: UNKNOWN_INTENT,
      understood: false,
    };
    const html = renderAskOutput(s, "otro");
    expect(html).toContain("No te entendí.");
    expect(html).toContain("agent-chip");
  });

  it("escapes answer text (no raw HTML injection)", () => {
    const s: AskState = {
      kind: "answer",
      question: "q",
      text: "<img src=x onerror=alert(1)>",
      intent: "ventas_hoy",
      understood: true,
    };
    const html = renderAskOutput(s, "otro");
    expect(html).not.toContain("<img src=x");
    expect(html).toContain("&lt;img");
  });

  it("renders errors in an alert role", () => {
    const html = renderAskOutput({ kind: "error", question: "q", message: "Sin conexión." }, "otro");
    expect(html).toContain('role="alert"');
    expect(html).toContain("Sin conexión.");
  });
});

describe("isProposal", () => {
  it("is true for an answer carrying a usable confirm_token", () => {
    const ans: AssistAnswer = { intent: "gasto_crear", text: "…", proposal: PROPOSAL };
    expect(isProposal(ans)).toBe(true);
  });

  it("is false for a read-only answer (no proposal)", () => {
    expect(isProposal({ intent: "ventas_hoy", text: "Hoy vendiste $1." })).toBe(false);
  });

  it("is false when the token is empty/blank (never confirm nothing)", () => {
    const ans: AssistAnswer = {
      intent: "gasto_crear",
      text: "…",
      proposal: { ...PROPOSAL, confirm_token: "   " },
    };
    expect(isProposal(ans)).toBe(false);
  });
});

describe("responseState", () => {
  it("maps a write proposal to a proposal state (NOT an answer)", () => {
    const s = responseState("registra un gasto de 5000 nafta", {
      intent: "gasto_crear",
      text: "…",
      proposal: PROPOSAL,
    });
    expect(s.kind).toBe("proposal");
    if (s.kind === "proposal") expect(s.proposal.confirm_token).toBe("tok_abc123");
  });

  it("routes a normal answer through answerState", () => {
    const s = responseState("¿cuánto vendí hoy?", { intent: "ventas_hoy", text: "Hoy $10." });
    expect(s).toMatchObject({ kind: "answer", understood: true, intent: "ventas_hoy" });
  });
});

describe("doneState", () => {
  it("uses the server receipt prose", () => {
    const s = doneState("q", { text: "Gasto de $5.000 registrado." });
    expect(s).toEqual({ kind: "done", question: "q", text: "Gasto de $5.000 registrado." });
  });

  it("falls back to a friendly receipt when prose is empty", () => {
    const s = doneState("q", { text: "  " });
    if (s.kind === "done") expect(s.text).toBe(FALLBACK_DONE_TEXT);
  });
});

describe("renderAskOutput — action states", () => {
  it("renders a proposal card with Confirmar/Cancelar and the resumen", () => {
    const html = renderAskOutput({ kind: "proposal", question: "q", proposal: PROPOSAL }, "otro");
    expect(html).toContain("data-confirm");
    expect(html).toContain("data-cancel");
    expect(html).toContain("Confirmar");
    expect(html).toContain("Cancelar");
    expect(html).toContain("Vas a registrar un gasto de $5.000 (nafta).");
    expect(html).toContain("no hará nada hasta que confirmes");
  });

  it("never leaks the confirm_token into the markup", () => {
    const html = renderAskOutput({ kind: "proposal", question: "q", proposal: PROPOSAL }, "otro");
    expect(html).not.toContain("tok_abc123");
  });

  it("escapes the resumen (no HTML injection from the agent)", () => {
    const evil: AgentProposal = { ...PROPOSAL, resumen: "<img src=x onerror=alert(1)>" };
    const html = renderAskOutput({ kind: "proposal", question: "q", proposal: evil }, "otro");
    expect(html).not.toContain("<img src=x");
    expect(html).toContain("&lt;img");
  });

  it("locks the buttons and shows progress while executing", () => {
    const html = renderAskOutput({ kind: "executing", question: "q", proposal: PROPOSAL }, "otro");
    expect(html).toContain("Registrando…");
    expect(html).toContain("disabled");
  });

  it("renders the done receipt as a success bubble", () => {
    const html = renderAskOutput({ kind: "done", question: "q", text: "Gasto registrado." }, "otro");
    expect(html).toContain("agent-bubble-ok");
    expect(html).toContain("Gasto registrado.");
    expect(html).toContain('role="status"');
  });
});
