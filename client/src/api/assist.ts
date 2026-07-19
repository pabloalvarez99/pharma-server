// Agent assist wrappers, "Pregúntale a tu negocio" (ADR-0016)
// (client/src-tauri/src/commands/assist.rs).
import { invoke } from "@tauri-apps/api/core";

/** A WRITE the agent proposes but will NOT run until the owner confirms
 *  (`crates/api/src/v1/assist.rs` propose→confirm contract). `action` is the
 *  machine label (`gasto.crear`, `compra.crear` …); `resumen` is the Spanish
 *  one-liner of exactly what will happen ("Vas a registrar un gasto de $5.000
 *  (nafta)."); `confirm_token` is the opaque, single-use ticket the client hands
 *  back to `/assist/act` to execute. Nothing mutates server-side on `ask` — the
 *  token is the ONLY way to commit. */
export interface AgentProposal {
  action: string;
  resumen: string;
  confirm_token: string;
}

/** The agent's reply (`crates/assist/src/provider.rs::Answer`). `intent` is the
 *  stable machine label (`ventas_hoy` … `desconocido`); `text` the Spanish
 *  answer grounded in the tenant's own data; `data` an optional structured
 *  payload backing the prose (absent when the intent carries no figures).
 *  `proposal` is present ONLY when the question asked for a WRITE — then the UI
 *  must show a confirmation card and run nothing until the owner says yes. */
export interface AssistAnswer {
  intent: string;
  text: string;
  data?: unknown;
  proposal?: AgentProposal;
}

/** Outcome of a confirmed action (`POST /api/v1/assist/act`). `text` is the
 *  Spanish receipt of what just happened ("Gasto de $5.000 registrado."); the
 *  rest of the server payload rides along untyped in `data` (kept loose so the
 *  exact action result shape can grow without breaking the client). */
export interface AssistActResult {
  text?: string;
  data?: unknown;
}

/** POST /api/v1/assist/ask (Bearer, cashier+) — the read-only, offline-first
 *  business agent. Resolves with the answer even when the agent didn't
 *  understand (`intent = "desconocido"` is a normal 200, not an error); a server
 *  failure rejects with a Spanish string. When the owner asks for a write the
 *  answer carries a {@link AgentProposal} — still no mutation until `assistAct`. */
export function assistAsk(serverUrl: string, question: string): Promise<AssistAnswer> {
  return invoke<AssistAnswer>("assist_ask", { serverUrl, question });
}

/** POST /api/v1/assist/act (Bearer) — EXECUTE the write the agent proposed,
 *  authorised by the single-use `confirmToken` from a prior {@link AgentProposal}.
 *  This is the only call that mutates: it must be reachable ONLY from the
 *  owner's explicit "Confirmar" click. A role-denied write (`403`) or an expired
 *  token rejects with the server's Spanish message. */
export function assistAct(
  serverUrl: string,
  confirmToken: string,
): Promise<AssistActResult> {
  return invoke<AssistActResult>("assist_act", { serverUrl, confirmToken });
}
