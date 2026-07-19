// Importar view — bulk CSV catalog load for admins. The operator picks a CSV,
// the file is read in the browser (no path round-trip), and the raw text is
// handed to the Tauri commands which POST it as multipart to
// POST /api/v1/products/import. Migration is two-step: a `dry_run` PREVIEW first
// (validate + count + per-row errors WITHOUT writing), then a CONFIRM that
// actually commits. The server upserts by `external_id` (idempotent: re-running
// the same file never duplicates), sniffs `;`/`,`/TAB delimiters, strips the
// Excel BOM, accepts Spanish headers (`nombre`/`precio`/`código`) and CLP
// thousands separators (`1.990` → 1990). Writes are admin+ server-side; a 403
// surfaces as the Spanish permission copy. Spanish throughout.
import {
  importProducts,
  importProductsPreview,
  exportProducts,
  type ImportSummary,
} from "../api";
import { asMessage, escapeHtml } from "./view-blocks";

// Columns the server understands (crates/api/src/v1/catalog.rs::import_products).
const REQUIRED = ["name", "price"];
const OPTIONAL = [
  "external_id",
  "barcode",
  "sale_price",
  "cost_price",
  "stock",
  "category",
  "laboratory",
  "active_ingredient",
  "therapeutic_action",
  "prescription_type",
  "presentation",
  "discount_percent",
  "image_url",
  "description",
  "slug",
];

export function renderImportar(host: HTMLElement, serverUrl: string): void {
  // DEFER (C3): column labels by pack vocab (Producto→Servicio, etc.) need a
  // server import header map for Spanish pack attrs (talla/color). Until then
  // keep the fixed column list below; the form alta already uses pack attrs.
  host.innerHTML = `
    <section class="view view-importar">
      <div class="view-head">
        <div>
          <h2>Importar / Exportar catálogo</h2>
          <p class="muted">Carga masiva del catálogo desde CSV (admin) o descarga todo el catálogo actual.</p>
        </div>
        <button id="imp-export" class="btn-ghost">Exportar catálogo CSV</button>
      </div>

      <div class="import-grid">
        <div class="import-panel">
          <label class="import-drop" id="imp-drop">
            <input id="imp-file" type="file" accept=".csv,text/csv" hidden />
            <span class="import-drop-mark">＋</span>
            <span class="import-drop-label">Elegir archivo CSV</span>
            <span class="muted import-file-name" id="imp-name">ningún archivo seleccionado</span>
          </label>
          <div id="imp-error" class="pos-error" hidden></div>
          <button id="imp-preview" class="btn-primary" disabled>
            <span class="btn-label">Previsualizar</span>
            <span class="btn-pulse"></span>
          </button>
          <button id="imp-go" class="btn-primary" hidden>
            <span class="btn-label">Confirmar importación</span>
            <span class="btn-pulse"></span>
          </button>
          <button id="imp-cancel" class="btn-ghost" hidden>Cancelar</button>
        </div>

        <div class="import-help">
          <h3>Formato</h3>
          <p class="muted">Primera fila = cabecera. Columnas obligatorias:</p>
          <p>${REQUIRED.map((c) => `<code>${c}</code>`).join(" ")} <span class="muted">(o <code>sale_price</code> en vez de <code>price</code>)</span></p>
          <p class="muted">Opcionales reconocidas:</p>
          <p class="import-cols">${OPTIONAL.map((c) => `<code>${escapeHtml(c)}</code>`).join(" ")}</p>
          <p class="muted import-note">Acepta cabeceras en <strong>español</strong> (<code>nombre</code>, <code>precio</code>, <code>código</code>, <code>existencia</code>…), separador <code>;</code> o <code>,</code> (Excel CL) y precios con punto de miles (<code>1.990</code> = 1990).</p>
          <p class="muted import-note">Si incluyes <code>external_id</code>, reimportar el mismo archivo <strong>actualiza</strong> (no duplica).</p>
          <p class="muted import-note">El CSV exportado usa estas mismas columnas: exporta → edita en Excel → reimporta.</p>
        </div>
      </div>

      <div id="imp-result"></div>
      <div id="imp-toast" class="toast" hidden></div>
    </section>
  `;

  const fileEl = host.querySelector<HTMLInputElement>("#imp-file")!;
  const nameEl = host.querySelector<HTMLElement>("#imp-name")!;
  const previewBtn = host.querySelector<HTMLButtonElement>("#imp-preview")!;
  const goBtn = host.querySelector<HTMLButtonElement>("#imp-go")!;
  const cancelBtn = host.querySelector<HTMLButtonElement>("#imp-cancel")!;
  const errEl = host.querySelector<HTMLElement>("#imp-error")!;
  const resultEl = host.querySelector<HTMLElement>("#imp-result")!;
  const dropEl = host.querySelector<HTMLElement>("#imp-drop")!;
  const toastEl = host.querySelector<HTMLElement>("#imp-toast")!;
  const exportBtn = host.querySelector<HTMLButtonElement>("#imp-export")!;

  let picked: File | null = null;
  // Text staged by a successful preview; the confirm step re-uses it verbatim so
  // we import exactly what was previewed (no re-read between preview & commit).
  let previewed: string | null = null;

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  // Drops back to the "choose file → preview" state, discarding any staged text.
  function resetToPreview(): void {
    previewed = null;
    goBtn.hidden = true;
    cancelBtn.hidden = true;
    previewBtn.hidden = false;
    previewBtn.disabled = !picked;
  }

  fileEl.addEventListener("change", () => {
    picked = fileEl.files?.[0] ?? null;
    nameEl.textContent = picked ? picked.name : "ningún archivo seleccionado";
    errEl.hidden = true;
    resultEl.innerHTML = "";
    dropEl.classList.toggle("has-file", !!picked);
    resetToPreview();
  });

  // Step 1 — dry-run preview: validate + count WITHOUT writing. Shows the same
  // summary table a real run produces, then reveals the Confirm button.
  previewBtn.addEventListener("click", async () => {
    if (!picked) return;
    errEl.hidden = true;
    previewBtn.disabled = true;
    previewBtn.classList.add("loading");
    try {
      const text = await picked.text();
      if (text.trim() === "") {
        throw new Error("El archivo está vacío.");
      }
      const summary = await importProductsPreview(serverUrl, text);
      renderSummary(resultEl, summary, true);
      previewed = text;
      previewBtn.hidden = true;
      goBtn.hidden = false;
      cancelBtn.hidden = false;
      const ok = summary.created + summary.updated;
      goBtn.disabled = ok === 0;
      toast(
        ok === 0
          ? "Sin filas válidas — revisa los errores"
          : `Vista previa · ${ok} fila(s) ok, ${summary.failed} con error`,
      );
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
    } finally {
      previewBtn.classList.remove("loading");
      if (!previewed) previewBtn.disabled = !picked;
    }
  });

  cancelBtn.addEventListener("click", () => {
    resultEl.innerHTML = "";
    errEl.hidden = true;
    resetToPreview();
  });

  // Step 2 — confirm: commit the previewed text for real.
  goBtn.addEventListener("click", async () => {
    if (!previewed) return;
    errEl.hidden = true;
    goBtn.disabled = true;
    goBtn.classList.add("loading");
    try {
      const summary = await importProducts(serverUrl, previewed);
      renderSummary(resultEl, summary, false);
      toast(`Importación lista · ${summary.created + summary.updated} fila(s) ok`);
      resetToPreview();
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
      goBtn.disabled = false;
    } finally {
      goBtn.classList.remove("loading");
    }
  });

  // Export the whole catalog as CSV → Blob download. Filename carries the date so
  // repeated exports don't clobber each other in the downloads folder.
  exportBtn.addEventListener("click", async () => {
    errEl.hidden = true;
    exportBtn.disabled = true;
    exportBtn.classList.add("loading");
    try {
      const csv = await exportProducts(serverUrl);
      const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `catalogo-${new Date().toISOString().slice(0, 10)}.csv`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      toast("Catálogo exportado");
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
    } finally {
      exportBtn.disabled = false;
      exportBtn.classList.remove("loading");
    }
  });
}

function renderSummary(
  host: HTMLElement,
  s: ImportSummary,
  preview: boolean,
): void {
  const ok = s.created + s.updated;
  const banner = preview
    ? `<p class="muted import-preview-banner">Vista previa — todavía no se guardó nada. Revisa los números y confirma para importar.</p>`
    : "";
  const errorsTable =
    s.errors.length > 0
      ? `
        <h3 class="import-err-title">Filas rechazadas (${s.errors.length})</h3>
        <table class="table import-err-table">
          <thead><tr><th>Línea</th><th>Motivo</th></tr></thead>
          <tbody>
            ${s.errors
              .map(
                (e) =>
                  `<tr><td>${e.line}</td><td>${escapeHtml(e.message)}</td></tr>`,
              )
              .join("")}
          </tbody>
        </table>`
      : `<p class="muted import-clean">Sin filas rechazadas.</p>`;

  host.innerHTML = `
    ${banner}
    <div class="import-summary">
      <div class="import-stat import-stat-ok">
        <strong>${s.created}</strong><span>creados</span>
      </div>
      <div class="import-stat import-stat-upd">
        <strong>${s.updated}</strong><span>actualizados</span>
      </div>
      <div class="import-stat ${s.failed > 0 ? "import-stat-err" : ""}">
        <strong>${s.failed}</strong><span>fallidos</span>
      </div>
      <div class="import-stat">
        <strong>${ok}</strong><span>total ok</span>
      </div>
    </div>
    ${errorsTable}
  `;
}
