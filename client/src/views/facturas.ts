// Facturas view — emisión de factura electrónica (33), nota de débito (56),
// nota de crédito (61) y guía de despacho (52) sobre POST /api/v1/dte/documentos
// (PR #125). Tres bloques:
//   • CAF banner   → /dte/caf-status?tipo=N (folios del tipo seleccionado)
//   • Emitir       → form receptor completo + items dinámicos + referencias
//                    (obligatorias en notas) + motivo traslado (guía)
//   • Listado      → /dte?tipo=N con acciones XML / Enviar SII / Consultar /
//                    Anular (mismo contrato que la vista Boletas)
// Mismo skeleton → fetch → swap de las demás vistas; español siempre.
import {
  listDtes,
  dteCafStatus,
  dteXml,
  emitDocumento,
  sendDte,
  pollDte,
  cancelDte,
  parseSaleError,
  type Dte,
  type DocItem,
  type DocReferencia,
} from "../api";
import { clp, num, isValidRut, canonicalRut, formatRut } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./view-blocks";
import { facturaTotals } from "./facturas-helpers";
import { emptyState, errorState } from "./ui";
import { dteCssKey, dteRowHtml } from "./dte-row";

const LIST_LIMIT = 100;
const LOW_FOLIOS = 20;

const TIPOS = [
  { value: 33, label: "Factura electrónica (33)" },
  { value: 61, label: "Nota de crédito (61)" },
  { value: 56, label: "Nota de débito (56)" },
  { value: 52, label: "Guía de despacho (52)" },
] as const;

const TRASLADOS = [
  { value: 1, label: "1 — Operación constituye venta" },
  { value: 2, label: "2 — Venta por efectuar" },
  { value: 3, label: "3 — Consignación" },
  { value: 4, label: "4 — Entrega gratuita" },
  { value: 5, label: "5 — Traslado interno" },
  { value: 6, label: "6 — Otros traslados no venta" },
  { value: 7, label: "7 — Guía de devolución" },
  { value: 8, label: "8 — Traslado para exportación" },
  { value: 9, label: "9 — Venta para exportación" },
] as const;

const COD_REF = [
  { value: 1, label: "1 — Anula documento" },
  { value: 2, label: "2 — Corrige texto" },
  { value: 3, label: "3 — Corrige montos" },
] as const;

interface ItemRow {
  nombre: string;
  cantidad: string;
  precio: string;
  exento: boolean;
}

export function renderFacturas(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-facturas">
      <div class="view-head">
        <div>
          <h2 class="rb-display">Facturas y otros documentos</h2>
          <p class="muted">Factura electrónica, notas de crédito/débito y guía de despacho (DTE 33/61/56/52).</p>
        </div>
      </div>

      <div id="fac-caf" class="caf-banner muted">Consultando folios…</div>

      <div class="panel fac-emit">
        <h3 class="section-title rb-display">Emitir documento</h3>
        <div class="bol-emit-form">
          <div class="field">
            <label for="fac-tipo">Tipo de documento</label>
            <select id="fac-tipo">
              ${TIPOS.map((t) => `<option value="${t.value}">${t.label}</option>`).join("")}
            </select>
          </div>
          <div class="field" id="fac-traslado-field" hidden>
            <label for="fac-traslado">Motivo del traslado</label>
            <select id="fac-traslado">
              ${TRASLADOS.map((t) => `<option value="${t.value}">${t.label}</option>`).join("")}
            </select>
          </div>
        </div>

        <h4 class="section-title rb-display">Cliente (receptor)</h4>
        <div class="bol-emit-form">
          <div class="field"><label for="fac-rut">RUT</label>
            <input id="fac-rut" type="text" placeholder="76123456-7" autocomplete="off" inputmode="text" />
            <div id="fac-rut-hint" class="field-hint" hidden></div></div>
          <div class="field"><label for="fac-rs">Razón social</label>
            <input id="fac-rs" type="text" placeholder="EMPRESA LTDA" autocomplete="off" /></div>
          <div class="field"><label for="fac-giro">Giro</label>
            <input id="fac-giro" type="text" placeholder="Comercio mayorista" autocomplete="off" /></div>
          <div class="field"><label for="fac-dir">Dirección</label>
            <input id="fac-dir" type="text" placeholder="Av. Siempre Viva 742" autocomplete="off" /></div>
          <div class="field"><label for="fac-comuna">Comuna</label>
            <input id="fac-comuna" type="text" placeholder="La Serena" autocomplete="off" /></div>
        </div>

        <h4 class="section-title rb-display">Items (precios con IVA incluido)</h4>
        <div id="fac-items"></div>
        <button id="fac-add-item" class="btn-ghost rb-btn ghost">+ Agregar item</button>
        <div id="fac-totals" class="fac-totals" hidden></div>

        <div id="fac-ref-block" hidden>
          <h4 class="section-title rb-display">Documento que corrige (referencia)</h4>
          <p class="muted">Las notas siempre referencian el documento original.</p>
          <div class="bol-emit-form">
            <div class="field"><label for="fac-ref-tipo">Tipo doc. original</label>
              <input id="fac-ref-tipo" type="text" placeholder="33" autocomplete="off" /></div>
            <div class="field"><label for="fac-ref-folio">Folio original</label>
              <input id="fac-ref-folio" type="text" placeholder="123" autocomplete="off" /></div>
            <div class="field"><label for="fac-ref-fecha">Fecha original</label>
              <input id="fac-ref-fecha" type="date" autocomplete="off" /></div>
            <div class="field"><label for="fac-ref-cod">Motivo</label>
              <select id="fac-ref-cod">
                ${COD_REF.map((c) => `<option value="${c.value}">${c.label}</option>`).join("")}
              </select></div>
            <div class="field"><label for="fac-ref-razon">Detalle (opcional)</label>
              <input id="fac-ref-razon" type="text" placeholder="ANULA FACTURA" autocomplete="off" /></div>
          </div>
        </div>

        <div class="bol-emit-form">
          <div class="field">
            <label for="fac-pass">Clave del certificado</label>
            <input id="fac-pass" type="password" placeholder="••••••••" autocomplete="off" />
          </div>
          <button id="fac-emit-btn" class="btn-primary rb-btn">Emitir y firmar</button>
        </div>
        <div id="fac-emit-status" class="cfg-status" hidden></div>
      </div>

      <div class="view-toolbar">
        <label class="muted" for="fac-list-tipo">Ver</label>
        <select id="fac-list-tipo">
          ${TIPOS.map((t) => `<option value="${t.value}">${t.label}</option>`).join("")}
        </select>
        <button id="fac-refresh" class="btn-ghost rb-btn ghost">Actualizar</button>
      </div>

      <div id="fac-list">${tableSkeleton(6)}</div>
      <div id="fac-toast" class="toast" hidden></div>
    </section>
  `;

  const cafEl = host.querySelector<HTMLElement>("#fac-caf")!;
  const tipoSel = host.querySelector<HTMLSelectElement>("#fac-tipo")!;
  const trasladoField = host.querySelector<HTMLElement>("#fac-traslado-field")!;
  const trasladoSel = host.querySelector<HTMLSelectElement>("#fac-traslado")!;
  const refBlock = host.querySelector<HTMLElement>("#fac-ref-block")!;
  const itemsHost = host.querySelector<HTMLElement>("#fac-items")!;
  const listEl = host.querySelector<HTMLElement>("#fac-list")!;
  const listTipoSel = host.querySelector<HTMLSelectElement>("#fac-list-tipo")!;
  const toastEl = host.querySelector<HTMLElement>("#fac-toast")!;
  const emitStatus = host.querySelector<HTMLElement>("#fac-emit-status")!;
  const rutInput = host.querySelector<HTMLInputElement>("#fac-rut")!;
  const rutHint = host.querySelector<HTMLElement>("#fac-rut-hint")!;
  const totalsEl = host.querySelector<HTMLElement>("#fac-totals")!;

  const items: ItemRow[] = [{ nombre: "", cantidad: "1", precio: "", exento: false }];

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  function selectedTipo(): number {
    return Number(tipoSel.value);
  }

  function syncTipoUi(): void {
    const t = selectedTipo();
    trasladoField.hidden = t !== 52;
    refBlock.hidden = t !== 56 && t !== 61;
  }

  function renderItems(): void {
    itemsHost.innerHTML = `
      <table class="data-table fac-items-table">
        <thead><tr><th>Descripción</th><th class="num">Cantidad</th><th class="num">Precio unit.</th><th>Exento</th><th></th></tr></thead>
        <tbody>
          ${items
            .map(
              (it, i) => `
            <tr data-i="${i}">
              <td><input data-f="nombre" data-i="${i}" type="text" value="${escapeHtml(it.nombre)}" placeholder="Ibuprofeno 400mg caja" /></td>
              <td class="num"><input data-f="cantidad" data-i="${i}" type="text" value="${escapeHtml(it.cantidad)}" class="fac-num" /></td>
              <td class="num"><input data-f="precio" data-i="${i}" type="text" value="${escapeHtml(it.precio)}" class="fac-num" placeholder="1190" /></td>
              <td><input data-f="exento" data-i="${i}" type="checkbox" ${it.exento ? "checked" : ""} /></td>
              <td>${items.length > 1 ? `<button class="btn-ghost-danger" data-del="${i}">×</button>` : ""}</td>
            </tr>`,
            )
            .join("")}
        </tbody>
      </table>
    `;
    itemsHost.querySelectorAll<HTMLInputElement>("input[data-f]").forEach((inp) => {
      inp.addEventListener("input", () => {
        const i = Number(inp.dataset.i);
        const f = inp.dataset.f as keyof ItemRow;
        if (f === "exento") items[i].exento = inp.checked;
        else items[i][f] = inp.value as never;
        renderTotals();
      });
    });
    itemsHost.querySelectorAll<HTMLButtonElement>("button[data-del]").forEach((btn) => {
      btn.addEventListener("click", () => {
        items.splice(Number(btn.dataset.del), 1);
        renderItems();
      });
    });
    renderTotals();
  }

  function renderTotals(): void {
    const t = facturaTotals(items);
    if (t.total <= 0) {
      totalsEl.hidden = true;
      totalsEl.innerHTML = "";
      return;
    }
    const exentoLine = t.exento > 0
      ? `<div class="rcpt-line"><span>Exento</span><strong>${clp(t.exento)}</strong></div>`
      : "";
    totalsEl.hidden = false;
    totalsEl.innerHTML = `
      <div class="rcpt-totals">
        <div class="rcpt-line"><span>Neto</span><strong>${clp(t.neto)}</strong></div>
        <div class="rcpt-line"><span>IVA 19%</span><strong>${clp(t.iva)}</strong></div>
        ${exentoLine}
        <div class="rcpt-line total"><span>Total</span><strong>${clp(t.total)}</strong></div>
      </div>
      <p class="fac-totals-note">Estimación con precios IVA-incluido; el SII recibe estos montos.</p>
    `;
  }

  /** Live mód-11 check on the receptor RUT. Returns validity; paints the input
   *  and the hint (green echo / red warning / hidden when empty). */
  function validateRut(): boolean {
    const raw = rutInput.value.trim();
    if (!raw) {
      rutInput.classList.remove("invalid");
      rutHint.hidden = true;
      rutHint.className = "field-hint";
      return false;
    }
    if (isValidRut(raw)) {
      rutInput.classList.remove("invalid");
      rutHint.hidden = false;
      rutHint.className = "field-hint ok";
      rutHint.textContent = `RUT válido — ${formatRut(raw)}`;
      return true;
    }
    rutInput.classList.add("invalid");
    rutHint.hidden = false;
    rutHint.className = "field-hint err";
    rutHint.textContent = "RUT inválido: revisa el dígito verificador.";
    return false;
  }

  async function loadCaf(): Promise<void> {
    const t = selectedTipo();
    cafEl.textContent = "Consultando folios…";
    try {
      const s = await dteCafStatus(serverUrl, t);
      if (s.cafs.length === 0) {
        cafEl.innerHTML = `<span class="pill pill-danger">Sin CAF tipo ${t}</span>
          <span>Importa folios del SII con <code>pharma caf import</code> antes de emitir este tipo.</span>`;
        return;
      }
      const tone = s.folios_restantes <= 0 ? "pill-danger" : s.folios_restantes <= LOW_FOLIOS ? "pill-warn" : "pill-ok";
      cafEl.innerHTML = `<span class="pill ${tone}">${num(s.folios_restantes)} folios</span>
        <span>disponibles para el tipo ${t}.</span>`;
    } catch (err) {
      cafEl.innerHTML = `<span class="muted">${escapeHtml(asMessage(err))}</span>`;
    }
  }

  async function loadList(): Promise<void> {
    listEl.innerHTML = tableSkeleton(6);
    try {
      const tipo = Number(listTipoSel.value);
      const rows = await listDtes(serverUrl, { tipo, limit: LIST_LIMIT });
      if (rows.length === 0) {
        listEl.innerHTML = emptyState({
          title: "No hay documentos de este tipo todavía",
          hint: "Emite una factura, nota de crédito/débito o guía con el formulario de arriba.",
        });
        return;
      }
      listEl.innerHTML = `
        <table class="data-table rb-table">
          <thead>
            <tr><th class="num">Folio</th><th>Fecha</th><th>Receptor</th><th class="num">Total</th><th>Estado</th><th>SII</th><th>Acciones</th></tr>
          </thead>
          <tbody>${rows.map((d) => dteRowHtml(d, { prefix: "fac", sendPlan: "Business" })).join("")}</tbody>
        </table>
      `;
      rows.forEach((d) => wireRow(d));
    } catch (err) {
      listEl.innerHTML = errorState(asMessage(err));
    }
  }

  function wireRow(d: Dte): void {
    const k = dteCssKey(d.id);
    listEl.querySelector<HTMLButtonElement>(`#fac-xml-${k}`)?.addEventListener("click", async (ev) => {
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        const xml = await dteXml(serverUrl, d.id);
        downloadXml(xml, `dte-${d.tipo}-${d.folio}.xml`);
      } catch (err) {
        toast(asMessage(err));
      } finally {
        btn.disabled = false;
      }
    });

    listEl.querySelector<HTMLButtonElement>(`#fac-send-${k}`)?.addEventListener("click", async (ev) => {
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        await sendDte(serverUrl, d.id);
        toast(`Documento ${num(d.folio)} enviado al SII`);
        await loadList();
      } catch (err) {
        const { code, message } = parseSaleError(err);
        toast(code === "FEATURE_REQUIRES_UPGRADE"
          ? `Plan Business requerido para envío automático. ${message}`
          : message);
        btn.disabled = false;
      }
    });

    listEl.querySelector<HTMLButtonElement>(`#fac-poll-${k}`)?.addEventListener("click", async (ev) => {
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

    listEl.querySelector<HTMLButtonElement>(`#fac-cancel-${k}`)?.addEventListener("click", async (ev) => {
      const reason = window.prompt(`Motivo de anulación del documento ${num(d.folio)}:`);
      if (!reason || !reason.trim()) return;
      const btn = ev.currentTarget as HTMLButtonElement;
      btn.disabled = true;
      try {
        await cancelDte(serverUrl, d.id, reason.trim());
        toast(`Documento ${num(d.folio)} anulado`);
        await loadList();
      } catch (err) {
        toast(asMessage(err));
        btn.disabled = false;
      }
    });
  }

  // --- emit ------------------------------------------------------------------
  const emitBtn = host.querySelector<HTMLButtonElement>("#fac-emit-btn")!;

  function showEmitStatus(msg: string, ok: boolean): void {
    emitStatus.textContent = msg;
    emitStatus.className = ok ? "cfg-status cfg-status-ok" : "cfg-status cfg-status-err";
    emitStatus.hidden = false;
  }

  emitBtn.addEventListener("click", async () => {
    emitStatus.hidden = true;
    const tipo = selectedTipo();
    const rutRaw = val("#fac-rut");
    const receptor = {
      rut: canonicalRut(rutRaw),
      razon_social: val("#fac-rs"),
      giro: val("#fac-giro"),
      direccion: val("#fac-dir"),
      comuna: val("#fac-comuna"),
    };
    if (Object.values(receptor).some((v) => !v)) {
      showEmitStatus("Completa todos los datos del cliente (RUT, razón social, giro, dirección, comuna).", false);
      return;
    }
    if (!isValidRut(rutRaw)) {
      validateRut();
      showEmitStatus("El RUT del cliente no es válido (revisa el dígito verificador mód-11).", false);
      return;
    }
    const docItems: DocItem[] = [];
    for (const [i, it] of items.entries()) {
      if (!it.nombre.trim()) {
        showEmitStatus(`Item ${i + 1}: falta la descripción.`, false);
        return;
      }
      if (!it.cantidad.trim() || Number.isNaN(Number(it.cantidad))) {
        showEmitStatus(`Item ${i + 1}: cantidad inválida.`, false);
        return;
      }
      if (!it.precio.trim() || Number.isNaN(Number(it.precio))) {
        showEmitStatus(`Item ${i + 1}: precio inválido.`, false);
        return;
      }
      docItems.push({
        nombre: it.nombre.trim(),
        cantidad: it.cantidad.trim(),
        precio_unitario: it.precio.trim(),
        exento: it.exento || undefined,
      });
    }
    let referencias: DocReferencia[] | undefined;
    if (tipo === 56 || tipo === 61) {
      const refTipo = val("#fac-ref-tipo");
      const refFolio = val("#fac-ref-folio");
      const refFecha = val("#fac-ref-fecha");
      if (!refTipo || !refFolio || !refFecha) {
        showEmitStatus("La nota requiere el documento original: tipo, folio y fecha.", false);
        return;
      }
      referencias = [{
        tipo_doc_ref: refTipo,
        folio_ref: refFolio,
        fecha_ref: refFecha,
        cod_ref: Number(host.querySelector<HTMLSelectElement>("#fac-ref-cod")!.value),
        razon_ref: val("#fac-ref-razon") || undefined,
      }];
    }
    const pass = host.querySelector<HTMLInputElement>("#fac-pass")!;
    if (!pass.value) {
      showEmitStatus("Ingresa la clave del certificado digital.", false);
      return;
    }
    emitBtn.disabled = true;
    try {
      const dte = await emitDocumento(serverUrl, {
        tipo,
        certPassphrase: pass.value,
        receptor,
        items: docItems,
        referencias,
        indTraslado: tipo === 52 ? Number(trasladoSel.value) : undefined,
      });
      pass.value = "";
      showEmitStatus(`Documento tipo ${dte.tipo} folio ${num(dte.folio)} emitido y firmado.`, true);
      toast(`Folio ${num(dte.folio)} firmado`);
      listTipoSel.value = String(tipo);
      await Promise.all([loadList(), loadCaf()]);
    } catch (err) {
      const { message } = parseSaleError(err);
      showEmitStatus(message, false);
    } finally {
      emitBtn.disabled = false;
    }
  });

  function val(sel: string): string {
    return host.querySelector<HTMLInputElement>(sel)!.value.trim();
  }

  host.querySelector<HTMLButtonElement>("#fac-add-item")!.addEventListener("click", () => {
    items.push({ nombre: "", cantidad: "1", precio: "", exento: false });
    renderItems();
  });
  tipoSel.addEventListener("change", () => {
    syncTipoUi();
    void loadCaf();
  });
  listTipoSel.addEventListener("change", () => void loadList());
  host.querySelector<HTMLButtonElement>("#fac-refresh")!.addEventListener("click", () => {
    void loadList();
    void loadCaf();
  });
  rutInput.addEventListener("input", validateRut);
  rutInput.addEventListener("blur", () => {
    if (isValidRut(rutInput.value)) rutInput.value = formatRut(rutInput.value);
  });

  syncTipoUi();
  renderItems();
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
