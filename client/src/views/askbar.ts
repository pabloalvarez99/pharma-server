// Agent ask-bar — "Pregúntale a tu negocio" (ADR-0016).
//
// This is the north star made visible: 1 RUT = 1 negocio = 1 agente. The owner
// TYPES a plain-Spanish question ("¿cuánto vendí hoy?", "¿qué se vence?") and
// the agent answers from the tenant's OWN data — read-only, offline-first, no
// LLM in this MVP (the server runs a deterministic intent parser, ADR-0016).
//
// The contract (crates/api/src/v1/assist.rs) is consumed STABLE here:
//   POST /api/v1/assist/ask { question } -> { intent, text, data? }
// "No entendí" is NOT an error — it is a normal 200 with intent "desconocido";
// we flag it (`understood = false`) so the UI nudges with example questions
// instead of exploding in the owner's face (the agent always answers something).
//
// Pure logic (state mapping, suggested questions, output HTML) is separated from
// the DOM mount so it is unit-testable without a browser (see askbar.test.ts).
import { assistAsk, type AssistAnswer } from "../api";
import { escapeHtml } from "./inventory";
import { featuresForRubro, type Rubro } from "../vertical";

/** The intent label the server returns when it could not classify the question
 *  (mirrors `assist::Intent::Unknown.label()`). */
export const UNKNOWN_INTENT = "desconocido";

/** Shown when the server answers 200 but with empty prose (defensive — the
 *  deterministic provider always fills `text`, but never trust the wire). */
export const FALLBACK_TEXT = "No tengo una respuesta para eso todavía.";

/** The ask-bar's presentation state. A tiny explicit machine so every visual
 *  case (idle / loading / answered / didn't-understand / error) is handled. */
export type AskState =
  | { kind: "idle" }
  | { kind: "loading"; question: string }
  | {
      kind: "answer";
      question: string;
      text: string;
      intent: string;
      /** false when intent === "desconocido" → render the gentle nudge. */
      understood: boolean;
    }
  | { kind: "error"; question: string; message: string };

/** Map a raw server answer into an `answer` state. "No entendí" stays a normal
 *  answer (it has friendly Spanish prose) but is flagged `understood = false`. */
export function answerState(question: string, ans: AssistAnswer): AskState {
  const text = (ans.text ?? "").trim();
  const intent = (ans.intent ?? UNKNOWN_INTENT).trim() || UNKNOWN_INTENT;
  return {
    kind: "answer",
    question,
    text: text.length > 0 ? text : FALLBACK_TEXT,
    intent,
    understood: intent !== UNKNOWN_INTENT,
  };
}

/** Coerce a rejected `assist_ask` (Spanish string) into an error state. */
export function errorState(question: string, err: unknown): AskState {
  const message =
    typeof err === "string" && err.trim().length > 0
      ? err.trim()
      : "No se pudo consultar al agente. Intenta nuevamente.";
  return { kind: "error", question, message };
}

/** Example questions seeded into the ask-bar, adapted to the active rubro so the
 *  prompts read native — a service rubro (no stock) is never told to ask about
 *  vencimientos, and only farmacia phrasing leaks into farmacia. Pure: drives
 *  both the chips UI and the "no entendí" nudge. Multi-rubro, es-CL, no copy
 *  hardcoded to pharmacy. */
export function suggestedQuestions(rubro: Rubro): string[] {
  const f = featuresForRubro(rubro);
  const out = ["¿Cuánto vendí hoy?", "¿Cuánto vendí este mes?", "¿Cuánto hay en caja?"];
  out.push(
    f.physicalStock
      ? "¿Qué productos se están agotando?"
      : "¿Cuáles son mis servicios más vendidos?",
  );
  if (f.physicalStock) out.push("¿Cuáles son mis más vendidos?");
  if (f.lotes) out.push("¿Qué se está por vencer?");
  return out;
}

// --- pure HTML (no DOM API; testable as strings) ----------------------------

function chip(q: string): string {
  return `<button type="button" class="agent-chip" data-q="${escapeHtml(q)}">${escapeHtml(q)}</button>`;
}

function suggestBlock(rubro: Rubro, lead: string): string {
  const chips = suggestedQuestions(rubro).map(chip).join("");
  return `<p class="agent-suggest-lead">${escapeHtml(lead)}</p><div class="agent-chips">${chips}</div>`;
}

/** The output region's inner HTML for a given state. Pure — fed the same state
 *  in tests. The spinner / answer / nudge / error are all just markup here. */
export function renderAskOutput(state: AskState, rubro: Rubro): string {
  switch (state.kind) {
    case "idle":
      return suggestBlock(rubro, "Prueba preguntando:");
    case "loading":
      return `
        <div class="agent-thinking" role="status">
          <span class="agent-dots" aria-hidden="true"><i></i><i></i><i></i></span>
          <span class="agent-thinking-txt">Pensando con tus datos…</span>
        </div>`;
    case "error":
      return `<div class="agent-bubble agent-bubble-error" role="alert">${escapeHtml(state.message)}</div>`;
    case "answer": {
      const bubble = `<div class="agent-bubble${
        state.understood ? "" : " agent-bubble-soft"
      }">${escapeHtml(state.text)}</div>`;
      // Didn't understand → re-offer example questions so it's never a dead end.
      const nudge = state.understood ? "" : suggestBlock(rubro, "Puedo responder, por ejemplo:");
      return bubble + nudge;
    }
  }
}

// --- DOM mount --------------------------------------------------------------

/** Handle returned to the shell so a keyboard shortcut can focus the ask-bar. */
export interface AskBarHandle {
  focus(): void;
}

/** Mount the agent ask-bar into `host`. Wires the input + suggestion chips to
 *  the `assist_ask` Tauri command and renders each state into a live region.
 *  Keyboard-first: returns a `focus()` so the shell's "/" shortcut lands here. */
export function mountAgentAskBar(
  host: HTMLElement,
  serverUrl: string,
  rubro: Rubro,
): AskBarHandle {
  host.innerHTML = `
    <section class="agent-ask" aria-label="Pregúntale a tu negocio">
      <div class="agent-ask-head">
        <span class="agent-spark" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 3.2l1.7 4.3 4.3 1.7-4.3 1.7L12 15.2l-1.7-4.3L6 9.2l4.3-1.7z"/>
            <path d="M18.5 14.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z"/>
          </svg>
        </span>
        <div class="agent-ask-headtext">
          <h3 class="agent-ask-title">Pregúntale a tu negocio</h3>
          <p class="agent-ask-sub">Tu agente responde con tus propios datos, sin salir del local. <kbd>/</kbd> para preguntar.</p>
        </div>
      </div>
      <form class="agent-ask-form" autocomplete="off">
        <input
          class="agent-ask-input"
          type="text"
          name="q"
          placeholder="¿Cuánto vendí hoy?"
          aria-label="Escribe tu pregunta"
          maxlength="200" />
        <button type="submit" class="agent-ask-send" aria-label="Preguntar">
          <span class="agent-ask-send-txt">Preguntar</span>
        </button>
      </form>
      <div class="agent-ask-out" aria-live="polite" aria-busy="false"></div>
    </section>
  `;

  const form = host.querySelector<HTMLFormElement>(".agent-ask-form")!;
  const input = host.querySelector<HTMLInputElement>(".agent-ask-input")!;
  const send = host.querySelector<HTMLButtonElement>(".agent-ask-send")!;
  const out = host.querySelector<HTMLElement>(".agent-ask-out")!;

  let seq = 0; // guards against an out-of-order late response overwriting a newer one

  const paint = (state: AskState): void => {
    out.innerHTML = renderAskOutput(state, rubro);
    out.setAttribute("aria-busy", state.kind === "loading" ? "true" : "false");
    bindChips();
  };

  const bindChips = (): void => {
    out.querySelectorAll<HTMLButtonElement>(".agent-chip").forEach((c) => {
      c.addEventListener("click", () => {
        input.value = c.dataset.q ?? "";
        void run(input.value);
      });
    });
  };

  const run = async (raw: string): Promise<void> => {
    const question = raw.trim();
    if (question.length === 0) {
      input.focus();
      return;
    }
    const mine = ++seq;
    send.disabled = true;
    input.setAttribute("aria-busy", "true");
    paint({ kind: "loading", question });
    try {
      const ans = await assistAsk(serverUrl, question);
      if (mine === seq) paint(answerState(question, ans));
    } catch (err) {
      if (mine === seq) paint(errorState(question, err));
    } finally {
      if (mine === seq) {
        send.disabled = false;
        input.removeAttribute("aria-busy");
      }
    }
  };

  form.addEventListener("submit", (e) => {
    e.preventDefault();
    void run(input.value);
  });

  paint({ kind: "idle" });

  return {
    focus() {
      input.focus();
      input.select();
    },
  };
}
