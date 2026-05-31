// AppShell — the ERP frame the launcher hands off to after login. Persistent
// sidebar (nav) + topbar (tenant, license tier, live health) + a swappable
// content host. `dispatchNav` is the in-shell router: each nav id maps to a
// view module that renders into `#view-host`. Views fetch their own data via
// the typed wrappers in ../api (which call the Rust Tauri commands).
import {
  licenseStatus,
  serverHealth,
  logout,
  type SessionInfo,
  type LicenseSummary,
  type HealthInfo,
} from "../api";
import { renderInventory } from "./inventory";
import { renderPos } from "./pos";
import { renderReports } from "./reports";
import { renderDashboard } from "./dashboard";
import { renderCaja } from "./caja";
import { renderClientes } from "./clientes";
import { renderCompras } from "./compras";
import { renderRecetas } from "./recetas";
import { renderDevoluciones } from "./devoluciones";
import { renderAuditoria } from "./auditoria";
import { renderGastos } from "./gastos";
import { renderConfiguracion } from "./configuracion";
import { renderImportar } from "./importar";

const NAV = [
  { id: "dashboard", label: "Panel", hint: "Resumen ejecutivo" },
  { id: "pos", label: "POS", hint: "Punto de venta" },
  { id: "devoluciones", label: "Devoluciones", hint: "Reembolsos de ventas" },
  { id: "inventory", label: "Inventario", hint: "Stock y lotes" },
  { id: "importar", label: "Importar", hint: "Carga masiva CSV" },
  { id: "caja", label: "Caja", hint: "Apertura y arqueo" },
  { id: "clientes", label: "Clientes", hint: "Búsqueda y fidelidad" },
  { id: "compras", label: "Compras", hint: "Órdenes a proveedores" },
  { id: "recetas", label: "Recetas", hint: "Controlados y libro" },
  { id: "gastos", label: "Gastos", hint: "Egresos y caja chica" },
  { id: "reports", label: "Reportes", hint: "Ventas y márgenes" },
  { id: "auditoria", label: "Auditoría", hint: "Registro inmutable" },
  { id: "configuracion", label: "Configuración", hint: "Parámetros del servidor" },
] as const;

type NavId = (typeof NAV)[number]["id"];

function tierClass(tier: string): string {
  switch (tier.toLowerCase()) {
    case "enterprise":
      return "tier-enterprise";
    case "business":
      return "tier-business";
    case "pro":
      return "tier-pro";
    default:
      return "tier-free";
  }
}

// Server /me returns the raw tenant record id (e.g. "tenant:pp3v…"). Show the
// friendly slug captured at login instead, title-cased.
function prettyTenant(tenantId: string): string {
  const slug = (() => {
    try { return sessionStorage.getItem("tenant_slug"); } catch { return null; }
  })();
  if (slug) {
    return slug
      .split(/[-_]/)
      .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
      .join(" ");
  }
  return tenantId.replace(/^tenant:/, "");
}

function healthDot(h: HealthInfo | null): string {
  if (!h) return `<span class="dot dot-unknown"></span> Comprobando…`;
  if (!h.reachable) return `<span class="dot dot-down"></span> Sin conexión`;
  if (h.status === "ok") return `<span class="dot dot-ok"></span> Operativo`;
  return `<span class="dot dot-warn"></span> Degradado (db: ${h.db})`;
}

/** Route a nav id to its view, rendering into the content host. Panel
 *  (Dashboard) is the landing view (core ERP, always available on every tier). */
function dispatchNav(host: HTMLElement, id: NavId, serverUrl: string): void {
  switch (id) {
    case "dashboard":
      renderDashboard(host, serverUrl);
      break;
    case "pos":
      renderPos(host, serverUrl);
      break;
    case "devoluciones":
      renderDevoluciones(host, serverUrl);
      break;
    case "inventory":
      renderInventory(host, serverUrl);
      break;
    case "caja":
      renderCaja(host, serverUrl);
      break;
    case "clientes":
      renderClientes(host, serverUrl);
      break;
    case "compras":
      renderCompras(host, serverUrl);
      break;
    case "recetas":
      renderRecetas(host, serverUrl);
      break;
    case "gastos":
      renderGastos(host, serverUrl);
      break;
    case "reports":
      renderReports(host, serverUrl);
      break;
    case "auditoria":
      renderAuditoria(host, serverUrl);
      break;
    case "configuracion":
      renderConfiguracion(host, serverUrl);
      break;
    case "importar":
      renderImportar(host, serverUrl);
      break;
  }
}

export function renderShell(
  root: HTMLElement,
  session: SessionInfo,
  serverUrl: string,
  onLogout: () => void,
): void {
  // Landing view = Panel (Dashboard) so the operator opens onto an exec overview
  // of the day rather than an empty POS cart or a raw table.
  const LANDING: NavId = "dashboard";

  root.innerHTML = `
    <div class="shell">
      <aside class="sidebar">
        <div class="sidebar-brand">
          <img src="/tu-farmacia-logo.jpeg" alt="" width="26" height="26" />
          <span>Tu Farmacia</span>
        </div>
        <nav id="nav">
          ${NAV.map(
            (n) => `
            <button class="nav-item ${n.id === LANDING ? "active" : ""}" data-nav="${n.id}">
              <span class="nav-label">${n.label}</span>
              <span class="nav-hint">${n.hint}</span>
            </button>`,
          ).join("")}
        </nav>
        <button id="logout" class="nav-logout">Cerrar sesión</button>
      </aside>

      <main class="content">
        <header class="topbar">
          <div class="topbar-id">
            <span class="muted">Sucursal</span>
            <strong>${prettyTenant(session.tenant_id)}</strong>
          </div>
          <div class="topbar-meta">
            <span class="muted topbar-user">${session.user_id}</span>
            <span id="tier-badge" class="badge tier-free">…</span>
            <span id="health" class="health">${healthDot(null)}</span>
          </div>
        </header>

        <div id="view-host"></div>
      </main>
    </div>
  `;

  const host = root.querySelector<HTMLDivElement>("#view-host")!;
  const navButtons = Array.from(root.querySelectorAll<HTMLButtonElement>(".nav-item"));
  navButtons.forEach((b) => {
    b.addEventListener("click", () => {
      if (b.classList.contains("active")) return; // already here — no re-fetch
      navButtons.forEach((x) => x.classList.remove("active"));
      b.classList.add("active");
      dispatchNav(host, b.dataset.nav as NavId, serverUrl);
    });
  });

  root.querySelector<HTMLButtonElement>("#logout")!.addEventListener("click", async () => {
    try {
      await logout();
    } catch {
      // even if the backend logout fails, drop back to login locally
    }
    onLogout();
  });

  // Initial view + topbar hydration.
  dispatchNav(host, LANDING, serverUrl);
  void hydrateLicense(root, serverUrl);
  void hydrateHealth(root, serverUrl);
}

async function hydrateLicense(root: HTMLElement, serverUrl: string): Promise<void> {
  const badge = root.querySelector<HTMLSpanElement>("#tier-badge")!;
  try {
    const lic: LicenseSummary = await licenseStatus(serverUrl);
    badge.textContent = lic.tier.toUpperCase();
    badge.className = `badge ${tierClass(lic.tier)}`;
    const exp = lic.expires_at ? ` · vence ${lic.expires_at.slice(0, 10)}` : "";
    badge.title = `${lic.status} · ${lic.seat_count} asiento(s)${exp}`;
  } catch (err) {
    badge.textContent = "N/D";
    badge.className = "badge tier-free";
    badge.title = typeof err === "string" ? err : "Licencia no disponible";
  }
}

async function hydrateHealth(root: HTMLElement, serverUrl: string): Promise<void> {
  const el = root.querySelector<HTMLSpanElement>("#health")!;
  const tick = async () => {
    try {
      const h = await serverHealth(serverUrl);
      el.innerHTML = healthDot(h);
    } catch {
      el.innerHTML = healthDot({ status: "down", db: "—", reachable: false });
    }
  };
  await tick();
  // Light poll so the operator sees the server drop without a manual refresh.
  window.setInterval(tick, 15000);
}
