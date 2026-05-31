// Importar view — bulk CSV catalog load for admins. The operator picks a CSV,
// the file is read in the browser (no path round-trip), and the raw text is
// handed to the `import_products` Tauri command which POSTs it as multipart to
// POST /api/v1/products/import. The server upserts by `external_id` (idempotent:
// re-running the same file never duplicates) and returns per-row counts +
// errors, which we render as a summary + a rejected-rows table. Writes are
// admin+ server-side; a 403 surfaces as the Spanish permission copy. Spanish
// throughout. Same skeleton/fetch/swap idiom as the other views.
import { importProducts, type ImportSummary } from "../api";
import { asMessage, escapeHtml } from "./inventory";

// Columns the server understands (crates/api/src/v1/catalog.rs::import_products).
const REQUIRED = ["name", "price"];
const OPTIONAL = [
  "external_id",
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
  host.innerHTML = `
    <section class="view view-importar">
      <div class="view-head">
        <div>
          <h2>Importar productos</h2>
          <p class="muted">Carga masiva del catálogo desde un archivo CSV. Sólo administradores.</p>
        </div>
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
          <button id="imp-go" class="btn-primary" disabled>
            <span class="btn-label">Importar</span>
            <span class="btn-pulse"></span>
          </button>
        </div>

        <div class="import-help">
          <h3>Formato</h3>
          <p class="muted">Primera fila = cabecera. Columnas obligatorias:</p>
          <p>${REQUIRED.map((c) => `<code>${c}</code>`).join(" ")} <span class="muted">(o <code>sale_price</code> en vez de <code>price</code>)</span></p>
          <p class="muted">Opcionales reconocidas:</p>
          <p class="import-cols">${OPTIONAL.map((c) => `<code>${escapeHtml(c)}</code>`).join(" ")}</p>
          <p class="muted import-note">Si incluyes <code>external_id</code>, reimportar el mismo archivo <strong>actualiza</strong> (no duplica).</p>
        </div>
      </div>

      <div id="imp-result"></div>
      <div id="imp-toast" class="toast" hidden></div>
    </section>
  `;

  const fileEl = host.querySelector<HTMLInputElement>("#imp-file")!;
  const nameEl = host.querySelector<HTMLElement>("#imp-name")!;
  const goBtn = host.querySelector<HTMLButtonElement>("#imp-go")!;
  const errEl = host.querySelector<HTMLElement>("#imp-error")!;
  const resultEl = host.querySelector<HTMLElement>("#imp-result")!;
  const dropEl = host.querySelector<HTMLElement>("#imp-drop")!;
  const toastEl = host.querySelector<HTMLElement>("#imp-toast")!;

  let picked: File | null = null;

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  fileEl.addEventListener("change", () => {
    picked = fileEl.files?.[0] ?? null;
    nameEl.textContent = picked ? picked.name : "ningún archivo seleccionado";
    goBtn.disabled = !picked;
    errEl.hidden = true;
    dropEl.classList.toggle("has-file", !!picked);
  });

  goBtn.addEventListener("click", async () => {
    if (!picked) return;
    errEl.hidden = true;
    goBtn.disabled = true;
    goBtn.classList.add("loading");
    try {
      const text = await picked.text();
      if (text.trim() === "") {
        throw new Error("El archivo está vacío.");
      }
      const summary = await importProducts(serverUrl, text);
      renderSummary(resultEl, summary);
      toast(`Importación lista · ${summary.created + summary.updated} fila(s) ok`);
    } catch (err) {
      errEl.textContent = asMessage(err);
      errEl.hidden = false;
    } finally {
      goBtn.disabled = false;
      goBtn.classList.remove("loading");
    }
  });
}

function renderSummary(host: HTMLElement, s: ImportSummary): void {
  const ok = s.created + s.updated;
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
