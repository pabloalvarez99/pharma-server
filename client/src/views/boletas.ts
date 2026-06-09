// Boletas view — DTE / boleta electrónica SII over the /api/v1/dte surface
// (PR #118 wiring). Three blocks:
//   • CAF banner    → /dte/caf-status (folios restantes; warns when low)
//   • Emitir        → POST /dte/boletas (order id + cert passphrase; cashier+)
//   • Listado       → /dte filtered by estado, with per-row actions:
//                     XML (Blob download, Free-tier export), Enviar SII
//                     (admin+, tier-gated → upgrade note on 402), Consultar
//                     (poll verdict) and Anular (pre-envío, asks a reason).
// Same skeleton → fetch → swap pattern as the other views; Spanish throughout.
import {
  listDtes,
  dteCafStatus,
  dteXml,
  emitBoleta,
  sendDte,
  pollDte,
  cancelDte,
  parseSaleError,
  type Dte,
} from "../api";
import { clp, num } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";

const LIST_LIMIT = 100;
const LOW_FOLIOS = 50;

const ESTADOS = [
  { value: "", label: "Todos" },
  { value: "signed", label: "Firmadas" },
  { value: "sent", label: "Enviadas" },
  { value: "accepted", label: "Aceptadas" },
  { value: "rejected", label: "Rechazadas" },
  { value: "cancelled", label: "Anuladas" },
] as const;

const ESTADO_BADGE: Record<string, { label: string; cls: string }> = {
  draft: { label: "Borrador", cls: "pill" },
  signed: { label: "Firmada", cls: "pill pill-ok" },
  sent: { label: "Enviada SII", cls: "pill pill-warn" },
  accepted: { label: "Aceptada", cls: "pill pill-ok" },
  rejected: { label: "Rechazada", cls: "pill pill-danger" },
  cancelled: { label: "Anulada", cls: "pill" },
};

export function renderBoletas(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-boletas">
      <div class="view-head">
        <div>
          <h2>Boletas electrónicas</h2>
          <p class="muted">Emisión, firma y envío al SII de boletas (DTE 39).</p>
        </div>
      </div>

      <div id="bol-caf" class="caf-banner muted">Consultando folios…</div>

      <div class="panel bol-emit">
        <h3 class="section-title">Emitir boleta de una venta</h3>
        <p class="muted">Ingresa el id de la orden pagada (lo muestra el POS al cobrar) y la clave del certificado digital. La boleta queda firmada localmente; el envío al SII es opcional.</p>
        <div class="bol-emit-form">
          <div class="field">
            <label for="bol-order">Orden POS</label>
            <input id="bol-order" type="text" placeholder="order:abc123…" autocomplete="off" />
          </div>
          <div class="field">
            <label for="bol-pass">Clave del certificado</label>
            <input id="bol-pass" type="password" placeholder="••••••••" autocomplete="off" />
          </div>
          <div class="field">
            <label for="bol-rut">RUT receptor (opcional)</label>
            <input id="bol-rut" type="text" placeholder="66666666-6 (consumidor final)" autocomplete="off" />
          </div>
          <button id="bol-emit-btn" class="btn-primary">Emitir y firmar</button>
        </div>
        <div id="bol-emit-status" class="cfg-status" hidden></div>
      </div>

      <div class="view-toolbar">
        <label class="muted" for="bol-estado">Estado</label>
        <select id="bol-estado">
          ${ESTADOS.map((e) => `<option value="${e.value}">${e.label}</option>`).join("")}
        </select>
        <button id="bol-refresh" class="btn-ghost">Actualizar</button>
      </div>

      <div id="bol-list">${tableSkeleton(6)}</div>
      <div id="bol-toast" class="toast" hidden></div>
    </section>
  `;

  const cafEl = host.querySelector<HTMLElement>("#bol-caf")!;
  const listEl = host.querySelector<HTMLElement>("#bol-list")!;
  const estadoSel = host.querySelector<HTMLSelectElement>("#bol-estado")!;
  const toastEl = host.querySelector<HTMLElement>("#bol-toast")!;

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  async function loadCaf(): Promise<void> {
    try {
      const s = await dteCafStatus(serverUrl);
      if (s.cafs.length === 0) {
        cafEl.innerHTML = `<span class="pill pill-danger">Sin CAF</span>
          <span>No hay folios autorizados cargados. Importa un CAF del SII con <code>pharma caf import</code> antes de emitir.</span>`;
        return;
      }
      const tone = s.folios_restantes <= 0 ? "pill-danger" : s.folios_restantes <= LOW_FOLIOS ? "pill-warn" : "pill-ok";
      cafEl.innerHTML = `<span class="pill ${tone}">${num(s.folios_restantes)} folios</span>
        <span>disponibles para boleta electrónica (tipo ${s.tipo}).</span>`;
    } catch (err) {
      cafEl.innerHTML = `<span class="muted">${escapeHtml(asMessage(err))}</span>`;
    }
  }

  async function loadList(): Promise<void> {
    listEl.innerHTML = tableSkeleton(6);
    try {
      const estado = estadoSel.value || undefined;
      const rows = await listDtes(serverUrl, { estado, limit: LIST_LIMIT });
      if (rows.length === 0) {
        listEl.innerHTML = `<p class="empty">No hay boletas${estado ? " en este estado" : " emitidas todavía"}.</p>`;
        return;
      }
      listEl.innerHTML = `
        <table class="data-table">
          <thead>
            <tr><th class="num">Folio</th><th>Fecha</th><th>Receptor</th><th class="num">Total</th><th>Estado</th><th>SII</th><th>Acciones</th></tr>
          </thead>
          <tbody>${rows.map(rowHtml).join("")}</tbody>
        </table>
      `;
      rows.forEach((d) => wireRow(d));
    } catch (err) {
      listEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function rowHtml(d: Dte): string {
    const badge = ESTADO_BADGE[d.estado] ?? { label: d.estado, cls: "pill" };
    const sii = d.track_id !== null
      ? `track ${d.track_id}${d.sii_glosa ? ` · ${escapeHtml(d.sii_glosa)}` : ""}`
      : "—";
    const k = cssKey(d.id);
    const actions: string[] = [];
    if (d.has_xml) {
      actions.push(`<button class="btn-ghost" id="bol-xml-${k}" title="Descargar XML firmado">XML</button>`);
    }
    if (d.estado === "signed") {
      actions.push(`<button class="btn-ghost" id="bol-send-${k}" title="Enviar al SII (requiere plan Pro)">Enviar SII</button>`);
      actions.push(`<button class="btn-ghost-danger" id="bol-cancel-${k}" title="Anular antes de enviar">Anular</button>`);
    }
    if (d.estado === "sent") {
      actions.push(`<button class="btn-ghost" id="bol-poll-${k}" title="Consultar veredicto del SII">Consultar</button>`);
    }
    return `
      <tr data-dte="${escapeHtml(d.id)}">
        <td class="num">${num(d.folio)}</td>
        <td>${fmtDate(d.fecha_emision)}</td>
        <td><div class="cell-main">${escapeHtml(d.razon_social_receptor)}</div><div class="muted">${escapeHtml(d.rut_receptor)}</div></td>
        <td class="num">${clp(d.monto_total)}</td>
        <td><span class="${badge.cls}">${badge.label}</span></td>
        <td class="muted">${sii}</td>
        <td class="bol-actions">${actions.join("") || "—"}</td>
      </tr>
    `;
  }

  function wireRow(d: Dte): void {
    const k = cssKey(d.id);
    listEl.querySelector<HTMLButtonElement>(`#bol-xml-${k}`)?.addEventListener("click", async (ev) => {
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        const xml = await dteXml(serverUrl, d.id);
        downloadXml(xml, `boleta-${d.folio}.xml`);
      } catch (err) {
        toast(asMessage(err));
      } finally {
        btn.disabled = false;
      }
    });

    listEl.querySelector<HTMLButtonElement>(`#bol-send-${k}`)?.addEventListener("click", async (ev) => {
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        await sendDte(serverUrl, d.id);
        toast(`Boleta ${num(d.folio)} enviada al SII`);
        await loadList();
      } catch (err) {
        const { code, message } = parseSaleError(err);
        toast(code === "FEATURE_REQUIRES_UPGRADE"
          ? `Plan Pro requerido para envío automático. ${message}`
          : message);
        btn.disabled = false;
      }
    });

    listEl.querySelector<HTMLButtonElement>(`#bol-poll-${k}`)?.addEventListener("click", async (ev) => {
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        const res = await pollDte(serverUrl, d.id);
        toast(`SII: ${res.sii_estado.replace(/_/g, " ")}`);
        await loadList();
      } catch (err) {
        toast(asMessage(err));
        btn.disabled = false;
      }
    });

    listEl.querySelector<HTMLButtonElement>(`#bol-cancel-${k}`)?.addEventListener("click", async (ev) => {
      const reason = window.prompt(`Motivo de anulación de la boleta ${num(d.folio)}:`);
      if (!reason || !reason.trim()) return;
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        await cancelDte(serverUrl, d.id, reason.trim());
        toast(`Boleta ${num(d.folio)} anulada`);
        await loadList();
      } catch (err) {
        toast(asMessage(err));
        btn.disabled = false;
      }
    });
  }

  // --- emit form -------------------------------------------------------------
  const orderInput = host.querySelector<HTMLInputElement>("#bol-order")!;
  const passInput = host.querySelector<HTMLInputElement>("#bol-pass")!;
  const rutInput = host.querySelector<HTMLInputElement>("#bol-rut")!;
  const emitBtn = host.querySelector<HTMLButtonElement>("#bol-emit-btn")!;
  const emitStatus = host.querySelector<HTMLElement>("#bol-emit-status")!;

  emitBtn.addEventListener("click", async () => {
    const orderId = orderInput.value.trim();
    const pass = passInput.value;
    emitStatus.hidden = true;
    if (!orderId) {
      showEmitError("Ingresa el id de la orden POS (ej. order:abc123).");
      return;
    }
    if (!pass) {
      showEmitError("Ingresa la clave del certificado digital.");
      return;
    }
    emitBtn.disabled = true;
    try {
      const dte = await emitBoleta(serverUrl, orderId, pass, rutInput.value.trim() || undefined);
      passInput.value = "";
      orderInput.value = "";
      emitStatus.textContent = `Boleta folio ${num(dte.folio)} emitida y firmada.`;
      emitStatus.className = "cfg-status cfg-status-ok";
      emitStatus.hidden = false;
      toast(`Boleta ${num(dte.folio)} firmada`);
      await Promise.all([loadList(), loadCaf()]);
    } catch (err) {
      const { message } = parseSaleError(err);
      showEmitError(message);
    } finally {
      emitBtn.disabled = false;
    }
  });

  function showEmitError(msg: string): void {
    emitStatus.textContent = msg;
    emitStatus.className = "cfg-status cfg-status-err";
    emitStatus.hidden = false;
  }

  estadoSel.addEventListener("change", () => void loadList());
  host.querySelector<HTMLButtonElement>("#bol-refresh")!.addEventListener("click", () => {
    void loadList();
    void loadCaf();
  });

  void loadCaf();
  void loadList();
}

/** Record ids (`dte:abc`) are not valid in CSS selectors — strip to the key. */
function cssKey(id: string): string {
  return id.replace(/[^a-zA-Z0-9_-]/g, "-");
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-CL", {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function downloadXml(xml: string, filename: string): void {
  const blob = new Blob([xml], { type: "application/xml" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
