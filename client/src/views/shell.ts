// AppShell — phase 2 placeholder. Shows tenant, license tier badge, and live
// server status. Nav items (POS / Inventario / Reportes) are stubs for later
// waves. This is the "produced game" the launcher hands off to.
import {
  licenseStatus,
  serverHealth,
  logout,
  type SessionInfo,
  type LicenseSummary,
  type HealthInfo,
} from "../api";

const NAV = [
  { id: "pos", label: "POS", hint: "Punto de venta" },
  { id: "inventory", label: "Inventario", hint: "Stock y lotes" },
  { id: "reports", label: "Reportes", hint: "Ventas y márgenes" },
];

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

function healthDot(h: HealthInfo | null): string {
  if (!h) return `<span class="dot dot-unknown"></span> Comprobando…`;
  if (!h.reachable) return `<span class="dot dot-down"></span> Sin conexión`;
  if (h.status === "ok") return `<span class="dot dot-ok"></span> Operativo`;
  return `<span class="dot dot-warn"></span> Degradado (db: ${h.db})`;
}

export function renderShell(
  root: HTMLElement,
  session: SessionInfo,
  serverUrl: string,
  onLogout: () => void,
): void {
  root.innerHTML = `
    <div class="shell">
      <aside class="sidebar">
        <div class="sidebar-brand">PHARMA</div>
        <nav id="nav">
          ${NAV.map(
            (n, i) => `
            <button class="nav-item ${i === 0 ? "active" : ""}" data-nav="${n.id}">
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
            <strong>${session.tenant_id}</strong>
          </div>
          <div class="topbar-meta">
            <span id="tier-badge" class="badge tier-free">…</span>
            <span id="health" class="health">${healthDot(null)}</span>
          </div>
        </header>

        <section class="panel">
          <h2 id="panel-title">POS</h2>
          <p class="muted">Módulo en construcción. Esta es la base del shell ERP — las vistas reales llegan en olas posteriores.</p>
          <div class="info-grid">
            <div class="info-card">
              <span class="muted">Usuario</span>
              <strong>${session.user_id}</strong>
            </div>
            <div class="info-card">
              <span class="muted">Roles</span>
              <strong>${session.roles.length ? session.roles.join(", ") : "—"}</strong>
            </div>
            <div class="info-card">
              <span class="muted">Servidor</span>
              <strong>${serverUrl}</strong>
            </div>
            <div class="info-card" id="license-card">
              <span class="muted">Licencia</span>
              <strong id="license-detail">Cargando…</strong>
            </div>
          </div>
        </section>
      </main>
    </div>
  `;

  // Nav stubs: swap the panel title only (no real routing yet).
  const navButtons = Array.from(root.querySelectorAll<HTMLButtonElement>(".nav-item"));
  const title = root.querySelector<HTMLHeadingElement>("#panel-title")!;
  navButtons.forEach((b) => {
    b.addEventListener("click", () => {
      navButtons.forEach((x) => x.classList.remove("active"));
      b.classList.add("active");
      title.textContent = b.querySelector(".nav-label")?.textContent ?? "";
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

  void hydrateLicense(root, serverUrl);
  void hydrateHealth(root, serverUrl);
}

async function hydrateLicense(root: HTMLElement, serverUrl: string): Promise<void> {
  const badge = root.querySelector<HTMLSpanElement>("#tier-badge")!;
  const detail = root.querySelector<HTMLElement>("#license-detail")!;
  try {
    const lic: LicenseSummary = await licenseStatus(serverUrl);
    badge.textContent = lic.tier.toUpperCase();
    badge.className = `badge ${tierClass(lic.tier)}`;
    const exp = lic.expires_at ? ` · vence ${lic.expires_at.slice(0, 10)}` : "";
    detail.textContent = `${lic.status} · ${lic.seat_count} asiento(s)${exp}`;
  } catch (err) {
    badge.textContent = "N/D";
    badge.className = "badge tier-free";
    detail.textContent = typeof err === "string" ? err : "No disponible";
  }
}

async function hydrateHealth(root: HTMLElement, serverUrl: string): Promise<void> {
  const el = root.querySelector<HTMLSpanElement>("#health")!;
  try {
    const h = await serverHealth(serverUrl);
    el.innerHTML = healthDot(h);
  } catch {
    el.innerHTML = healthDot({ status: "down", db: "—", reachable: false });
  }
}
