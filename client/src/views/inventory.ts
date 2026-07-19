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
  listProductVariants,
  createProductVariant,
  type Product,
  type ProductDetail,
  type Batch,
  type NearExpiryRow,
} from "../api";
import { clp, num, fecha } from "../format";
import {
  toRfc3339Noon,
  stockLevel,
  expiryStatus,
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
} from "./stock-helpers";
import {
  activeVocab,
  activeFeatures,
  cachedPack,
  loadRubroPack,
  localAttrsForRubro,
} from "../vertical";
import {
  productFormLabels,
  visibleAttrFields,
  attrFieldsHtml,
  readAttrValues,
  buildProductInput,
  type ProductFormOptions,
} from "./product-form";
import {
  variantsParentBanner,
  variantsSectionTitle,
  parentStockLabel,
  shouldOfferVariantsUi,
  variantFormAttrFields,
  addVariantButtonLabel,
  addVariantModalTitle,
  hasVariantsToggleLabel,
  hasVariantsToggleHint,
  parentCreatedOpenVariantsToast,
  variantsEmptyHint,
  variantChildNote,
  buildNewVariantInput,
  toVariantTableRow,
  parentStockWhenHasVariants,
  variantsListBadgeFromDto,
  variantStockCellLabel,
  variantRowAriaLabel,
  variantsLoadError,
  variantEditBlockedHint,
  variantFormKeyboardHint,
  matrixComboSuggestions,
} from "./variants-ui";
import { bindModalKeys } from "./modal-keys";

const PAGE_LIMIT = 60;
// Single-page cap the server enforces on `/products` (`limit.min(500)`). The
// in-app export pulls one capped page; a fuller catalog uses the CLI export.
const EXPORT_CAP = 500;
// Reposición scans the whole (server-capped) catalog so a low/out SKU on page 2
// isn't missed — the per-page PAGE_LIMIT 60 would hide it. Same 500 server cap.
const REORDER_SCAN = 500;
type Tab = "productos" | "vencimientos" | "reposicion" | "rotacion";

export function renderInventory(host: HTMLElement, serverUrl: string): void {
  // Pack cache (shell post-login) drives labels + lotes tab; offline → local vocab.
  // Race: if this view mounts before hydrateBranding finishes, we re-apply after
  // loadRubroPack so tabs/labels never stay on the generic "Producto" default.
  let vocab = activeVocab();
  let features = activeFeatures();
  let itemLabel = vocab.item;
  let itemPlural = `${itemLabel}s`;
  let catalogLabel = vocab.catalog;

  host.innerHTML = `
    ${invStyles()}
    <section class="view view-inventory">
      <div class="view-head">
        <div>
          <h2 id="inv-title">${escapeHtml(catalogLabel)}</h2>
          <p class="muted" id="inv-subtitle">${escapeHtml(inventorySubtitle(features))}</p>
        </div>
        <div class="inv-head-actions">
          <div class="view-search" id="inv-search-wrap">
            <input id="inv-search" type="search" placeholder="Buscar ${escapeHtml(itemLabel.toLowerCase())}…" autocomplete="off" />
          </div>
          <div class="inv-export-wrap" id="inv-export-wrap">
            <button id="inv-export-btn" class="btn-ghost inv-export-btn" type="button" aria-haspopup="menu" aria-expanded="false">Exportar ▾</button>
            <div id="inv-export-menu" class="inv-export-menu" role="menu" hidden>
              <button type="button" role="menuitem" data-fmt="csv">CSV (Excel)</button>
              <button type="button" role="menuitem" data-fmt="json">JSON</button>
            </div>
          </div>
          <button id="inv-new-btn" class="btn-primary inv-new-btn">
            <span class="btn-label" id="inv-new-label">+ Nuevo ${escapeHtml(itemLabel.toLowerCase())}</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>

      <div class="inv-tabs" role="tablist">
        <button class="inv-tab" data-tab="productos" role="tab" aria-selected="true" id="inv-tab-productos">${escapeHtml(itemPlural)}</button>
        <button class="inv-tab" data-tab="vencimientos" role="tab" aria-selected="false"${features.lotes ? "" : " hidden"}>Próximos a vencer</button>
        <button class="inv-tab" data-tab="reposicion" role="tab" aria-selected="false"${features.physicalStock ? "" : " hidden"}>Reposición</button>
        <button class="inv-tab" data-tab="rotacion" role="tab" aria-selected="false"${features.physicalStock ? "" : " hidden"}>Rotación</button>
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

  // Pack-driven clinical flag (not the legacy business_vertical setting).
  // Default false until pack loads — never flash farmacia ficha on a tienda.
  let showClinical = features.clinical;
  let physicalStock = features.physicalStock;

  function applyPackChrome(): void {
    vocab = activeVocab();
    features = activeFeatures();
    itemLabel = vocab.item;
    itemPlural = `${itemLabel}s`;
    catalogLabel = vocab.catalog;
    showClinical = features.clinical;
    physicalStock = features.physicalStock;

    const titleEl = host.querySelector<HTMLElement>("#inv-title");
    const subEl = host.querySelector<HTMLElement>("#inv-subtitle");
    const newLabel = host.querySelector<HTMLElement>("#inv-new-label");
    const tabProd = host.querySelector<HTMLElement>("#inv-tab-productos");
    if (titleEl) titleEl.textContent = catalogLabel;
    if (subEl) subEl.textContent = inventorySubtitle(features);
    if (newLabel) newLabel.textContent = `+ Nuevo ${itemLabel.toLowerCase()}`;
    if (tabProd) tabProd.textContent = itemPlural;
    searchEl.placeholder = `Buscar ${itemLabel.toLowerCase()}…`;

    host.querySelectorAll<HTMLButtonElement>(".inv-tab").forEach((b) => {
      const t = b.dataset.tab as Tab;
      if (t === "vencimientos") b.hidden = !features.lotes;
      if (t === "reposicion" || t === "rotacion") b.hidden = !features.physicalStock;
    });
    // If the active tab was hidden by the pack, fall back to productos.
    if (
      (tab === "vencimientos" && !features.lotes) ||
      ((tab === "reposicion" || tab === "rotacion") && !features.physicalStock)
    ) {
      selectTab("productos");
    }
  }

  // Re-hydrate when shell pack arrives after this view mounted (C4 race).
  void loadRubroPack(serverUrl)
    .then(() => {
      applyPackChrome();
      if (tab === "productos") {
        void loadKpis(kpiHost, serverUrl);
        const table = panel.querySelector<HTMLElement>("#inv-table");
        if (table) void loadProducts(table, serverUrl, searchEl.value.trim(), openDetail, physicalStock);
      }
    })
    .catch(() => {
      /* loadRubroPack never throws; belt-and-suspenders */
    });

  function openDetail(id: string): void {
    void openProductDetail(modalHost, serverUrl, id, toast, refreshProductos, {
      clinical: showClinical,
      physicalStock,
      lotes: features.lotes,
      itemWord: itemLabel,
    });
  }

  function refreshProductos(): void {
    if (tab !== "productos") return;
    void loadKpis(kpiHost, serverUrl);
    const table = panel.querySelector<HTMLElement>("#inv-table");
    if (table) void loadProducts(table, serverUrl, searchEl.value.trim(), openDetail, physicalStock);
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
        physicalStock,
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
    openNewProduct(modalHost, serverUrl, (created, meta) => {
      toast(
        meta?.hasVariants
          ? parentCreatedOpenVariantsToast(created.name)
          : `${itemLabel} creado: ${created.name}`,
      );
      refreshProductos();
    }),
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
      void runInventoryExport(serverUrl, b.dataset.fmt === "json" ? "json" : "csv", showClinical, toast);
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
        void loadProducts(table, serverUrl, searchEl.value.trim(), openDetail, physicalStock);
      }
    }, 220);
  });
}

/** Subtitle under the catalog title — honest per pack (no "lotes" on tienda). */
export function inventorySubtitle(f: { lotes: boolean; physicalStock: boolean }): string {
  if (!f.physicalStock) return "Catálogo de servicios para el POS.";
  if (f.lotes) return "Stock, lotes y vencimientos del catálogo.";
  return "Stock y catálogo de productos.";
}

async function loadKpis(host: HTMLElement, serverUrl: string): Promise<void> {
  try {
    const s = await inventorySummary(serverUrl);
    const itemPlural = `${activeVocab().item}s`;
    host.innerHTML = [
      kpiCard(itemPlural, num(s.total), `${num(s.active)} activos`),
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
  physicalStock = true,
): Promise<void> {
  try {
    const rows: Product[] = await listProducts(serverUrl, search || undefined, PAGE_LIMIT);
    const itemLabel = activeVocab().item;
    if (rows.length === 0) {
      host.innerHTML = emptyStateHtml(
        inventoryEmpty(search !== "", { itemWord: itemLabel, physicalStock }),
        search ? undefined : "inv-empty-new",
      );
      host
        .querySelector<HTMLButtonElement>("#inv-empty-new")
        ?.addEventListener("click", () =>
          host.closest(".view-inventory")?.querySelector<HTMLButtonElement>("#inv-new-btn")?.click(),
        );
      return;
    }
    const stockHead = physicalStock ? `<th class="num">Stock</th><th>Estado</th>` : `<th>Estado</th>`;
    const footHint = physicalStock
      ? "toca una fila para detalle, ajustar stock y lotes"
      : "toca una fila para ver el detalle";
    host.innerHTML = `
      <table class="data-table inv-products">
        <thead>
          <tr><th>${escapeHtml(itemLabel)}</th><th class="num">Precio</th>${stockHead}</tr>
        </thead>
        <tbody>
          ${rows.map((p) => productRow(p, physicalStock)).join("")}
        </tbody>
      </table>
      <p class="table-foot muted">${rows.length} ${escapeHtml(itemLabel.toLowerCase())}(s)${
        rows.length === PAGE_LIMIT ? ` · mostrando los primeros ${PAGE_LIMIT}` : ""
      } · ${footHint}</p>
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

function productRow(p: Product, physicalStock = true): string {
  const sub = p.laboratory || p.active_ingredient || "";
  const isParent =
    physicalStock &&
    (p.variant_count != null || p.variants_stock != null) &&
    (typeof p.variant_count === "number" || typeof p.variants_stock === "number");
  const badgeLabel = isParent ? variantsListBadgeFromDto(p) : "";
  const badge = badgeLabel
    ? `<span class="pill pill-info inv-var-badge">${escapeHtml(badgeLabel)}</span>`
    : "";
  // Min-stock signal: only for plain physical rows (parent shell stock is not sellable).
  const reorder =
    physicalStock && !isParent && stockLevel(p.stock) !== "ok"
      ? `<div class="cell-sub inv-reorder">Reponer ${num(reorderSuggestion(p.stock))} u.</div>`
      : "";
  const plainOut = physicalStock && !isParent && p.stock <= 0;
  const stockCells = physicalStock
    ? isParent
      ? `<td class="num">${
          p.variants_stock != null ? `${num(Number(p.variants_stock))} <span class="muted">u. var.</span>` : "—"
        }</td>
      <td>${badge}</td>`
      : plainOut
        ? `<td class="num">0</td>
      <td><span class="pill pill-danger">Agotado</span>${reorder}</td>`
        : `<td class="num">${num(p.stock)}</td>
      <td>${stockPill(p.stock)}${reorder}</td>`
    : `<td><span class="pill pill-ok">Servicio</span></td>`;
  return `
    <tr data-id="${escapeHtml(p.id)}" class="inv-row" tabindex="0" ${
      isParent ? 'aria-description="Producto multi-SKU con variantes"' : ""
    }>
      <td>
        <div class="cell-main">${escapeHtml(p.name)}</div>
        ${sub ? `<div class="cell-sub muted">${escapeHtml(sub)}</div>` : ""}
        ${isParent ? `<div class="cell-sub muted">Vender por código de barras del hijo</div>` : ""}
      </td>
      <td class="num">${clp(p.price)}</td>
      ${stockCells}
    </tr>
  `;
}

// --- "Nuevo producto" modal (pack-driven attrs + clinical + service UX) -----

function openNewProduct(
  modalHost: HTMLElement,
  serverUrl: string,
  onDone: (created: ProductDetail, meta?: { hasVariants?: boolean }) => void,
): void {
  const pack = cachedPack();
  const features = activeFeatures();
  const vocab = activeVocab();
  // Prefer server pack attrs; offline localPack already fills them. Extra
  // safety: if cache is empty (pre-hydrate race), use local mirror by rubro.
  const rawAttrs =
    pack?.attrs?.length
      ? pack.attrs
      : localAttrsForRubro(pack?.rubro ?? null);
  const attrFields = visibleAttrFields(rawAttrs, features.clinical);
  const formOpts: ProductFormOptions = {
    vocab,
    physicalStock: features.physicalStock,
    clinical: features.clinical,
    attrFields,
  };
  const labels = productFormLabels(formOpts);
  // Presentación is universal (size / format) and NOT a pack attr for most
  // rubros — keep a single optional field. Clinical keys come from pack attrs
  // when clinical (farmacia), so we don't double-render lab/ingredient.
  const offerVariants = shouldOfferVariantsUi(features.physicalStock);
  const stockBlock = features.physicalStock
    ? `<label class="field modal-field" id="np-stock-wrap">
          <span class="modal-label">${escapeHtml(labels.stockLabel)}</span>
          <input id="np-stock" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
        </label>`
    : `<div class="field modal-field">
          <span class="modal-label">${escapeHtml(labels.stockLabel)}</span>
          <p class="muted inv-service-hint">${escapeHtml(labels.stockHint ?? "")}</p>
        </div>`;
  const variantsToggle = offerVariants
    ? `<label class="field modal-field np-has-variants">
          <span class="modal-check-row">
            <input id="np-has-variants" type="checkbox" />
            <span class="modal-label">${escapeHtml(hasVariantsToggleLabel())}</span>
          </span>
          <p class="muted inv-service-hint" id="np-has-variants-hint" hidden>${escapeHtml(hasVariantsToggleHint())}</p>
        </label>`
    : "";

  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal inv-modal-wide" role="dialog" aria-modal="true" aria-label="${escapeHtml(labels.title)}">
        <h3 class="modal-title">${escapeHtml(labels.title)}</h3>
        <label class="field modal-field">
          <span class="modal-label">${escapeHtml(labels.nameLabel)}</span>
          <input id="np-name" type="text" autocomplete="off" placeholder="${escapeHtml(labels.namePlaceholder)}" />
        </label>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">${escapeHtml(labels.priceLabel)}</span>
            <input id="np-price" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">${escapeHtml(labels.costLabel)}</span>
            <input id="np-cost" type="text" inputmode="numeric" placeholder="opcional" autocomplete="off" />
          </label>
        </div>
        <div class="inv-form-row">
          ${stockBlock}
          <label class="field modal-field">
            <span class="modal-label">Presentación</span>
            <input id="np-presentation" type="text" autocomplete="off" placeholder="opcional" />
          </label>
        </div>
        ${variantsToggle}
        ${attrFieldsHtml(attrFields, escapeHtml)}
        <div id="np-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button id="np-cancel" class="btn-ghost">Cancelar</button>
          <button id="np-confirm" class="btn-primary modal-confirm">
            <span class="btn-label">${escapeHtml(labels.submitLabel)}</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    </div>
  `;

  let unbindNpKeys: (() => void) | undefined;
  const close = (): void => {
    unbindNpKeys?.();
    modalHost.innerHTML = "";
  };
  unbindNpKeys = bindModalKeys(close);
  const nameEl = modalHost.querySelector<HTMLInputElement>("#np-name")!;
  const priceEl = modalHost.querySelector<HTMLInputElement>("#np-price")!;
  const costEl = modalHost.querySelector<HTMLInputElement>("#np-cost")!;
  const stockEl = modalHost.querySelector<HTMLInputElement>("#np-stock");
  const stockWrap = modalHost.querySelector<HTMLElement>("#np-stock-wrap");
  const hasVarEl = modalHost.querySelector<HTMLInputElement>("#np-has-variants");
  const hasVarHint = modalHost.querySelector<HTMLElement>("#np-has-variants-hint");
  const presEl = modalHost.querySelector<HTMLInputElement>("#np-presentation")!;
  const errEl = modalHost.querySelector<HTMLElement>("#np-error")!;
  const confirmBtn = modalHost.querySelector<HTMLButtonElement>("#np-confirm")!;

  const syncHasVariantsUi = (): void => {
    const on = Boolean(hasVarEl?.checked);
    if (hasVarHint) hasVarHint.hidden = !on;
    if (stockEl) {
      stockEl.disabled = on;
      if (on) stockEl.value = "0";
    }
    if (stockWrap) stockWrap.classList.toggle("is-muted", on);
  };
  hasVarEl?.addEventListener("change", syncHasVariantsUi);

  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#np-cancel")!.addEventListener("click", close);
  nameEl.focus();

  // Enter on price confirms (keyboard-first alta).
  for (const el of [nameEl, priceEl, costEl, stockEl, presEl].filter(Boolean) as HTMLInputElement[]) {
    el.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        confirmBtn.click();
      }
    });
  }

  confirmBtn.addEventListener("click", async () => {
    const hasVariants = Boolean(hasVarEl?.checked);
    const stockForParent = parentStockWhenHasVariants(
      hasVariants,
      features.physicalStock,
      stockEl?.value,
    );
    const built = buildProductInput(
      {
        name: nameEl.value,
        price: priceEl.value,
        costPrice: costEl.value,
        // When has-variants: force "0" so pure model also validates cleanly.
        stock: hasVariants ? "0" : stockEl?.value,
        presentation: presEl.value,
        attrs: readAttrValues(modalHost, attrFields),
      },
      formOpts,
    );
    if (!built.ok) {
      showErr(errEl, built.error);
      return;
    }
    // Belt-and-suspenders: parent shell stock is 0 when multi-SKU.
    if (hasVariants && features.physicalStock) {
      built.value.stock = stockForParent ?? 0;
    }
    errEl.hidden = true;
    busy(confirmBtn, true);
    try {
      const created = await createProduct(serverUrl, built.value);
      close();
      onDone(created, { hasVariants });
    } catch (err) {
      showErr(errEl, asMessage(err));
      busy(confirmBtn, false);
    }
  });
}

// --- product detail modal: detail + ajustar stock + lotes ------------------

interface DetailPackOpts {
  clinical: boolean;
  physicalStock: boolean;
  lotes: boolean;
  itemWord: string;
}

async function openProductDetail(
  modalHost: HTMLElement,
  serverUrl: string,
  id: string,
  toast: (msg: string) => void,
  onChanged: () => void,
  packOpts: DetailPackOpts = {
    clinical: true,
    physicalStock: true,
    lotes: true,
    itemWord: "Producto",
  },
): Promise<void> {
  modalHost.innerHTML = `
    <div class="modal-backdrop">
      <div class="modal inv-modal-wide" role="dialog" aria-modal="true" aria-labelledby="pd-title" aria-label="Detalle de producto">
        <button id="pd-close" class="inv-modal-x" aria-label="Cerrar detalle">×</button>
        <div id="pd-body" aria-busy="true">${detailSkeleton()}</div>
      </div>
    </div>
  `;
  let unbindKeys: (() => void) | undefined;
  const close = (): void => {
    unbindKeys?.();
    modalHost.innerHTML = "";
  };
  unbindKeys = bindModalKeys(close);
  modalHost.querySelector<HTMLElement>(".modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });
  modalHost.querySelector<HTMLButtonElement>("#pd-close")!.addEventListener("click", close);
  const bodyEl = modalHost.querySelector<HTMLElement>("#pd-body")!;

  let p: ProductDetail;
  try {
    p = await productDetail(serverUrl, id);
  } catch (err) {
    bodyEl.setAttribute("aria-busy", "false");
    bodyEl.innerHTML = errorStateHtml(err);
    return;
  }

  // Multi-SKU children (empty when product is plain or is itself a child).
  let variants: ProductDetail[] = [];
  let variantsLoadErr = "";
  try {
    // Only fetch for top-level products; a child has parent_id set.
    if (!p.parent_id && shouldOfferVariantsUi(packOpts.physicalStock)) {
      bodyEl.setAttribute("aria-busy", "true");
      bodyEl.setAttribute("aria-live", "polite");
      variants = await listProductVariants(serverUrl, id);
    }
  } catch (err) {
    // Pre-variants server: silent empty. Real network errors: surface Spanish.
    const msg = asMessage(err);
    if (/404|not found|no encontr/i.test(msg)) {
      variants = [];
    } else {
      variantsLoadErr = variantsLoadError(msg);
      variants = [];
    }
  }
  bodyEl.setAttribute("aria-busy", "false");

  function paintDetail(): void {
    const kids = variants;
    const clinicalBits = packOpts.clinical
      ? [p.laboratory, p.active_ingredient, p.presentation]
      : [p.presentation];
    const attrBits = formatAttrsSubline(p.attrs);
    const sub = [...clinicalBits.filter(Boolean), ...attrBits].join(" · ");
    const isChild = Boolean(p.parent_id);
    const isParent = kids.length > 0;
    // Physical rubro + not a child: offer multi-SKU panel (empty or with rows).
    const offerVariants =
      shouldOfferVariantsUi(packOpts.physicalStock) && !isChild && !p.parent_id;
    // Parent multi-SKU: stock lives on children — hide adjust on the shell product.
    const showStockOps = packOpts.physicalStock && !isParent;
    const stockStat = !packOpts.physicalStock
      ? pdStat("Tipo", "Servicio")
      : isParent
        ? pdStat("Stock", parentStockLabel(kids))
        : pdStat("Stock", `${num(p.stock)} ${stockPill(p.stock)}`);
    const childNote = isChild
      ? `<p class="muted pd-sub">${escapeHtml(variantChildNote())}</p>`
      : "";
    const variantsSection = offerVariants
      ? `
      <div class="pd-section pd-variants">
        <div class="pd-section-head">
          <h4>${escapeHtml(variantsSectionTitle(kids.length))}</h4>
          <button type="button" id="pd-add-variant" class="btn-ghost pd-add-variant">${escapeHtml(addVariantButtonLabel())}</button>
        </div>
        ${
          kids.length > 0
            ? `<p class="pd-variants-banner" role="status">${escapeHtml(variantsParentBanner(kids.length))}</p>
        <table class="data-table pd-variant-table">
          <thead><tr><th>Variante</th><th>Cód. barras</th><th class="num">Precio</th><th class="num">Stock</th></tr></thead>
          <tbody>
            ${kids
              .map((v) => {
                const row = toVariantTableRow({
                  id: v.id,
                  name: v.name,
                  price: v.price,
                  stock: v.stock,
                  active: v.active,
                  attrs: v.attrs as Record<string, unknown> | null,
                  barcode: v.barcode,
                });
                const lab = row.attrsLabel;
                const st = variantStockCellLabel(row.stock);
                const aria = variantRowAriaLabel({
                  name: row.name,
                  barcode: row.barcode,
                  stock: row.stock,
                  attrsLabel: lab,
                });
                return `<tr data-variant-id="${escapeHtml(v.id)}" aria-label="${escapeHtml(aria)}">
                  <td><div class="cell-main">${escapeHtml(row.name)}</div>
                    ${lab && lab !== row.name ? `<div class="cell-sub muted">${escapeHtml(lab)}</div>` : ""}</td>
                  <td class="rb-num">${escapeHtml(row.barcode)}</td>
                  <td class="num">${clp(row.price)}</td>
                  <td class="num">${
                    st.out
                      ? `<span class="pill pill-danger" title="Sin stock">${escapeHtml(st.text)}</span>`
                      : escapeHtml(st.text)
                  }</td>
                </tr>`;
              })
              .join("")}
          </tbody>
        </table>
        <p class="muted pd-variants-edit-hint">${escapeHtml(variantEditBlockedHint())}</p>`
            : variantsLoadErr
              ? `<p class="pos-error pd-variants-empty" role="alert">${escapeHtml(variantsLoadErr)}</p>`
              : `<p class="muted pd-variants-empty" role="status">${escapeHtml(variantsEmptyHint())}</p>`
        }
        ${matrixHintHtml(kids)}
        <div id="pd-variant-form"></div>
      </div>`
      : "";
    const adjustSection = showStockOps
      ? `
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
      </div>`
      : "";
    const lotesSection = packOpts.lotes && !isParent
      ? `
      <div class="pd-section">
        <div class="pd-section-head">
          <h4>Lotes y vencimientos</h4>
          <button id="pd-add-lote" class="btn-ghost pd-add-lote">+ Agregar lote</button>
        </div>
        <div id="pd-lotes">${tableSkeleton(3)}</div>
        <div id="pd-lote-form"></div>
      </div>`
      : "";
    bodyEl.innerHTML = `
      <h3 class="modal-title" id="pd-title">${escapeHtml(p.name)}</h3>
      ${sub ? `<p class="muted pd-sub">${escapeHtml(sub)}</p>` : ""}
      ${childNote}
      <div class="pd-grid">
        ${pdStat("Precio venta", clp(p.price))}
        ${pdStat("Costo", p.cost_price ? clp(p.cost_price) : "—")}
        ${stockStat}
      </div>
      ${variantsSection}
      ${adjustSection}
      ${lotesSection}
    `;
    if (showStockOps) wireAdjust();
    if (offerVariants) {
      bodyEl
        .querySelector<HTMLButtonElement>("#pd-add-variant")!
        .addEventListener("click", () => toggleVariantForm());
    }
    if (packOpts.lotes && !isParent) {
      bodyEl
        .querySelector<HTMLButtonElement>("#pd-add-lote")!
        .addEventListener("click", toggleLoteForm);
      void loadLotes();
    }
  }

  /** Thin talla×color honesty: missing combos as text (no full matrix API). */
  function matrixHintHtml(kids: ProductDetail[]): string {
    const pack = cachedPack();
    const rawAttrs =
      pack?.attrs?.length
        ? pack.attrs
        : localAttrsForRubro(pack?.rubro ?? null);
    const keys = new Set((rawAttrs ?? []).map((a) => a.key.toLowerCase()));
    if (!keys.has("talla") && !keys.has("color")) return "";
    const existing = kids.map((k) => {
      const a = (k.attrs ?? {}) as Record<string, unknown>;
      return {
        talla: a.talla != null ? String(a.talla) : "",
        color: a.color != null ? String(a.color) : "",
      };
    });
    const tallas = existing.map((e) => e.talla).filter(Boolean);
    const colores = existing.map((e) => e.color).filter(Boolean);
    // Seed common Chilean retail sizes when empty so the hint is useful after first SKUs.
    const tPool = tallas.length ? tallas : ["XS", "S", "M", "L", "XL"];
    const cPool = colores.length ? colores : [];
    if (tallas.length === 0 && colores.length === 0) return "";
    const combos = matrixComboSuggestions(tPool, cPool.length ? cPool : [""], existing);
    const missing = combos.filter((c) => c.missing && c.label.trim() !== "").slice(0, 8);
    if (missing.length === 0) return "";
    return `<p class="muted pd-matrix-hint" role="note">Combinaciones sugeridas sin alta: ${escapeHtml(
      missing.map((m) => m.label).join(", "),
    )}. Agrégalas con código de barras (matriz completa próximamente).</p>`;
  }

  function toggleVariantForm(): void {
    const host = bodyEl.querySelector<HTMLElement>("#pd-variant-form");
    if (!host) return;
    if (host.innerHTML !== "") {
      host.innerHTML = "";
      return;
    }
    const pack = cachedPack();
    const rawAttrs =
      pack?.attrs?.length
        ? pack.attrs
        : localAttrsForRubro(pack?.rubro ?? null);
    const vFields = variantFormAttrFields(rawAttrs);
    host.innerHTML = `
      <div class="pd-variant-form-inner" role="form" aria-label="${escapeHtml(addVariantModalTitle(p.name))}">
        <h4 class="pd-variant-form-title" id="vf-title">${escapeHtml(addVariantModalTitle(p.name))}</h4>
        <p class="muted pd-sub">Código de barras primero — el POS vende escaneando este SKU.</p>
        <p class="muted pd-sub pd-kb-hint">${escapeHtml(variantFormKeyboardHint())}</p>
        <label class="field modal-field">
          <span class="modal-label">Código de barras *</span>
          <input id="vf-barcode" type="text" autocomplete="off" inputmode="text"
                 placeholder="Escanear o escribir EAN/SKU" aria-required="true"
                 aria-describedby="vf-title" />
        </label>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Nombre (opcional)</span>
            <input id="vf-name" type="text" autocomplete="off" placeholder="Vacío = padre + talla/color" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">Stock inicial</span>
            <input id="vf-stock" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
          </label>
        </div>
        <div class="inv-form-row">
          <label class="field modal-field">
            <span class="modal-label">Precio (opcional)</span>
            <input id="vf-price" type="text" inputmode="numeric" placeholder="hereda del padre" autocomplete="off" />
          </label>
          <label class="field modal-field">
            <span class="modal-label">Costo (opcional)</span>
            <input id="vf-cost" type="text" inputmode="numeric" placeholder="opcional" autocomplete="off" />
          </label>
        </div>
        ${attrFieldsHtml(vFields, escapeHtml, "vf-attr-")}
        <div id="vf-error" class="pos-error" hidden></div>
        <div class="modal-actions">
          <button type="button" id="vf-cancel" class="btn-ghost">Cancelar</button>
          <button type="button" id="vf-confirm" class="btn-primary modal-confirm">
            <span class="btn-label">Crear variante</span>
            <span class="btn-pulse"></span>
          </button>
        </div>
      </div>
    `;
    const barcodeEl = host.querySelector<HTMLInputElement>("#vf-barcode")!;
    const nameEl = host.querySelector<HTMLInputElement>("#vf-name")!;
    const stockEl = host.querySelector<HTMLInputElement>("#vf-stock")!;
    const priceEl = host.querySelector<HTMLInputElement>("#vf-price")!;
    const costEl = host.querySelector<HTMLInputElement>("#vf-cost")!;
    const errEl = host.querySelector<HTMLElement>("#vf-error")!;
    const btn = host.querySelector<HTMLButtonElement>("#vf-confirm")!;
    const closeForm = (): void => {
      host.innerHTML = "";
    };
    host.querySelector<HTMLButtonElement>("#vf-cancel")!.addEventListener("click", closeForm);
    barcodeEl.focus();

    const submit = async (): Promise<void> => {
      const built = buildNewVariantInput({
        barcode: barcodeEl.value,
        name: nameEl.value,
        stock: stockEl.value,
        price: priceEl.value,
        costPrice: costEl.value,
        attrs: readAttrValues(host, vFields, "vf-attr-"),
      });
      if (!built.ok) {
        showErr(errEl, built.error);
        barcodeEl.focus();
        return;
      }
      errEl.hidden = true;
      busy(btn, true);
      try {
        const created = await createProductVariant(serverUrl, id, built.value);
        variants = [...variants, created];
        toast(`Variante creada: ${created.name}`);
        onChanged();
        paintDetail();
      } catch (err) {
        showErr(errEl, asMessage(err));
        busy(btn, false);
        barcodeEl.focus();
      }
    };

    btn.addEventListener("click", () => {
      void submit();
    });

    // Enter on barcode submits (scan gun dumps code + Enter). Esc closes form
    // without closing the whole product detail (scoped handler).
    const onFormKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        closeForm();
      } else if (e.key === "Enter" && e.target === barcodeEl) {
        e.preventDefault();
        void submit();
      }
    };
    host.addEventListener("keydown", onFormKey);
  }

  /** Flatten product.attrs into short subline tokens (talla M · color Azul). */
  function formatAttrsSubline(attrs: ProductDetail["attrs"]): string[] {
    if (!attrs || typeof attrs !== "object" || Array.isArray(attrs)) return [];
    const out: string[] = [];
    for (const [k, v] of Object.entries(attrs)) {
      if (v == null || v === "") continue;
      out.push(`${k}: ${String(v)}`);
    }
    return out.slice(0, 6);
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
      <td>${fecha(b.expiry_date)}</td>
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
      <td>${fecha(r.expiry_date)}</td>
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
// Canonical home is ./view-blocks (moved out of this file); re-exported here
// so the 16 views that import helpers `from "./inventory"` keep working.

export {
  escapeHtml,
  asMessage,
  kpiCard,
  kpiSkeleton,
  tableSkeleton,
  errorCard,
  emptyStateHtml,
  errorStateHtml,
  attachRutAdvisory,
} from "./view-blocks";

import {
  escapeHtml,
  asMessage,
  kpiCard,
  kpiSkeleton,
  tableSkeleton,
  errorCard,
  emptyStateHtml,
  errorStateHtml,
} from "./view-blocks";
