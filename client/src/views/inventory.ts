// Inventario view — core ERP, available on every tier. KPI cards (count, low /
// out of stock, valuation) from `/products/stats` + a searchable product table
// from `/products`. This is the reference pattern the POS and Reportes views
// follow: render skeleton → fetch via ../api → swap in real DOM → Spanish copy,
// CLP via ../format.
import { inventorySummary, listProducts, type Product } from "../api";
import { clp, num } from "../format";

const PAGE_LIMIT = 60;

export function renderInventory(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-inventory">
      <div class="view-head">
        <div>
          <h2>Inventario</h2>
          <p class="muted">Stock actual y valorización del catálogo.</p>
        </div>
        <div class="view-search">
          <input id="inv-search" type="search" placeholder="Buscar producto…" autocomplete="off" />
        </div>
      </div>

      <div id="inv-kpis" class="kpi-grid">${kpiSkeleton()}</div>

      <div class="table-card">
        <div id="inv-table">${tableSkeleton()}</div>
      </div>
    </section>
  `;

  const kpiHost = host.querySelector<HTMLElement>("#inv-kpis")!;
  const tableHost = host.querySelector<HTMLElement>("#inv-table")!;
  const searchEl = host.querySelector<HTMLInputElement>("#inv-search")!;

  void loadKpis(kpiHost, serverUrl);
  void loadProducts(tableHost, serverUrl, "");

  // Debounced server-side search.
  let timer: number | undefined;
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => {
      tableHost.innerHTML = tableSkeleton();
      void loadProducts(tableHost, serverUrl, searchEl.value.trim());
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

async function loadProducts(host: HTMLElement, serverUrl: string, search: string): Promise<void> {
  try {
    const rows: Product[] = await listProducts(serverUrl, search || undefined, PAGE_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = `<p class="empty">Sin productos para «${escapeHtml(search)}».</p>`;
      return;
    }
    host.innerHTML = `
      <table class="data-table">
        <thead>
          <tr><th>Producto</th><th class="num">Precio</th><th class="num">Stock</th><th>Estado</th></tr>
        </thead>
        <tbody>
          ${rows.map(productRow).join("")}
        </tbody>
      </table>
      <p class="table-foot muted">${rows.length} producto(s)${
        rows.length === PAGE_LIMIT ? ` · mostrando los primeros ${PAGE_LIMIT}` : ""
      }</p>
    `;
  } catch (err) {
    host.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }
}

function productRow(p: Product): string {
  const state =
    p.stock <= 0
      ? `<span class="pill pill-danger">Agotado</span>`
      : p.stock <= 5
        ? `<span class="pill pill-warn">Bajo</span>`
        : `<span class="pill pill-ok">OK</span>`;
  const sub = p.laboratory || p.active_ingredient || "";
  return `
    <tr>
      <td>
        <div class="cell-main">${escapeHtml(p.name)}</div>
        ${sub ? `<div class="cell-sub muted">${escapeHtml(sub)}</div>` : ""}
      </td>
      <td class="num">${clp(p.price)}</td>
      <td class="num">${num(p.stock)}</td>
      <td>${state}</td>
    </tr>
  `;
}

// --- shared bits reused by other views ------------------------------------

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

export function asMessage(err: unknown): string {
  return typeof err === "string" ? err : "No se pudo cargar la información.";
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : c === '"' ? "&quot;" : "&#39;",
  );
}
