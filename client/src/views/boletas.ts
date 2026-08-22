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
  dteLibroVentas,
  dteLibroVentasSigned,
  emitBoleta,
  sendDte,
  pollDte,
  cancelDte,
  parseSaleError,
  type Dte,
} from "../api";
import { clp, num } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./view-blocks";
import { emptyState, errorState } from "./ui";
import { dteCssKey, dteRowHtml } from "./dte-row";

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

export function renderBoletas(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-boletas">
      <div class="view-head">
        <div>
          <h2 class="rb-display">Boletas electrónicas</h2>
          <p class="muted">Emisión, firma y envío al SII de boletas (DTE 39).</p>
        </div>
      </div>

      <div id="bol-caf" class="caf-banner muted">Consultando folios…</div>

      <div class="panel bol-emit">
        <h3 class="section-title rb-display">Emitir boleta de una venta</h3>
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
          <button id="bol-emit-btn" class="btn-primary rb-btn">Emitir y firmar</button>
        </div>
        <div id="bol-emit-status" class="cfg-status" hidden></div>
      </div>

      <div class="panel bol-libro">
        <h3 class="section-title rb-display">Libro de ventas mensual</h3>
        <p class="muted">XML <code>LibroCompraVenta</code> con las boletas aceptadas del mes. Sin firma para revisión contable; con la clave del certificado se descarga firmado (EnvioLibro) listo para el portal SII.</p>
        <div class="bol-emit-form">
          <div class="field">
            <label for="bol-libro-period">Período</label>
            <input id="bol-libro-period" type="month" autocomplete="off" />
          </div>
          <div class="field">
            <label for="bol-libro-pass">Clave del certificado (sólo firmado)</label>
            <input id="bol-libro-pass" type="password" placeholder="••••••••" autocomplete="off" />
          </div>
          <button id="bol-libro-btn" class="btn-ghost rb-btn ghost">Descargar XML</button>
          <button id="bol-libro-signed-btn" class="btn-primary rb-btn">Descargar firmado</button>
        </div>
        <div id="bol-libro-status" class="cfg-status" hidden></div>
      </div>

      <div class="view-toolbar">
        <label class="muted" for="bol-estado">Estado</label>
        <select id="bol-estado">
          ${ESTADOS.map((e) => `<option value="${e.value}">${e.label}</option>`).join("")}
        </select>
        <button id="bol-refresh" class="btn-ghost rb-btn ghost">Actualizar</button>
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
        listEl.innerHTML = emptyState({
          title: estado ? "No hay boletas en este estado" : "Aún no emites boletas",
          hint: estado
            ? "Prueba con otro filtro de estado."
            : "Emite tu primera boleta electrónica desde una venta del POS.",
        });
        return;
      }
      listEl.innerHTML = `
        <table class="data-table rb-table">
          <thead>
            <tr><th class="num">Folio</th><th>Fecha</th><th>Receptor</th><th class="num">Total</th><th>Estado</th><th>SII</th><th>Acciones</th></tr>
          </thead>
          <tbody>${rows.map((d) => dteRowHtml(d, { prefix: "bol", sendPlan: "Pro" })).join("")}</tbody>
        </table>
      `;
      rows.forEach((d) => wireRow(d));
    } catch (err) {
      listEl.innerHTML = errorState(asMessage(err));
    }
  }

  function wireRow(d: Dte): void {
    const k = dteCssKey(d.id);
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

  // --- libro de ventas ---------------------------------------------------------
  const libroPeriod = host.querySelector<HTMLInputElement>("#bol-libro-period")!;
  const libroPass = host.querySelector<HTMLInputElement>("#bol-libro-pass")!;
  const libroBtn = host.querySelector<HTMLButtonElement>("#bol-libro-btn")!;
  const libroSignedBtn = host.querySelector<HTMLButtonElement>("#bol-libro-signed-btn")!;
  const libroStatus = host.querySelector<HTMLElement>("#bol-libro-status")!;
  // Default: current month (input type=month wants YYYY-MM).
  libroPeriod.value = new Date().toISOString().slice(0, 7);

  function showLibroStatus(msg: string, ok: boolean): void {
    libroStatus.textContent = msg;
    libroStatus.className = ok ? "cfg-status cfg-status-ok" : "cfg-status cfg-status-err";
    libroStatus.hidden = false;
  }

  async function downloadLibro(signed: boolean): Promise<void> {
    const period = libroPeriod.value.trim();
    libroStatus.hidden = true;
    if (!/^\d{4}-\d{2}$/.test(period)) {
      showLibroStatus("Selecciona el período (mes) del libro.", false);
      return;
    }
    if (signed && !libroPass.value) {
      showLibroStatus("Ingresa la clave del certificado para firmar el libro.", false);
      return;
    }
    const btn = signed ? libroSignedBtn : libroBtn;
    btn.disabled = true;
    try {
      const xml = signed
        ? await dteLibroVentasSigned(serverUrl, period, libroPass.value)
        : await dteLibroVentas(serverUrl, period);
      libroPass.value = "";
      downloadXml(xml, `libro-ventas-${period}${signed ? "-firmado" : ""}.xml`);
      showLibroStatus(
        signed
          ? `Libro ${period} firmado descargado.`
          : `Libro ${period} descargado (sin firma).`,
        true,
      );
    } catch (err) {
      showLibroStatus(asMessage(err), false);
    } finally {
      btn.disabled = false;
    }
  }

  libroBtn.addEventListener("click", () => void downloadLibro(false));
  libroSignedBtn.addEventListener("click", () => void downloadLibro(true));

  estadoSel.addEventListener("change", () => void loadList());
  host.querySelector<HTMLButtonElement>("#bol-refresh")!.addEventListener("click", () => {
    void loadList();
    void loadCaf();
  });

  void loadCaf();
  void loadList();
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
