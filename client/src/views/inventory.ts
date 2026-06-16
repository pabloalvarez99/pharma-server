// Inventario view — core ERP, available on every tier. Two tabs:
//   • "Productos": KPI cards (count, low/out of stock, valuation) + a searchable
//     product table. Rows open a detail modal with two write actions —
//     "Ajustar stock" (POST /products/{id}/stock) and "Lotes" (GET/POST
//     /batches). A "+ Nuevo producto" button creates products (POST /products).
//   • "Próximos a vencer": GET /reports/near-expiry with a 30/60/90-día window,
//     caducados flagged. Expiry = money saved + legal (Ley caducados).
// Writes require admin+ server-side; a non-admin 403 surfaces as the Spanish
// "Permiso denegado…" copy (no role threading). Money crosses the wire as
// STRINGS (Decimal) and is formatted via ../format; same skeleton → fetch → swap
// pattern as the other views. Spanish throughout.
import {
  inventorySummary,
  listProducts,
  productDetail,
  adjustProductStock,
  createProduct,
  listBatches,
  createBatch,
  nearExpiry,
  stockRotation,
  getSetting,
  type Product,
  type ProductDetail,
  type Batch,
  type NearExpiryRow,
  type NewProductInput,
} from "../api";
import { clp, num, isValidRut, canonicalRut, formatRut } from "../format";
import {
  toRfc3339Noon,
  stockLevel,
  expiryStatus,
  pharmaFieldsVisible,
  validateStockAdjust,
  nearExpiryView,
  reorderSuggestion,
  reorderList,
  REORDER_TARGET,
  rotacionRows,
  buildInventoryExport,
  exportFilename,
  classifyFetchError,
  inventoryEmpty,
  capRows,
  LIST_RENDER_CAP,
  type EmptyCopy,
} from "./stock-helpers";

const PAGE_LIMIT = 60;
// Single-page cap the server enforces on `/products` (`limit.min(500)`). The
// in-app export pulls one capped page; a fuller catalog uses the CLI export.
const EXPORT_CAP = 500;
// Reposición scans the whole (server-capped) catalog so a low/out SKU on page 2
// isn't missed — the per-page PAGE_LIMIT 60 would hide it. Same 500 server cap.
const REORDER_SCAN = 500;
type Tab = "productos" | "vencimientos" | "reposicion" | "rotacion";

export function renderInventory(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    ${invStyles()}
    <section class="view view-inventory">
      <div class="view-head">
        <div>
          <h2>Inventario</h2>
          <p class="muted">Stock, lotes y vencimientos del catálogo.</p>
        </div>
        <div class="inv-head-actions">
          <div class="view-search" id="inv-search-wrap">
            <input id="inv-search" type="search" placeholder="Buscar producto…" autocomplete="off" />
          </div>
          <div class="inv-export-wrap" id="inv-export-wrap">
            <button id="inv-export-btn" class="btn-ghost inv-export-btn" type="button" aria-haspopup="menu" aria-expanded="false">Exportar ▾</button>
            <div id="inv-export-menu" class="inv-export-menu" role="menu" hidden>
              <button type="button" role="menuitem" data-fmt="csv">CSV (Excel)</button>
              <button type="button" role="menuitem" data-fmt="json">JSON</button>
            </div>
          </div>
          <button id="inv-new-btn" class="btn-primary inv-new-btn">
            <span class="btn-label">+ Nuevo producto</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>

      <div class="inv-tabs" role="tablist">
        <button class="inv-tab" data-tab="productos" role="tab" aria-selected="true">Productos</button>
        <button class="inv-tab" data-tab="vencimientos" role="tab" aria-selected="false">Próximos a vencer</button>
        <button class="inv-tab" data-tab="reposicion" role="tab" aria-selected="false">Reposición</button>
        <button class="inv-tab" data-tab="rotacion" role="tab" aria-selected="false">Rotación</button>
      </div>

      <div id="inv-kpis" class="kpi-grid">${kpiSkeleton()}</div>
      <div id="inv-panel"></div>

      <div id="inv-modal-host"></div>
      <div id="inv-toast" class="toast" hidden></div>
    </section>
  `;

  const kpiHost = host.querySelector<HTMLElement>("#inv-kpis")!;
  const panel = host.querySelector<HTMLElement>("#inv-panel")!;
  const searchWrap = host.querySelector<HTMLElement>("#inv-search-wrap")!;
  const searchEl = host.querySelector<HTMLInputElement>("#inv-search")!;
  const newBtn = host.querySelector<HTMLButtonElement>("#inv-new-btn")!;
  const modalHost = host.querySelector<HTMLElement>("#inv-modal-host")!;
  const toastEl = host.querySelector<HTMLElement>("#inv-toast")!;

  const toast = (msg: string): void => showToast(toastEl, msg);

  let tab: Tab = "productos";

  // Multi-rubro: pharmacy-only product fields (laboratorio, principio activo)
  // are shown by default and hidden when the tenant runs a non-pharmacy vertical
  // (minimarket / general store). Read once; tolerate a 403/404/null by keeping
  // the pharmacy default (back-compat). Lote/vencimiento stay for everyone —
  // perishables (pan, leche) need them as much as fármacos do.
  let showPharma = true;
  void getSetting(serverUrl, "business_vertical")
    .then((s) => {
      showPharma = pharmaFieldsVisible(s?.value ?? null);
    })
    .catch(() => {
      /* keep pharmacy default when the setting can't be read */
    });

  function openDetail(id: string): void {
    void openProductDetail(modalHost, serverUrl, id, toast, refreshProductos);
  }

  function refreshProductos(): void {
    if (tab !== "productos") return;
    void loadKpis(kpiHost, serverUrl);
    const table = panel.querySelector<HTMLElement>("#inv-table");
    if (table) void loadProducts(table, serverUrl, searchEl.value.trim(), openDetail);
  }

  function selectTab(next: Tab): void {
    tab = next;
    host.querySelectorAll<HTMLButtonElement>(".inv-tab").forEach((b) =>
      b.setAttribute("aria-selected", String(b.dataset.tab === next)),
    );
    const productos = next === "productos";
    kpiHost.hidden = !productos;
    searchWrap.hidden = !productos;
    if (productos) {
      panel.innerHTML = `<div class="table-card"><div id="inv-table">${tableSkeleton()}</div></div>`;
      void loadProducts(
        panel.querySelector<HTMLElement>("#inv-table")!,
        serverUrl,
        searchEl.value.trim(),
        openDetail,
      );
    } else if (next === "vencimientos") {
      renderVencimientos(panel, serverUrl, openDetail);
    } else if (next === "reposicion") {
      renderReposicion(panel, serverUrl, openDetail);
    } else {
      renderRotacion(panel, serverUrl, openDetail);
    }
  }

  host.querySelectorAll<HTMLButtonElement>(".inv-tab").forEach((b) =>
    b.addEventListener("click", () => selectTab(b.dataset.tab as Tab)),
  );

  newBtn.addEventListener("click", () =>
    openNewProduct(
      modalHost,
      serverUrl,
      (created) => {
        toast(`Producto creado: ${created.name}`);
        refreshProductos();
      },
      showPharma,
    ),
  );

  // Export menu (CSV / JSON) — vendor-agnostic: the owner can take their data
  // out at any time, no proprietary format, no lock-in.
  const exportWrap = host.querySelector<HTMLElement>("#inv-export-wrap")!;
  const exportBtn = host.querySelector<HTMLButtonElement>("#inv-export-btn")!;
  const exportMenu = host.querySelector<HTMLElement>("#inv-export-menu")!;
  const closeExportMenu = (): void => {
    exportMenu.hidden = true;
    exportBtn.setAttribute("aria-expanded", "false");
  };
  exportBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    const open = exportMenu.hidden;
    exportMenu.hidden = !open;
    exportBtn.setAttribute("aria-expanded", String(open));
  });
  document.addEventListener("click", (e) => {
    if (!exportWrap.contains(e.target as Node)) closeExportMenu();
  });
  exportMenu.querySelectorAll<HTMLButtonElement>("button[data-fmt]").forEach((b) =>
    b.addEventListener("click", () => {
      closeExportMenu();
      void runInventoryExport(serverUrl, b.dataset.fmt === "json" ? "json" : "csv", showPharma, toast);
    }),
  );

  // Initial paint: KPIs + Productos tab.
  void loadKpis(kpiHost, serverUrl);
  selectTab("productos");

  // Debounced server-side search (Productos tab only).
  let timer: number | undefined;
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      const table = panel.querySelector<HTMLElement>("#inv-table");
      if (table) {
        table.innerHTML = tableSkeleton();
        void loadProducts(table, serverUrl, searchEl.value.trim(), openDetail);
      }
    }, 220);
  });
}

async function loadKpis(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const s = await inventorySummary(serverUrl);
    host.innerHTML = [
      kpiCard("Productos", num(s.total), `${num(s.active)} activos`),
      kpiCard("Stock bajo", num(s.low_stock), "bajo el mínimo", s.low_stock > 0 ? "warn" : ""),
      kpiCard("Sin stock", num(s.out_of_stock), "agotados", s.out_of_stock > 0 ? "danger" : ""),
      kpiCard("Valorización", clp(s.inventory_value), "a precio de venta"),
    ].join("");
  } catch (err) {
    host.innerHTML = errorCard(err);
  }
}

async function loadProducts(
  host: HTMLElement,
  serverUrl: string,
  search: string,
  onOpen: (id: string) => void,
): Promise<void> {
  try {
    const rows: Product[] = await listProducts(serverUrl, search || undefined, PAGE_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = emptyStateHtml(inventoryEmpty(search !== ""), search ? undefined : "inv-empty-new");
      host
        .querySelector<HTMLButtonElement>("#inv-empty-new")
        ?.addEventListener("click", () =>
          host.closest(".view-inventory")?.querySelector<HTMLButtonElement>("#inv-new-btn")?.click(),
        );
      return;
    }
    host.innerHTML = `
      <table class="data-table inv-products">
        <thead>
          <tr><th>Producto</th><th class="num">Precio</th><th class="num">Stock</th><th>Estado</th></tr>
        </thead>
        <tbody>
          ${rows.map(productRow).join("")}
        </tbody>
      </table>
      <p class="table-foot muted">${rows.length} producto(s)${
        rows.length === PAGE_LIMIT ? ` · mostrando los primeros ${PAGE_LIMIT}` : ""
      } · toca una fila para detalle, ajustar stock y lotes</p>
    `;
    host.querySelectorAll<HTMLElement>("tr[data-id]").forEach((tr) =>
      tr.addEventListener("click", () => onOpen(tr.dataset.id!)),
    );
  } catch (err) {
    host.innerHTML = errorStateHtml(err, "el inventario");
  }
}

/** Pull the (capped) product catalog and hand the operator a CSV or JSON file.
 *  No proprietary format, money as the raw Decimal string → re-imports cleanly. */
async function runInventoryExport(
  serverUrl: string,
  fmt: "csv" | "json",
  includePharma: boolean,
  toast: (msg: string) => void,
): Promise<void> {
  try {
    const products = await listProducts(serverUrl, undefined, EXPORT_CAP);
    if (products.length === 0) {
      toast("No hay productos para exportar.");
      return;
    }
    const bundle = buildInventoryExport(products, includePharma, EXPORT_CAP);
    const stem = exportFilename("inventario");
    if (fmt === "json") {
      downloadExport(`${stem}.json`, "application/json;charset=utf-8", bundle.json);
    } else {
      // Prepend a UTF-8 BOM so Excel (es-CL) reads tildes/ñ correctly.
      downloadExport(`${stem}.csv`, "text/csv;charset=utf-8", `﻿${bundle.csv}`);
    }
    toast(
      bundle.truncated
        ? `Exportados ${bundle.count} productos (máx. por página). Usa la CLI para el catálogo completo.`
        : `Exportados ${bundle.count} producto(s).`,
    );
  } catch (err) {
    toast(classifyFetchError(err, "el inventario").title);
  }
}

/** Trigger a client-side file download (mirrors boletas.ts `downloadXml`). */
function downloadExport(filename: string, mime: string, content: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function productRow(p: Product): string {
  const sub = p.laboratory || p.active_ingredient || "";
  // Min-stock signal: when a SKU is low/out, surface a concrete reorder hint so
  // the operator knows not just THAT it ran low but HOW MUCH to buy back.
  const reorder =
    stockLevel(p.stock) !== "ok"
      ? `<div class="cell-sub inv-reorder">Reponer ${num(reorderSuggestion(p.stock))} u.</div>`
      : "";
  return `
    <tr data-id="${escapeHtml(p.id)}" class="inv-row" tabindex="0">
      <td>
        <div class="cell-main">${escapeHtml(p.name)}</div>
        ${sub ? `<div class="cell-sub muted">${escapeHtml(sub)}</div>` : ""}
      </td>
      <td class="num">${clp(p.price)}</td>
      <td class="num">${num(p.stock)}</td>
      <td>${stockPill(p.stock)}${reorder}</td>
    </tr>
  `;
}

// --- "Nuevo producto" modal ------------------------------------------------

function openNewProduct(
  modalHost: HTMLElement,
  serverUrl: string,
  onDone: (created: ProductDetail) => void,
  showPharma = true,
): void {
  // Pharmacy-only fields (laboratorio, principio activo) — rendered only for a
  // pharmacy vertical. Presentación stays for everyone (generic: "500ml", "pack
  // x6"). When hidden the fields are simply omitted → sent undefined → null.
  const pharmaRow = showPharma
    ? `
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Principio activo</span>
            <input id="np-ingredient" type="text" autocomplete="off" placeholder="opcional" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">Presentación</span>
            <input id="np-presentation" type="text" autocomplete="off" placeholder="opcional" />
          </label>
        </div>`
    : `
        <label class="field modal-field">
          <span class="modal-label">Presentación</span>
          <input id="np-presentation" type="text" autocomplete="off" placeholder="opcional" />
        </label>`;
  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal inv-modal-wide" role="dialog" aria-modal="true" aria-label="Nuevo producto">
        <h3 class="modal-title">Nuevo producto</h3>
        <label class="field modal-field">
          <span class="modal-label">Nombre *</span>
          <input id="np-name" type="text" autocomplete="off" placeholder="Paracetamol 500mg" />
        </label>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Precio venta (CLP) *</span>
            <input id="np-price" type="number" inputmode="numeric" min="0" step="1" placeholder="0" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">Costo (CLP)</span>
            <input id="np-cost" type="number" inputmode="numeric" min="0" step="1" placeholder="opcional" />
          </label>
        </div>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Stock inicial</span>
            <input id="np-stock" type="number" inputmode="numeric" min="0" step="1" placeholder="0" />
          </label>
          ${
            showPharma
              ? `<label class="field modal-field">
            <span class="modal-label">Laboratorio</span>
            <input id="np-lab" type="text" autocomplete="off" placeholder="opcional" />
          </label>`
              : ""
          }
        </div>
        ${pharmaRow}
        <div id="np-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button id="np-cancel" class="btn-ghost">Cancelar</button>
          <button id="np-confirm" class="btn-primary modal-confirm">
            <span class="btn-label">Crear producto</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = (): void => {
    modalHost.innerHTML = "";
  };
  const nameEl = modalHost.querySelector<HTMLInputElement>("#np-name")!;
  const priceEl = modalHost.querySelector<HTMLInputElement>("#np-price")!;
  const costEl = modalHost.querySelector<HTMLInputElement>("#np-cost")!;
  const stockEl = modalHost.querySelector<HTMLInputElement>("#np-stock")!;
  // Pharma-only fields are absent in a non-pharmacy vertical → may be null.
  const labEl = modalHost.querySelector<HTMLInputElement>("#np-lab");
  const ingEl = modalHost.querySelector<HTMLInputElement>("#np-ingredient");
  const presEl = modalHost.querySelector<HTMLInputElement>("#np-presentation")!;
  const errEl = modalHost.querySelector<HTMLElement>("#np-error")!;
  const confirmBtn = modalHost.querySelector<HTMLButtonElement>("#np-confirm")!;

  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#np-cancel")!.addEventListener("click", close);
  nameEl.focus();

  confirmBtn.addEventListener("click", async () => {
    const name = nameEl.value.trim();
    const priceRaw = priceEl.value.trim();
    const price = Number(priceRaw);
    if (name === "") {
      showErr(errEl, "Ingresa el nombre del producto.");
      return;
    }
    if (priceRaw === "" || !Number.isFinite(price) || price < 0) {
      showErr(errEl, "Ingresa un precio de venta válido (0 o más).");
      return;
    }
    const input: NewProductInput = {
      name,
      price: String(Math.trunc(price)),
      costPrice: intStrOrUndef(costEl.value),
      stock: intOrUndef(stockEl.value),
      laboratory: labEl ? trimOrUndef(labEl.value) : undefined,
      activeIngredient: ingEl ? trimOrUndef(ingEl.value) : undefined,
      presentation: trimOrUndef(presEl.value),
    };
    errEl.hidden = true;
    busy(confirmBtn, true);
    try {
      const created = await createProduct(serverUrl, input);
      close();
      onDone(created);
    } catch (err) {
      showErr(errEl, asMessage(err));
      busy(confirmBtn, false);
    }
  });
}

// --- product detail modal: detail + ajustar stock + lotes ------------------

async function openProductDetail(
  modalHost: HTMLElement,
  serverUrl: string,
  id: string,
  toast: (msg: string) => void,
  onChanged: () => void,
): Promise<void> {
  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal inv-modal-wide" role="dialog" aria-modal="true" aria-label="Detalle de producto">
        <button id="pd-close" class="inv-modal-x" aria-label="Cerrar">×</button>
        <div id="pd-body">${detailSkeleton()}</div>
      </div>
    </div>
  `;
  const close = (): void => {
    modalHost.innerHTML = "";
  };
  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#pd-close")!.addEventListener("click", close);
  const bodyEl = modalHost.querySelector<HTMLElement>("#pd-body")!;

  let p: ProductDetail;
  try {
    p = await productDetail(serverUrl, id);
  } catch (err) {
    bodyEl.innerHTML = errorStateHtml(err);
    return;
  }

  function paintDetail(): void {
    const sub = [p.laboratory, p.active_ingredient, p.presentation].filter(Boolean).join(" · ");
    bodyEl.innerHTML = `
      <h3 class="modal-title">${escapeHtml(p.name)}</h3>
      ${sub ? `<p class="muted pd-sub">${escapeHtml(sub)}</p>` : ""}
      <div class="pd-grid">
        ${pdStat("Precio venta", clp(p.price))}
        ${pdStat("Costo", p.cost_price ? clp(p.cost_price) : "—")}
        ${pdStat("Stock", `${num(p.stock)} ${stockPill(p.stock)}`)}
      </div>

      <div class="pd-section">
        <div class="pd-section-head"><h4>Ajustar stock</h4></div>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Modo</span>
            <select id="pd-mode">
              <option value="delta">Sumar / restar</option>
              <option value="set">Fijar a</option>
            </select>
          </label>
          <label class="field modal-field">
            <span class="modal-label" id="pd-qty-label">Cantidad (+/−)</span>
            <input id="pd-qty" type="number" inputmode="numeric" step="1" placeholder="0" />
          </label>
        </div>
        <label class="field modal-field">
          <span class="modal-label">Motivo</span>
          <input id="pd-reason" type="text" autocomplete="off" placeholder="recuento, merma, ingreso…" />
        </label>
        <div id="pd-adj-error" class="pos-error" hidden></div>
        <button id="pd-adj-btn" class="btn-primary modal-confirm pd-adj-btn">
          <span class="btn-label">Aplicar ajuste</span>
          <span class="btn-pulse"></span>
        </button>
      </div>

      <div class="pd-section">
        <div class="pd-section-head">
          <h4>Lotes y vencimientos</h4>
          <button id="pd-add-lote" class="btn-ghost pd-add-lote">+ Agregar lote</button>
        </div>
        <div id="pd-lotes">${tableSkeleton(3)}</div>
        <div id="pd-lote-form"></div>
      </div>
    `;
    wireAdjust();
    bodyEl
      .querySelector<HTMLButtonElement>("#pd-add-lote")!
      .addEventListener("click", toggleLoteForm);
    void loadLotes();
  }

  function wireAdjust(): void {
    const modeEl = bodyEl.querySelector<HTMLSelectElement>("#pd-mode")!;
    const qtyLabel = bodyEl.querySelector<HTMLElement>("#pd-qty-label")!;
    const qtyEl = bodyEl.querySelector<HTMLInputElement>("#pd-qty")!;
    const reasonEl = bodyEl.querySelector<HTMLInputElement>("#pd-reason")!;
    const errEl = bodyEl.querySelector<HTMLElement>("#pd-adj-error")!;
    const btn = bodyEl.querySelector<HTMLButtonElement>("#pd-adj-btn")!;

    modeEl.addEventListener("change", () => {
      qtyLabel.textContent = modeEl.value === "set" ? "Nuevo stock" : "Cantidad (+/−)";
      qtyEl.min = modeEl.value === "set" ? "0" : "";
    });

    btn.addEventListener("click", async () => {
      const raw = qtyEl.value.trim();
      const n = Number(raw);
      const adjErr = raw === "" ? "Ingresa una cantidad entera." : validateStockAdjust(modeEl.value as "delta" | "set", n);
      if (adjErr) {
        showErr(errEl, adjErr);
        return;
      }
      const reason = reasonEl.value.trim() || undefined;
      errEl.hidden = true;
      busy(btn, true);
      try {
        p =
          modeEl.value === "set"
            ? await adjustProductStock(serverUrl, id, { set: n, reason })
            : await adjustProductStock(serverUrl, id, { delta: n, reason });
        toast(`Stock actualizado: ${num(p.stock)}`);
        onChanged();
        paintDetail();
      } catch (err) {
        showErr(errEl, asMessage(err));
        busy(btn, false);
      }
    });
  }

  async function loadLotes(): Promise<void> {
    const host = bodyEl.querySelector<HTMLElement>("#pd-lotes")!;
    try {
      const lotes = await listBatches(serverUrl, id, undefined, false);
      if (lotes.length === 0) {
        host.innerHTML = `<p class="empty pd-empty">Sin lotes registrados para este producto.</p>`;
        return;
      }
      host.innerHTML = `
        <table class="data-table pd-lote-table">
          <thead><tr><th>Lote</th><th>Vence</th><th class="num">Stock</th><th>Estado</th></tr></thead>
          <tbody>${lotes.map(loteRow).join("")}</tbody>
        </table>
      `;
    } catch (err) {
      host.innerHTML = errorStateHtml(err);
    }
  }

  function toggleLoteForm(): void {
    const host = bodyEl.querySelector<HTMLElement>("#pd-lote-form")!;
    if (host.innerHTML !== "") {
      host.innerHTML = "";
      return;
    }
    host.innerHTML = `
      <div class="pd-lote-form-inner">
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Código de lote *</span>
            <input id="lf-code" type="text" autocomplete="off" placeholder="L-2026-001" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">Vencimiento *</span>
            <input id="lf-expiry" type="date" />
          </label>
        </div>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Stock del lote</span>
            <input id="lf-stock" type="number" inputmode="numeric" min="0" step="1" placeholder="0" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">Costo unitario (CLP)</span>
            <input id="lf-cost" type="number" inputmode="numeric" min="0" step="1" placeholder="opcional" />
          </label>
        </div>
        <label class="field modal-field">
          <span class="modal-label">Notas</span>
          <input id="lf-notes" type="text" autocomplete="off" placeholder="opcional" />
        </label>
        <div id="lf-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button id="lf-confirm" class="btn-primary modal-confirm">
            <span class="btn-label">Guardar lote</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    `;
    const codeEl = host.querySelector<HTMLInputElement>("#lf-code")!;
    const expEl = host.querySelector<HTMLInputElement>("#lf-expiry")!;
    const stockEl = host.querySelector<HTMLInputElement>("#lf-stock")!;
    const costEl = host.querySelector<HTMLInputElement>("#lf-cost")!;
    const notesEl = host.querySelector<HTMLInputElement>("#lf-notes")!;
    const errEl = host.querySelector<HTMLElement>("#lf-error")!;
    const btn = host.querySelector<HTMLButtonElement>("#lf-confirm")!;
    codeEl.focus();

    btn.addEventListener("click", async () => {
      const code = codeEl.value.trim();
      const date = expEl.value; // YYYY-MM-DD
      if (code === "") {
        showErr(errEl, "Ingresa el código de lote.");
        return;
      }
      if (!date) {
        showErr(errEl, "Selecciona la fecha de vencimiento.");
        return;
      }
      errEl.hidden = true;
      busy(btn, true);
      try {
        // Noon-UTC anchor: a midnight anchor renders one day early in CL's TZ
        // (es-CL `toLocaleDateString`) — a real expiry-date slip. `date` is
        // validated non-empty just above, so the helper never returns undefined.
        await createBatch(serverUrl, id, code, toRfc3339Noon(date)!, {
          stock: intOrUndef(stockEl.value),
          cost: intStrOrUndef(costEl.value),
          notes: trimOrUndef(notesEl.value),
        });
        toast(`Lote ${code} agregado`);
        // An initial batch stock emits a movement → product stock changed.
        try {
          p = await productDetail(serverUrl, id);
        } catch {
          /* keep previous detail if the re-fetch fails */
        }
        onChanged();
        paintDetail();
      } catch (err) {
        showErr(errEl, asMessage(err));
        busy(btn, false);
      }
    });
  }

  paintDetail();
}

function loteRow(b: Batch): string {
  return `
    <tr>
      <td>${escapeHtml(b.batch_code)}${
        b.notes ? `<div class="cell-sub muted">${escapeHtml(b.notes)}</div>` : ""
      }</td>
      <td>${fmtDate(b.expiry_date)}</td>
      <td class="num">${num(b.stock)}</td>
      <td>${expiryPill(b.expiry_date)}</td>
    </tr>
  `;
}

// --- "Próximos a vencer" tab ------------------------------------------------

function renderVencimientos(
  panel: HTMLElement,
  serverUrl: string,
  onOpen: (id: string) => void,
): void {
  panel.innerHTML = `
    <div class="inv-venc-head">
      <span class="muted">Ventana:</span>
      <div class="inv-chips" role="group" aria-label="Días de anticipación">
        <button class="inv-chip" data-days="30" aria-pressed="true">30 días</button>
        <button class="inv-chip" data-days="60" aria-pressed="false">60 días</button>
        <button class="inv-chip" data-days="90" aria-pressed="false">90 días</button>
      </div>
    </div>
    <div class="table-card"><div id="inv-venc-table">${tableSkeleton()}</div></div>
  `;
  const tableHost = panel.querySelector<HTMLElement>("#inv-venc-table")!;
  let days = 30;

  const load = async (): Promise<void> => {
    tableHost.innerHTML = tableSkeleton();
    try {
      const raw = await nearExpiry(serverUrl, days);
      if (raw.length === 0) {
        tableHost.innerHTML = `<p class="empty">Sin lotes próximos a vencer en ${days} días. 👍</p>`;
        return;
      }
      // FEFO surface order: the operator must see the most urgent lote first
      // (caducados, luego el que vence antes), no matter how the feed arrives.
      // The feed is UNBOUNDED (no server limit) → cap the rendered rows so a
      // 50k-SKU catalog can't paint tens of thousands of <tr> in one frame.
      // Top-N after FEFO ordering = exactly the lotes the operator must act on.
      const ordered = nearExpiryView(raw);
      const { rows, total, truncated } = capRows(ordered);
      tableHost.innerHTML = `
        <table class="data-table inv-venc">
          <thead><tr>
            <th>Producto</th><th>Lote</th><th>Vence</th>
            <th class="num">Stock</th><th class="num">Días</th><th>Estado</th>
          </tr></thead>
          <tbody>${rows.map(nearRow).join("")}</tbody>
        </table>
        <p class="table-foot muted">${
          truncated
            ? `${rows.length} de ${total} lotes · mostrando los ${LIST_RENDER_CAP} más urgentes`
            : `${total} lote(s)`
        } · toca una fila para ver el producto</p>
      `;
      tableHost.querySelectorAll<HTMLElement>("tr[data-id]").forEach((tr) =>
        tr.addEventListener("click", () => onOpen(tr.dataset.id!)),
      );
    } catch (err) {
      tableHost.innerHTML = errorStateHtml(err);
    }
  };

  panel.querySelectorAll<HTMLButtonElement>(".inv-chip").forEach((chip) =>
    chip.addEventListener("click", () => {
      days = Number(chip.dataset.days);
      panel.querySelectorAll<HTMLButtonElement>(".inv-chip").forEach((b) =>
        b.setAttribute("aria-pressed", String(b === chip)),
      );
      void load();
    }),
  );
  void load();
}

function nearRow(r: NearExpiryRow & { tone: "danger" | "warn" | "ok"; label: string }): string {
  return `
    <tr data-id="${escapeHtml(r.product_id)}" class="inv-row" tabindex="0">
      <td>${escapeHtml(r.product_name)}</td>
      <td>${escapeHtml(r.batch_code)}</td>
      <td>${fmtDate(r.expiry_date)}</td>
      <td class="num">${num(r.stock)}</td>
      <td class="num">${r.days_to_expiry}</td>
      <td><span class="pill pill-${r.tone}">${r.label}</span></td>
    </tr>
  `;
}

// --- "Reposición" tab (min-stock reorder worklist) --------------------------

/** Actionable buy-back worklist: every SKU at/below the low-stock threshold,
 *  ordered by urgency (agotado first, then lowest stock), each with the units to
 *  buy to reach the target. Healthy SKUs are excluded — zero false alarms. The
 *  Productos tab shows the per-row "Reponer N" hint inline; THIS tab pulls them
 *  all into one prioritised list the operator works top-down. Vertical-agnostic:
 *  fármacos bajo mínimo y abarrotes agotados (pan, bebidas) salen igual. */
function renderReposicion(
  panel: HTMLElement,
  serverUrl: string,
  onOpen: (id: string) => void,
): void {
  panel.innerHTML = `
    <div class="inv-venc-head">
      <span class="muted">Productos bajo el mínimo, ordenados por urgencia (agotados primero). "Reponer N u." = cuánto comprar para volver al objetivo (${REORDER_TARGET} u.).</span>
    </div>
    <div class="table-card"><div id="inv-repos-table">${tableSkeleton()}</div></div>
  `;
  const tableHost = panel.querySelector<HTMLElement>("#inv-repos-table")!;
  void (async (): Promise<void> => {
    try {
      // Scan the full (server-capped) catalog, not just one PAGE_LIMIT page, so a
      // low/out SKU isn't missed. reorderList drops the healthy ones and orders
      // agotado→menor; the worklist is short by construction, but cap defensively
      // (a catalog that's mostly out of stock could still be large).
      const products = await listProducts(serverUrl, undefined, REORDER_SCAN);
      const work = reorderList(products);
      if (work.length === 0) {
        tableHost.innerHTML = `<p class="empty">Todo el catálogo está sobre el mínimo. Nada por reponer. 👍</p>`;
        return;
      }
      const { rows, total, truncated } = capRows(work);
      tableHost.innerHTML = `
        <table class="data-table inv-repos">
          <thead><tr>
            <th>Producto</th><th class="num">Stock</th><th>Estado</th><th class="num">Reponer</th>
          </tr></thead>
          <tbody>${rows.map(reposRow).join("")}</tbody>
        </table>
        <p class="table-foot muted">${
          truncated
            ? `${rows.length} de ${total} por reponer · mostrando los ${LIST_RENDER_CAP} más urgentes`
            : `${total} producto(s) por reponer`
        } · toca una fila para ajustar stock</p>
      `;
      tableHost.querySelectorAll<HTMLElement>("tr[data-id]").forEach((tr) =>
        tr.addEventListener("click", () => onOpen(tr.dataset.id!)),
      );
    } catch (err) {
      tableHost.innerHTML = errorStateHtml(err, "el inventario");
    }
  })();
}

function reposRow(w: ReturnType<typeof reorderList<Product>>[number]): string {
  const p = w.item;
  const sub = p.laboratory || p.active_ingredient || "";
  return `
    <tr data-id="${escapeHtml(p.id)}" class="inv-row" tabindex="0">
      <td>
        <div class="cell-main">${escapeHtml(p.name)}</div>
        ${sub ? `<div class="cell-sub muted">${escapeHtml(sub)}</div>` : ""}
      </td>
      <td class="num">${num(w.stock)}</td>
      <td>${stockPill(w.stock)}</td>
      <td class="num inv-reorder">${num(w.suggest)} u.</td>
    </tr>
  `;
}

// --- "Rotación" tab (ABC / Pareto over units sold) --------------------------

function renderRotacion(
  panel: HTMLElement,
  serverUrl: string,
  onOpen: (id: string) => void,
): void {
  panel.innerHTML = `
    <div class="inv-venc-head">
      <span class="muted">Rotación por unidades vendidas — clasificación ABC (Pareto). A = lo que más se mueve, C = baja rotación.</span>
    </div>
    <div class="table-card"><div id="inv-rot-table">${tableSkeleton()}</div></div>
  `;
  const tableHost = panel.querySelector<HTMLElement>("#inv-rot-table")!;
  void (async (): Promise<void> => {
    try {
      const raw = await stockRotation(serverUrl);
      if (raw.length === 0) {
        tableHost.innerHTML = `<p class="empty">Sin ventas en el período para calcular rotación.</p>`;
        return;
      }
      // Unbounded feed (no server limit): at 50k SKUs this is the worst DOM
      // cliff. Rows are ABC-ranked (qty_sold desc) → the head is the A-class
      // movers the operator cares about; cap the render so the tail of C-class
      // dead stock can't jank the frame. Long tail still reachable via export.
      const { rows, total, truncated } = capRows(rotacionRows(raw));
      tableHost.innerHTML = `
        <table class="data-table inv-rot">
          <thead><tr>
            <th>Producto</th><th>Clase</th>
            <th class="num">Vendidas</th><th class="num">Participación</th><th class="num">Stock</th>
          </tr></thead>
          <tbody>${rows.map(rotacionRow).join("")}</tbody>
        </table>
        <p class="table-foot muted">${
          truncated
            ? `${rows.length} de ${total} productos · mostrando el top ${LIST_RENDER_CAP} por rotación`
            : `${total} producto(s)`
        } · toca una fila para ver el detalle</p>
      `;
      tableHost.querySelectorAll<HTMLElement>("tr[data-id]").forEach((tr) =>
        tr.addEventListener("click", () => onOpen(tr.dataset.id!)),
      );
    } catch (err) {
      tableHost.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  })();
}

function rotacionRow(r: ReturnType<typeof rotacionRows>[number]): string {
  const tone = r.class === "A" ? "ok" : r.class === "B" ? "warn" : "danger";
  return `
    <tr data-id="${escapeHtml(r.id)}" class="inv-row" tabindex="0">
      <td>${escapeHtml(r.name)}</td>
      <td><span class="pill pill-${tone}">${r.class}</span></td>
      <td class="num">${num(r.qty_sold)}</td>
      <td class="num">${escapeHtml(r.sharePct)}</td>
      <td class="num">${num(r.current_stock)}</td>
    </tr>
  `;
}

// --- small helpers ----------------------------------------------------------

function stockPill(stock: number): string {
  const level = stockLevel(stock);
  const tone = level === "out" ? "danger" : level === "low" ? "warn" : "ok";
  const label = level === "out" ? "Agotado" : level === "low" ? "Bajo" : "OK";
  return `<span class="pill pill-${tone}">${label}</span>`;
}

function expiryPill(iso: string): string {
  const s = expiryStatus(iso);
  if (s.tone === "muted") return `<span class="pill">—</span>`;
  return `<span class="pill pill-${s.tone}">${s.label}</span>`;
}

function pdStat(label: string, value: string): string {
  return `
    <div class="pd-stat">
      <span class="pd-stat-label muted">${escapeHtml(label)}</span>
      <strong class="pd-stat-value">${value}</strong>
    </div>
  `;
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("es-CL", { day: "2-digit", month: "2-digit", year: "numeric" });
}

function detailSkeleton(): string {
  return `<div class="table-skel">${Array.from({ length: 5 })
    .map(() => `<div class="sk sk-row"></div>`)
    .join("")}</div>`;
}

function trimOrUndef(v: string): string | undefined {
  const t = v.trim();
  return t === "" ? undefined : t;
}

/** Trimmed integer as a number, or undefined when blank/invalid. */
function intOrUndef(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isFinite(n) ? Math.trunc(n) : undefined;
}

/** Trimmed integer as a STRING (for money/Decimal fields), or undefined. */
function intStrOrUndef(v: string): string | undefined {
  const n = intOrUndef(v);
  return n === undefined ? undefined : String(n);
}

function showErr(el: HTMLElement, msg: string): void {
  el.textContent = msg;
  el.hidden = false;
}

function busy(btn: HTMLButtonElement, on: boolean): void {
  btn.disabled = on;
  btn.classList.toggle("loading", on);
}

function showToast(el: HTMLElement, msg: string): void {
  el.textContent = msg;
  el.hidden = false;
  el.classList.add("show");
  window.setTimeout(() => {
    el.classList.remove("show");
    window.setTimeout(() => (el.hidden = true), 250);
  }, 2800);
}

function invStyles(): string {
  return `<style id="inv-styles">
    .view-inventory .inv-head-actions { display: flex; align-items: center; gap: 12px; }
    .view-inventory .inv-new-btn { white-space: nowrap; }
    .view-inventory .inv-export-wrap { position: relative; }
    .view-inventory .inv-export-btn { white-space: nowrap; }
    .view-inventory .inv-export-menu {
      position: absolute; top: calc(100% + 6px); right: 0; z-index: 20;
      background: var(--bg-1); border: 1px solid var(--line); border-radius: var(--radius-field);
      box-shadow: var(--shadow, 0 8px 24px rgba(0,0,0,0.35)); padding: 4px; min-width: 160px;
    }
    .view-inventory .inv-export-menu button {
      appearance: none; display: block; width: 100%; text-align: left; background: transparent;
      border: 0; color: var(--text); font: inherit; padding: 9px 12px; border-radius: 8px; cursor: pointer;
    }
    .view-inventory .inv-export-menu button:hover { background: var(--bg-2); }
    .caja-empty .empty-cta { margin-top: 14px; }
    .view-inventory .inv-tabs { display: flex; gap: 4px; margin: 4px 0 16px; border-bottom: 1px solid var(--line); }
    .view-inventory .inv-tab {
      appearance: none; background: transparent; border: 0; color: var(--muted);
      font: inherit; font-weight: 600; padding: 10px 14px; cursor: pointer;
      border-bottom: 2px solid transparent; margin-bottom: -1px;
    }
    .view-inventory .inv-tab:hover { color: var(--text); }
    .view-inventory .inv-tab[aria-selected="true"] { color: var(--accent); border-bottom-color: var(--accent); }
    .view-inventory tr.inv-row { cursor: pointer; }
    .view-inventory tr.inv-row:hover { background: var(--bg-2); }
    .view-inventory .inv-reorder { color: var(--accent); font-weight: 600; margin-top: 2px; }
    .view-inventory .inv-venc-head { display: flex; align-items: center; gap: 12px; margin-bottom: 14px; }
    .view-inventory .inv-chips { display: flex; gap: 6px; }
    .view-inventory .inv-chip {
      appearance: none; background: var(--bg-2); border: 1px solid var(--line); color: var(--muted-strong);
      border-radius: var(--radius-pill); padding: 6px 14px; font: inherit; font-size: 0.85rem; cursor: pointer;
    }
    .view-inventory .inv-chip[aria-pressed="true"] { background: var(--accent-2); border-color: var(--accent); color: var(--text); }
    .view-inventory select {
      width: 100%; background: var(--bg-2); border: 1px solid var(--line); color: var(--text);
      border-radius: var(--radius-field); padding: 10px 12px; font: inherit;
    }
    .inv-modal-wide { max-width: 560px; width: 92vw; position: relative; }
    .inv-modal-x {
      position: absolute; top: 12px; right: 14px; appearance: none; background: transparent; border: 0;
      color: var(--muted); font-size: 1.5rem; line-height: 1; cursor: pointer;
    }
    .inv-modal-x:hover { color: var(--text); }
    .inv-form-row { display: flex; gap: 12px; }
    .inv-form-row .field { flex: 1; }
    .pd-sub { margin-top: -6px; }
    .pd-grid { display: flex; gap: 10px; margin: 14px 0 6px; }
    .pd-stat { flex: 1; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--radius-field); padding: 10px 12px; }
    .pd-stat-label { display: block; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.04em; }
    .pd-stat-value { display: block; font-size: 1.05rem; margin-top: 4px; }
    .pd-section { margin-top: 18px; border-top: 1px solid var(--line); padding-top: 14px; }
    .pd-section-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
    .pd-section-head h4 { margin: 0; font-size: 0.95rem; }
    .pd-add-lote { padding: 6px 12px; }
    .pd-adj-btn { margin-top: 4px; }
    .pd-empty { margin: 6px 0; }
    .pd-lote-table { margin-top: 6px; }
    .pd-lote-form-inner { margin-top: 10px; border-top: 1px dashed var(--line); padding-top: 12px; }
  </style>`;
}

// --- shared bits reused by other views (do NOT change signatures) ----------
// pos.ts and recetas.ts import tableSkeleton/asMessage/escapeHtml from here;
// caja.ts and reports.ts also import kpiCard/kpiSkeleton/errorCard.

export function kpiCard(label: string, value: string, sub: string, tone = ""): string {
  return `
    <div class="kpi-card ${tone ? `kpi-${tone}` : ""}">
      <span class="kpi-label">${escapeHtml(label)}</span>
      <strong class="kpi-value">${value}</strong>
      <span class="kpi-sub muted">${escapeHtml(sub)}</span>
    </div>
  `;
}

export function kpiSkeleton(n = 4): string {
  return Array.from({ length: n })
    .map(() => `<div class="kpi-card skel"><span class="sk sk-sm"></span><span class="sk sk-lg"></span><span class="sk sk-sm"></span></div>`)
    .join("");
}

export function tableSkeleton(rows = 6): string {
  return `<div class="table-skel">${Array.from({ length: rows })
    .map(() => `<div class="sk sk-row"></div>`)
    .join("")}</div>`;
}

export function errorCard(err: unknown): string {
  return `<div class="kpi-card kpi-danger kpi-span"><span class="kpi-label">Error</span><strong class="kpi-value sm">${escapeHtml(asMessage(err))}</strong></div>`;
}

/** Centered empty-state block (reuses the global `.caja-empty` chrome) — never a
 *  blank screen. Pass `ctaId` to render the call-to-action button; the caller
 *  wires its click. Shared by inventory / compras / gastos. */
export function emptyStateHtml(c: EmptyCopy, ctaId?: string): string {
  const cta =
    c.cta && ctaId
      ? `<button type="button" class="btn-primary empty-cta" id="${escapeHtml(ctaId)}"><span class="btn-label">${escapeHtml(c.cta)}</span></button>`
      : "";
  return `
    <div class="caja-empty">
      <div class="caja-empty-mark">●</div>
      <h3>${escapeHtml(c.title)}</h3>
      <p class="muted">${escapeHtml(c.hint)}</p>
      ${cta}
    </div>
  `;
}

/** Operator-facing error block. A permission / offline failure gets the friendly
 *  centered `.caja-empty` treatment with an actionable hint; an unclassified
 *  error keeps the raw message in the red `.view-error` band so nothing real is
 *  hidden. `resource` customizes the "sin acceso" line ("las compras"). */
export function errorStateHtml(err: unknown, resource?: string): string {
  const c = classifyFetchError(err, resource);
  if (c.kind === "generic") {
    return `<div class="view-error">${escapeHtml(c.hint)}</div>`;
  }
  return `
    <div class="caja-empty">
      <div class="caja-empty-mark">●</div>
      <h3>${escapeHtml(c.title)}</h3>
      <p class="muted">${escapeHtml(c.hint)}</p>
    </div>
  `;
}

export function asMessage(err: unknown): string {
  return typeof err === "string" ? err : "No se pudo cargar la información.";
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );
}

/** Wire a live, **advisory** mód-11 check onto an optional RUT input + its hint
 *  element: green echo when valid, amber warning when the verifier doesn't
 *  match (never blocks — the field is optional registry data), and a blur that
 *  rewrites a valid RUT to the pretty `NN.NNN.NNN-D` form. Used by the customer
 *  and supplier forms; the DTE emisor/receptor use their own *blocking* check.
 *  Returns a `canonical()` getter the caller uses on save to store the SII
 *  `NNNNNNNN-D` form for valid RUTs (raw passthrough otherwise). */
export function attachRutAdvisory(
  input: HTMLInputElement,
  hint: HTMLElement,
): { canonical: () => string } {
  const check = (): void => {
    const raw = input.value.trim();
    if (!raw) {
      hint.hidden = true;
      hint.className = "field-hint";
      return;
    }
    if (isValidRut(raw)) {
      hint.hidden = false;
      hint.className = "field-hint ok";
      hint.textContent = `RUT válido — ${formatRut(raw)}`;
    } else {
      hint.hidden = false;
      hint.className = "field-hint err";
      hint.textContent = "El dígito verificador no calza; revísalo (puedes guardar igual).";
    }
  };
  input.addEventListener("input", check);
  input.addEventListener("blur", () => {
    if (isValidRut(input.value)) input.value = formatRut(input.value);
  });
  check();
  return {
    canonical: () => {
      const raw = input.value.trim();
      return isValidRut(raw) ? canonicalRut(raw) : raw;
    },
  };
}
