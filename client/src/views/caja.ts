// Caja view — cash register lifecycle for the counter operator:
//   • show the CURRENT open session (register, opening cash, opened-at) or a
//     clear "sin caja abierta" empty state.
//   • "Abrir caja" → modal asking the opening amount (+ optional register name)
//     → POST /cash-sessions.
//   • "Cerrar caja" → fetch the arqueo (expected cash) first, then a modal to
//     enter the counted cash; on confirm POST /cash-sessions/{id}/close and show
//     expected vs counted + the discrepancy.
// Money crosses the wire as STRINGS (Decimal) and is re-emitted verbatim; local
// Number is display-only. Spanish throughout, CLP via ../format. Same skeleton →
// fetch → swap pattern as the other views.
import {
  cashSessions,
  openCashSession,
  cashArqueo,
  closeCashSession,
  type CashSession,
  type CashCloseSummary,
} from "../api";
import { clp, toNumber } from "../format";
import { kpiCard, kpiSkeleton, asMessage, escapeHtml } from "./inventory";

export function renderCaja(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-caja">
      <div class="view-head">
        <div>
          <h2>Caja</h2>
          <p class="muted">Apertura, cierre y arqueo del turno.</p>
        </div>
        <div id="caja-actions"></div>
      </div>

      <div id="caja-body" class="kpi-grid">${kpiSkeleton(4)}</div>

      <div id="caja-modal-host"></div>
      <div id="caja-toast" class="toast" hidden></div>
    </section>
  `;

  const bodyEl = host.querySelector<HTMLElement>("#caja-body")!;
  const actionsEl = host.querySelector<HTMLElement>("#caja-actions")!;
  const modalHost = host.querySelector<HTMLElement>("#caja-modal-host")!;
  const toastEl = host.querySelector<HTMLElement>("#caja-toast")!;

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  // Re-fetch the current open session and repaint body + actions.
  async function refresh(): Promise<void> {
    bodyEl.className = "kpi-grid";
    bodyEl.innerHTML = kpiSkeleton(4);
    actionsEl.innerHTML = "";
    try {
      const open = await cashSessions(serverUrl, "open", 1);
      const session = open[0];
      if (!session) {
        renderClosed();
      } else {
        renderOpen(session);
      }
    } catch (err) {
      bodyEl.className = "";
      bodyEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function renderClosed(): void {
    bodyEl.className = "";
    bodyEl.innerHTML = `
      <div class="caja-empty">
        <div class="caja-empty-mark">●</div>
        <h3>Sin caja abierta</h3>
        <p class="muted">Abre la caja para comenzar a registrar ventas en efectivo del turno.</p>
        <button id="caja-open-btn" class="btn-primary caja-open">
          <span class="btn-label">Abrir caja</span>
          <span class="btn-pulse"></span>
        </button>
      </div>
    `;
    bodyEl.querySelector<HTMLButtonElement>("#caja-open-btn")!
      .addEventListener("click", () => openModal(modalHost, serverUrl, async () => {
        toast("Caja abierta");
        await refresh();
      }));
  }

  function renderOpen(s: CashSession): void {
    bodyEl.className = "kpi-grid";
    bodyEl.innerHTML = [
      kpiCard("Caja", escapeHtml(s.register_name), `abierta ${fmtDateTime(s.opened_at)}`, "accent"),
      kpiCard("Monto inicial", clp(s.opening_cash), "fondo de apertura"),
      kpiCard("Estado", "Abierta", s.opening_notes ? escapeHtml(s.opening_notes) : "turno en curso"),
    ].join("");
    actionsEl.innerHTML = `<button id="caja-close-btn" class="btn-ghost-danger">Cerrar caja</button>`;
    actionsEl.querySelector<HTMLButtonElement>("#caja-close-btn")!
      .addEventListener("click", () => closeFlow(modalHost, serverUrl, s, async (summary) => {
        toast(`Caja cerrada · ${describeDiscrepancy(summary.session)}`);
        await refresh();
      }));
  }

  void refresh();
}

// --- open flow -------------------------------------------------------------

function openModal(
  modalHost: HTMLElement,
  serverUrl: string,
  onDone: () => void | Promise<void>,
): void {
  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal" role="dialog" aria-modal="true" aria-label="Abrir caja">
        <h3 class="modal-title">Abrir caja</h3>
        <label class="field modal-field">
          <span class="modal-label">Nombre de la caja</span>
          <input id="caja-name" type="text" value="caja-1" autocomplete="off" />
        </label>
        <label class="field modal-field">
          <span class="modal-label">Monto inicial (CLP)</span>
          <input id="caja-amount" type="number" inputmode="numeric" min="0" step="1" placeholder="0" />
        </label>
        <div id="caja-modal-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button id="caja-cancel" class="btn-ghost">Cancelar</button>
          <button id="caja-confirm" class="btn-primary modal-confirm">
            <span class="btn-label">Abrir</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = () => (modalHost.innerHTML = "");
  const nameEl = modalHost.querySelector<HTMLInputElement>("#caja-name")!;
  const amountEl = modalHost.querySelector<HTMLInputElement>("#caja-amount")!;
  const errEl = modalHost.querySelector<HTMLElement>("#caja-modal-error")!;
  const confirmBtn = modalHost.querySelector<HTMLButtonElement>("#caja-confirm")!;

  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#caja-cancel")!.addEventListener("click", close);
  amountEl.focus();

  confirmBtn.addEventListener("click", async () => {
    const raw = amountEl.value.trim();
    const n = Number(raw);
    if (raw === "" || !Number.isFinite(n) || n < 0) {
      errEl.textContent = "Ingresa un monto inicial válido (0 o más).";
      errEl.hidden = false;
      return;
    }
    const name = nameEl.value.trim() || "caja-1";
    errEl.hidden = true;
    confirmBtn.disabled = true;
    confirmBtn.classList.add("loading");
    try {
      // Send the amount as a plain integer string — server parses Decimal.
      await openCashSession(serverUrl, name, String(Math.trunc(n)));
      close();
      await onDone();
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
      confirmBtn.disabled = false;
      confirmBtn.classList.remove("loading");
    }
  });
}

// --- close flow ------------------------------------------------------------
// Two steps: fetch the arqueo (expected cash) so the operator sees the target,
// then collect the counted amount in the same modal and POST the close.

async function closeFlow(
  modalHost: HTMLElement,
  serverUrl: string,
  session: CashSession,
  onDone: (summary: CashCloseSummary) => void | Promise<void>,
): Promise<void> {
  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal" role="dialog" aria-modal="true" aria-label="Cerrar caja">
        <h3 class="modal-title">Cerrar caja</h3>
        <div id="caja-arqueo" class="arqueo-preview">${arqueoSkeleton()}</div>
        <label class="field modal-field">
          <span class="modal-label">Efectivo contado (CLP)</span>
          <input id="caja-counted" type="number" inputmode="numeric" min="0" step="1" placeholder="0" />
        </label>
        <div id="caja-diff" class="arqueo-diff" hidden></div>
        <div id="caja-modal-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button id="caja-cancel" class="btn-ghost">Cancelar</button>
          <button id="caja-confirm" class="btn-primary modal-confirm" disabled>
            <span class="btn-label">Cerrar caja</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = () => (modalHost.innerHTML = "");
  const arqueoEl = modalHost.querySelector<HTMLElement>("#caja-arqueo")!;
  const countedEl = modalHost.querySelector<HTMLInputElement>("#caja-counted")!;
  const diffEl = modalHost.querySelector<HTMLElement>("#caja-diff")!;
  const errEl = modalHost.querySelector<HTMLElement>("#caja-modal-error")!;
  const confirmBtn = modalHost.querySelector<HTMLButtonElement>("#caja-confirm")!;

  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#caja-cancel")!.addEventListener("click", close);

  // Expected cash = opening_cash + cash_sales + movements_in - movements_out.
  let expected = toNumber(session.opening_cash);
  try {
    const a = await cashArqueo(serverUrl, session.id);
    expected =
      toNumber(a.session.opening_cash) +
      toNumber(a.cash_sales) +
      toNumber(a.movements_in) -
      toNumber(a.movements_out);
    arqueoEl.innerHTML = `
      <div class="arqueo-row"><span>Apertura</span><strong>${clp(a.session.opening_cash)}</strong></div>
      <div class="arqueo-row"><span>Ventas efectivo</span><strong>${clp(a.cash_sales)}</strong></div>
      <div class="arqueo-row"><span>Ingresos</span><strong>${clp(a.movements_in)}</strong></div>
      <div class="arqueo-row"><span>Egresos</span><strong>−${clp(a.movements_out)}</strong></div>
      <div class="arqueo-row arqueo-total"><span>Esperado en caja</span><strong>${clp(expected)}</strong></div>
    `;
  } catch (err) {
    arqueoEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }

  const recomputeDiff = () => {
    const raw = countedEl.value.trim();
    confirmBtn.disabled = raw === "";
    if (raw === "") {
      diffEl.hidden = true;
      return;
    }
    const counted = toNumber(raw);
    const diff = counted - expected;
    const tone = diff === 0 ? "ok" : diff > 0 ? "warn" : "danger";
    const label = diff === 0 ? "Cuadra" : diff > 0 ? "Sobrante" : "Faltante";
    diffEl.className = `arqueo-diff arqueo-${tone}`;
    diffEl.innerHTML = `${label}: <strong>${clp(Math.abs(diff))}</strong>`;
    diffEl.hidden = false;
  };
  countedEl.addEventListener("input", recomputeDiff);
  countedEl.focus();

  confirmBtn.addEventListener("click", async () => {
    const raw = countedEl.value.trim();
    const n = Number(raw);
    if (raw === "" || !Number.isFinite(n) || n < 0) {
      errEl.textContent = "Ingresa el efectivo contado (0 o más).";
      errEl.hidden = false;
      return;
    }
    errEl.hidden = true;
    confirmBtn.disabled = true;
    confirmBtn.classList.add("loading");
    try {
      const summary = await closeCashSession(serverUrl, session.id, String(Math.trunc(n)));
      close();
      await onDone(summary);
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
      confirmBtn.disabled = false;
      confirmBtn.classList.remove("loading");
    }
  });
}

// --- helpers ---------------------------------------------------------------

function describeDiscrepancy(s: CashSession): string {
  if (s.discrepancia == null) return "arqueo registrado";
  const d = toNumber(s.discrepancia);
  if (d === 0) return "cuadró exacto";
  return d > 0 ? `sobrante ${clp(Math.abs(d))}` : `faltante ${clp(Math.abs(d))}`;
}

function fmtDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-CL", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function arqueoSkeleton(): string {
  return `<div class="table-skel">${Array.from({ length: 4 })
    .map(() => `<div class="sk sk-row"></div>`)
    .join("")}</div>`;
}
