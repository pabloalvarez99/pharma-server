// @vitest-environment happy-dom
//
// The agent ACTÚA, but only on an explicit yes. These drive the REAL
// `mountAgentAskBar` under a DOM to prove the non-negotiable invariant: a
// proposed write runs `assist_act` ONLY after the owner clicks Confirmar —
// never on `ask`, never on Cancelar. (`assistAsk`/`assistAct` are mocked so no
// Tauri/network is touched; everything else is the production code path.)
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return { ...actual, assistAsk: vi.fn(), assistAct: vi.fn() };
});

import { mountAgentAskBar } from "./askbar";
import { assistAsk, assistAct, type AssistAnswer } from "../api";

const mockedAsk = vi.mocked(assistAsk);
const mockedAct = vi.mocked(assistAct);

const PROPOSAL_ANSWER: AssistAnswer = {
  intent: "gasto_crear",
  text: "Propuesta lista.",
  proposal: {
    action: "gasto.crear",
    resumen: "Vas a registrar un gasto de $5.000 (nafta).",
    confirm_token: "tok_xyz",
  },
};

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function mount(): HTMLElement {
  const host = document.createElement("div");
  document.body.appendChild(host);
  mountAgentAskBar(host, "http://localhost:8080", "minimarket");
  return host;
}

function ask(host: HTMLElement, q: string): void {
  const input = host.querySelector<HTMLInputElement>(".agent-ask-input")!;
  const form = host.querySelector<HTMLFormElement>(".agent-ask-form")!;
  input.value = q;
  form.dispatchEvent(new Event("submit", { cancelable: true, bubbles: true }));
}

beforeEach(() => {
  document.body.innerHTML = "";
  mockedAsk.mockReset();
  mockedAct.mockReset();
});

describe("agent action flow", () => {
  it("asking for a write renders the confirmation card but executes NOTHING", async () => {
    mockedAsk.mockResolvedValue(PROPOSAL_ANSWER);
    const host = mount();

    ask(host, "registra un gasto de 5000 nafta");
    await flush();

    const out = host.querySelector(".agent-ask-out")!;
    expect(out.querySelector(".agent-proposal")).not.toBeNull();
    expect(out.textContent).toContain("Vas a registrar un gasto de $5.000 (nafta).");
    expect(out.querySelector("[data-confirm]")).not.toBeNull();
    // The invariant: no write happened on `ask`.
    expect(mockedAct).not.toHaveBeenCalled();
  });

  it("Confirmar executes the write exactly once with the token, then shows the receipt", async () => {
    mockedAsk.mockResolvedValue(PROPOSAL_ANSWER);
    mockedAct.mockResolvedValue({ text: "Gasto de $5.000 registrado." });
    const host = mount();

    ask(host, "registra un gasto de 5000 nafta");
    await flush();

    host.querySelector<HTMLButtonElement>("[data-confirm]")!.click();
    await flush();

    expect(mockedAct).toHaveBeenCalledTimes(1);
    expect(mockedAct).toHaveBeenCalledWith("http://localhost:8080", "tok_xyz");
    const out = host.querySelector(".agent-ask-out")!;
    expect(out.querySelector(".agent-bubble-ok")).not.toBeNull();
    expect(out.textContent).toContain("Gasto de $5.000 registrado.");
  });

  it("Cancelar runs nothing and returns to the idle suggestions", async () => {
    mockedAsk.mockResolvedValue(PROPOSAL_ANSWER);
    const host = mount();

    ask(host, "registra un gasto de 5000 nafta");
    await flush();

    host.querySelector<HTMLButtonElement>("[data-cancel]")!.click();
    await flush();

    expect(mockedAct).not.toHaveBeenCalled();
    const out = host.querySelector(".agent-ask-out")!;
    expect(out.querySelector(".agent-proposal")).toBeNull();
    expect(out.querySelector(".agent-chip")).not.toBeNull(); // back to suggestions
  });

  it("a role-denied write surfaces the server's Spanish message, not a crash", async () => {
    mockedAsk.mockResolvedValue(PROPOSAL_ANSWER);
    mockedAct.mockRejectedValue("No tienes permiso para registrar gastos.");
    const host = mount();

    ask(host, "registra un gasto de 5000 nafta");
    await flush();
    host.querySelector<HTMLButtonElement>("[data-confirm]")!.click();
    await flush();

    const out = host.querySelector(".agent-ask-out")!;
    const alert = out.querySelector('[role="alert"]');
    expect(alert).not.toBeNull();
    expect(alert!.textContent).toContain("No tienes permiso para registrar gastos.");
  });

  it("a read-only answer never shows a confirmation card", async () => {
    mockedAsk.mockResolvedValue({ intent: "ventas_hoy", text: "Hoy vendiste $42.000." });
    const host = mount();

    ask(host, "cuanto vendi hoy");
    await flush();

    const out = host.querySelector(".agent-ask-out")!;
    expect(out.querySelector(".agent-proposal")).toBeNull();
    expect(out.textContent).toContain("Hoy vendiste $42.000.");
    expect(mockedAct).not.toHaveBeenCalled();
  });
});
