// Configuración — the configuration HUB. A side-nav of human sections (Negocio,
// Facturación SII, Licencia, Agente, Preferencias + honest "próximamente" mounts
// for Usuarios/Sucursales/Respaldo) over a content panel, with an in-settings
// search that filters the nav and jumps to a field. The operator NEVER sees a
// raw `admin_setting` key — every control wears a Spanish label. The section
// catalog, search, save-state machine and validators live DOM-free in
// ./config-center (unit-tested); this file is the renderer + wiring.
import {
  getSetting,
  setSetting,
  serverHealth,
  seedDemo,
  storedServerUrl,
  rememberServerUrl,
  licenseStatus,
  dteCafStatus,
  listUsers,
  createUser,
  updateUser,
  SEED_ALREADY_EXISTS,
  type LicenseSummary,
  type CafStatus,
} from "../api";
import { isValidRut, formatRut } from "../format";
import {
  VERTICAL_KEY,
  BUSINESS_NAME_KEY,
  RUBRO_CATALOG,
  seedVerticalFor,
  parseRubro,
  type SeedVertical,
} from "../vertical";
import { rubroPreview } from "./first-run";
import { rubroIconSvg } from "../brand/rubro-icons";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";
import {
  CONFIG_SECTIONS,
  resolveSection,
  searchConfig,
  saveStatusClass,
  toSaving,
  toSaved,
  toFailed,
  validateBusinessRut,
  validateRequiredText,
  validateNonNegativeInt,
  validateActeco,
  type SectionId,
  type SaveState,
} from "./config-center";

// --- setting keys (technical — never surfaced to the operator) --------------
const EMISOR_KEY = "dte.emisor";
const SII_ENV_KEY = "dte.sii_env";
const FEDERATION_KEY = "federation_enabled";
const LOYALTY_KEY = "loyalty_points_per_clp";
const TELEMETRY_KEY = "telemetry_enabled";

interface EmisorField {
  name: string;
  label: string;
  placeholder: string;
  required: boolean;
}

const EMISOR_FIELDS: readonly EmisorField[] = [
  { name: "rut", label: "RUT empresa", placeholder: "76123456-7", required: true },
  { name: "razon_social", label: "Razón social", placeholder: "Mi Empresa SpA", required: true },
  { name: "giro", label: "Giro", placeholder: "Venta al por menor en comercios especializados", required: true },
  { name: "direccion", label: "Dirección", placeholder: "Av. Principal 123", required: true },
  { name: "comuna", label: "Comuna", placeholder: "Coquimbo", required: true },
  { name: "ciudad", label: "Ciudad (opcional)", placeholder: "Coquimbo", required: false },
] as const;

// Human labels for license features — the operator reads "Reporte de márgenes",
// not `reports.margins_daily`. Unknown features fall back to a de-emphasised raw
// id (forward-compatible: a new server feature still renders, just unlabelled).
const FEATURE_LABELS: Record<string, string> = {
  "reports.margins_daily": "Reporte de márgenes diario",
  "reports.top_products": "Top de productos",
  "reports.stock_rotation": "Rotación de inventario",
  "dte.sii": "Boleta y factura electrónica (SII)",
  "federation.quote": "Federación — cotizaciones B2B",
  "federation.po": "Federación — órdenes de compra B2B",
  "sync.online": "Sincronización en línea",
  "branding.pack": "Personalización de marca",
};

// Minimal line icons for the nav (self-hosted, no CDN). Keyed by section.icon.
function sectionIconSvg(id: string): string {
  const paths: Record<string, string> = {
    store: '<path d="M3 9l1.5-5h15L21 9"/><path d="M4 9v10h16V9"/><path d="M9 19v-5h6v5"/>',
    receipt: '<path d="M5 3v18l2-1.5L9 21l2-1.5L13 21l2-1.5L17 21l2-1.5V3l-2 1.5L15 3l-2 1.5L11 3 9 4.5 7 3 5 4.5"/><path d="M8 8h8M8 12h8"/>',
    key: '<circle cx="8" cy="8" r="4"/><path d="M11 11l9 9M17 17l2-2M14 14l2-2"/>',
    robot: '<rect x="4" y="8" width="16" height="11" rx="2"/><path d="M12 8V4M9 13h.01M15 13h.01M9 17h6"/>',
    sliders: '<path d="M4 6h10M18 6h2M4 12h2M10 12h10M4 18h8M16 18h4"/><circle cx="16" cy="6" r="2"/><circle cx="8" cy="12" r="2"/><circle cx="14" cy="18" r="2"/>',
    users: '<circle cx="9" cy="8" r="3"/><path d="M3 20c0-3 3-5 6-5s6 2 6 5"/><path d="M16 5a3 3 0 0 1 0 6M21 20c0-2-1.5-3.5-3.5-4.2"/>',
    building: '<rect x="5" y="3" width="14" height="18" rx="1"/><path d="M9 7h.01M12 7h.01M15 7h.01M9 11h.01M12 11h.01M15 11h.01M10 21v-4h4v4"/>',
    shield: '<path d="M12 3l7 3v6c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6z"/><path d="M9 12l2 2 4-4"/>',
  };
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths[id] ?? paths.sliders}</svg>`;
}

/** Apply a {@link SaveState} to an inline status element (uniform feedback). */
function applyStatus(el: HTMLElement, state: SaveState): void {
  el.className = saveStatusClass(state);
  if (state.message) {
    el.textContent = state.message;
    el.hidden = false;
  } else {
    el.hidden = true;
  }
}

function readBool(setting: { value: string } | null, fallback = false): boolean {
  if (!setting) return fallback;
  return setting.value === "true";
}

export function renderConfiguracion(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-configuracion cfg-hub">
      <div class="view-head">
        <div>
          <h2>Configuración</h2>
          <p class="muted">Todo lo que define tu negocio, en un solo lugar. Sólo administradores pueden guardar cambios.</p>
        </div>
      </div>

      <div class="cfg-search-wrap">
        <input id="cfg-search" class="rb-input cfg-search" type="search" autocomplete="off" spellcheck="false"
               placeholder="Buscar un ajuste… (rubro, folios, tema, respaldo…)" aria-label="Buscar en configuración" />
        <div id="cfg-search-hits" class="cfg-search-hits" hidden></div>
      </div>

      <div class="cfg-hub-body">
        <nav class="cfg-nav" id="cfg-nav" aria-label="Secciones de configuración">
          ${CONFIG_SECTIONS.map(
            (s) => `
            <button class="cfg-nav-item" type="button" data-section="${s.id}" role="tab">
              <span class="cfg-nav-icon" aria-hidden="true">${sectionIconSvg(s.icon)}</span>
              <span class="cfg-nav-text">
                <span class="cfg-nav-label">${escapeHtml(s.label)}</span>
                <span class="cfg-nav-blurb">${escapeHtml(s.blurb)}</span>
              </span>
              ${s.placeholder ? `<span class="cfg-nav-soon">pronto</span>` : ""}
            </button>`,
          ).join("")}
        </nav>
        <div class="cfg-panel" id="cfg-panel" role="tabpanel"></div>
      </div>

      <div id="cfg-toast" class="toast" hidden></div>
    </section>
  `;

  const navEl = host.querySelector<HTMLElement>("#cfg-nav")!;
  const panelEl = host.querySelector<HTMLElement>("#cfg-panel")!;
  const searchEl = host.querySelector<HTMLInputElement>("#cfg-search")!;
  const hitsEl = host.querySelector<HTMLElement>("#cfg-search-hits")!;
  const toastEl = host.querySelector<HTMLElement>("#cfg-toast")!;
  const navButtons = Array.from(navEl.querySelectorAll<HTMLButtonElement>(".cfg-nav-item"));

  let activeSection: SectionId = resolveSection(null);

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  function selectSection(id: SectionId): void {
    activeSection = id;
    navButtons.forEach((b) => {
      const on = b.dataset.section === id;
      b.classList.toggle("active", on);
      b.setAttribute("aria-selected", String(on));
    });
    renderSection(id);
    panelEl.scrollTop = 0;
  }

  navButtons.forEach((b) => {
    b.addEventListener("click", () => selectSection(b.dataset.section as SectionId));
  });

  // In-settings search: filter the nav to matching sections and offer "jump to
  // field" chips. If the active section is filtered away, hop to the first hit.
  function applySearch(): void {
    const res = searchConfig(searchEl.value);
    navButtons.forEach((b) => {
      b.hidden = !res.sectionIds.includes(b.dataset.section as SectionId);
    });
    if (!res.filtered) {
      hitsEl.hidden = true;
      hitsEl.innerHTML = "";
      return;
    }
    if (res.fieldHits.length) {
      hitsEl.hidden = false;
      hitsEl.innerHTML = res.fieldHits
        .slice(0, 10)
        .map(
          (h) =>
            `<button class="cfg-hit-chip" type="button" data-section="${h.section}">${escapeHtml(h.label)}</button>`,
        )
        .join("");
      hitsEl.querySelectorAll<HTMLButtonElement>(".cfg-hit-chip").forEach((c) => {
        c.addEventListener("click", () => {
          searchEl.value = "";
          applySearch();
          selectSection(c.dataset.section as SectionId);
        });
      });
    } else if (res.sectionIds.length === 0) {
      hitsEl.hidden = false;
      hitsEl.innerHTML = `<span class="muted cfg-no-hits">Sin resultados para "${escapeHtml(searchEl.value.trim())}".</span>`;
    } else {
      hitsEl.hidden = true;
      hitsEl.innerHTML = "";
    }
    if (res.sectionIds.length && !res.sectionIds.includes(activeSection)) {
      selectSection(res.sectionIds[0]);
    }
  }
  searchEl.addEventListener("input", applySearch);

  // --- section dispatcher ----------------------------------------------------
  function renderSection(id: SectionId): void {
    switch (id) {
      case "negocio":
        renderNegocio();
        break;
      case "sii":
        renderSii();
        break;
      case "licencia":
        renderLicencia();
        break;
      case "agente":
        renderAgente();
        break;
      case "preferencias":
        renderPreferencias();
        break;
      case "usuarios":
        renderUsuarios();
        break;
      default:
        renderPlaceholder(id);
        break;
    }
  }

  function sectionHead(id: SectionId): string {
    const s = CONFIG_SECTIONS.find((x) => x.id === id)!;
    return `
      <header class="cfg-sec-head">
        <span class="cfg-sec-icon" aria-hidden="true">${sectionIconSvg(s.icon)}</span>
        <div>
          <h3 class="cfg-sec-title">${escapeHtml(s.label)}</h3>
          <p class="muted">${escapeHtml(s.blurb)}</p>
        </div>
      </header>`;
  }

  // --- Negocio: rubro + nombre + identidad tributaria ------------------------
  function renderNegocio(): void {
    panelEl.innerHTML = `
      ${sectionHead("negocio")}
      <div class="cfg-card">
        <h4 class="cfg-card-title">Rubro y nombre</h4>
        <p class="muted cfg-help">Elige el rubro de tu negocio. Define qué secciones del ERP se muestran: Recetas y el Libro de controlados (Ley 20.000) sólo aparecen en Farmacia. Las boletas y facturas (SII) funcionan en todos los rubros.</p>
        <div id="cfg-vertical">${tableSkeleton(2)}</div>
      </div>
      <div class="cfg-card">
        <h4 class="cfg-card-title">Datos tributarios</h4>
        <p class="muted cfg-help">Identidad con la que se firman tus documentos ante el SII. Requerido antes de emitir el primer DTE.</p>
        <div id="cfg-emisor">${tableSkeleton(3)}</div>
      </div>
    `;
    void loadVerticalForm();
    void loadEmisor();
  }

  // --- Facturación SII: ambiente + folios + certificado ----------------------
  function renderSii(): void {
    panelEl.innerHTML = `
      ${sectionHead("sii")}
      <div class="cfg-card">
        <h4 class="cfg-card-title">Ambiente del SII</h4>
        <p class="muted cfg-help">Certificación (sandbox) para pruebas; Producción para documentos con validez tributaria real.</p>
        <div id="cfg-sii-env">${tableSkeleton(1)}</div>
      </div>
      <div class="cfg-card">
        <h4 class="cfg-card-title">Folios disponibles (CAF)</h4>
        <p class="muted cfg-help">Folios autorizados restantes para boleta electrónica (tipo 39).</p>
        <div id="cfg-caf">${tableSkeleton(2)}</div>
      </div>
      <div class="cfg-card cfg-card-soon">
        <h4 class="cfg-card-title">Certificado y archivos CAF <span class="cfg-soon-chip">próximamente</span></h4>
        <p class="muted cfg-help">La carga del certificado digital (.pfx) y de los archivos CAF desde esta pantalla llegará pronto. Por ahora se cargan en el servidor durante la puesta en marcha.</p>
      </div>
    `;
    void loadSiiEnv();
    void loadCaf();
  }

  // --- Licencia y plan -------------------------------------------------------
  function renderLicencia(): void {
    panelEl.innerHTML = `
      ${sectionHead("licencia")}
      <div class="cfg-card" id="cfg-license">${tableSkeleton(3)}</div>
    `;
    void loadLicense();
  }

  // --- Agente ----------------------------------------------------------------
  function renderAgente(): void {
    panelEl.innerHTML = `
      ${sectionHead("agente")}
      <div class="cfg-card" id="cfg-agent">${tableSkeleton(2)}</div>
      <div class="cfg-card cfg-card-soon">
        <h4 class="cfg-card-title">Asistente con IA <span class="cfg-soon-chip">próximamente</span></h4>
        <p class="muted cfg-help">Tu agente ya responde con los datos de tu negocio sin conexión. La conexión opcional a un modelo de lenguaje (opt-in, lo decides tú) llegará en una próxima versión.</p>
      </div>
    `;
    void loadAgent();
  }

  // --- Preferencias ----------------------------------------------------------
  function renderPreferencias(): void {
    panelEl.innerHTML = `
      ${sectionHead("preferencias")}
      <div class="cfg-card">
        <h4 class="cfg-card-title">Apariencia e idioma</h4>
        <div class="cfg-emisor-form rb">
          <div class="field">
            <label class="rb-label" for="cfg-theme">Tema</label>
            <select id="cfg-theme" class="rb-input">
              <option value="dark">Oscuro</option>
              <option value="light">Claro</option>
            </select>
          </div>
          <div class="field">
            <label class="rb-label" for="cfg-lang">Idioma</label>
            <select id="cfg-lang" class="rb-input" disabled title="Por ahora sólo español de Chile">
              <option value="es-CL" selected>Español (Chile)</option>
            </select>
            <p class="muted cfg-help">Más idiomas próximamente.</p>
          </div>
        </div>
      </div>
      <div class="cfg-card">
        <h4 class="cfg-card-title">Conexión al servidor</h4>
        <p class="muted cfg-help">Apunta este terminal al servidor de tu local. <code>localhost</code> si el servidor corre en este mismo equipo; la IP de red (ej: <code>http://192.168.1.50:8080</code>) si usas una tablet u otro equipo como caja. Se aplica al volver a iniciar sesión.</p>
        <div class="cfg-emisor-form rb">
          <div class="field">
            <label class="rb-label" for="cfg-server-url">URL del servidor</label>
            <input id="cfg-server-url" class="rb-input" type="text" spellcheck="false"
                   placeholder="http://127.0.0.1:8080" value="${escapeHtml(storedServerUrl())}" autocomplete="off" />
            <p class="muted cfg-help">Sesión actual: <code>${escapeHtml(serverUrl)}</code>.</p>
          </div>
          <div class="cfg-edit">
            <button class="rb-btn ghost" id="cfg-server-test" type="button">Probar conexión</button>
            <button class="rb-btn" id="cfg-server-save" type="button">Guardar URL</button>
            <span class="cfg-status" id="cfg-server-status" hidden></span>
          </div>
        </div>
      </div>
      <div class="cfg-card">
        <h4 class="cfg-card-title">Telemetría anónima</h4>
        <p class="muted cfg-help">Estadísticas de uso anónimas para mejorar el producto. <strong>Desactivada por defecto</strong>, sin datos personales (Ley 19.628). Tú decides si la activas.</p>
        <div id="cfg-telemetry">${tableSkeleton(1)}</div>
      </div>
    `;
    wireServerAndTheme();
    void loadTelemetry();
  }

  // --- Usuarios y roles: gestión de credenciales del negocio -----------------
  const ROLE_LABELS: Record<string, string> = {
    owner: "Dueño",
    admin: "Administrador",
    pharmacist: "Químico",
    cashier: "Cajero",
  };
  const ASSIGNABLE_ROLES = ["cashier", "pharmacist", "admin", "owner"] as const;
  const roleLabel = (r: string): string => ROLE_LABELS[r] ?? r;

  function renderUsuarios(): void {
    panelEl.innerHTML = `
      ${sectionHead("usuarios")}
      <div class="cfg-card">
        <h4 class="cfg-card-title">Crear usuario</h4>
        <p class="muted cfg-help">Crea una credencial para tu equipo. Cada persona entra con su propio correo y contraseña; el rol define qué puede hacer dentro del sistema.</p>
        <div class="cfg-emisor-form rb">
          <div class="field">
            <label class="rb-label" for="cfg-user-email">Correo</label>
            <input id="cfg-user-email" class="rb-input" type="email" autocomplete="off" spellcheck="false" placeholder="cajero@minegocio.cl" />
          </div>
          <div class="field">
            <label class="rb-label" for="cfg-user-pass">Contraseña</label>
            <input id="cfg-user-pass" class="rb-input" type="password" autocomplete="new-password" placeholder="Mínimo 8 caracteres" />
          </div>
          <div class="field">
            <span class="rb-label">Rol</span>
            <div class="cfg-roles" role="group" aria-label="Roles del usuario">
              ${ASSIGNABLE_ROLES.map(
                (r) => `
                <label class="cfg-role-opt">
                  <input type="checkbox" value="${r}"${r === "cashier" ? " checked" : ""} />
                  <span>${escapeHtml(roleLabel(r))}</span>
                </label>`,
              ).join("")}
            </div>
            <p class="muted cfg-help">Cajero: venta y caja · Químico: + recetas y controlados · Administrador: + configuración y usuarios · Dueño: control total.</p>
          </div>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-user-create" type="button">Crear usuario</button>
            <span class="cfg-status" id="cfg-user-status" hidden></span>
          </div>
        </div>
      </div>
      <div class="cfg-card">
        <h4 class="cfg-card-title">Usuarios del negocio</h4>
        <p class="muted cfg-help">Quienes pueden iniciar sesión en este negocio. Puedes desactivar a alguien (deja de entrar, sin borrar su historial) o cambiarle la contraseña.</p>
        <div id="cfg-users-list">${tableSkeleton(3)}</div>
      </div>
    `;
    wireUserCreate();
    void loadUsers();
  }

  function wireUserCreate(): void {
    const emailEl = panelEl.querySelector<HTMLInputElement>("#cfg-user-email");
    const passEl = panelEl.querySelector<HTMLInputElement>("#cfg-user-pass");
    const btn = panelEl.querySelector<HTMLButtonElement>("#cfg-user-create");
    const statusEl = panelEl.querySelector<HTMLElement>("#cfg-user-status");
    if (!emailEl || !passEl || !btn || !statusEl) return;
    btn.addEventListener("click", async () => {
      const email = emailEl.value.trim();
      const password = passEl.value;
      const roles = Array.from(
        panelEl.querySelectorAll<HTMLInputElement>('.cfg-roles input[type="checkbox"]'),
      )
        .filter((c) => c.checked)
        .map((c) => c.value);
      if (!email) {
        applyStatus(statusEl, toFailed("Indica el correo del usuario."));
        return;
      }
      if (password.length < 8) {
        applyStatus(statusEl, toFailed("La contraseña debe tener al menos 8 caracteres."));
        return;
      }
      if (roles.length === 0) {
        applyStatus(statusEl, toFailed("Elige al menos un rol."));
        return;
      }
      btn.disabled = true;
      applyStatus(statusEl, toSaving());
      try {
        await createUser(serverUrl, email, password, roles);
        applyStatus(statusEl, toSaved("Usuario creado."));
        toast(`Usuario ${email} creado.`);
        emailEl.value = "";
        passEl.value = "";
        await loadUsers();
      } catch (err) {
        applyStatus(statusEl, toFailed(asMessage(err)));
      } finally {
        btn.disabled = false;
      }
    });
  }

  async function loadUsers(): Promise<void> {
    const listEl = panelEl.querySelector<HTMLElement>("#cfg-users-list");
    if (!listEl) return;
    listEl.innerHTML = tableSkeleton(3);
    try {
      const users = await listUsers(serverUrl);
      if (users.length === 0) {
        listEl.innerHTML = `<p class="muted cfg-help">Aún no hay usuarios. Crea el primero con el formulario de arriba.</p>`;
        return;
      }
      listEl.innerHTML = `
        <div class="cfg-users-list">
          ${users
            .map(
              (u) => `
            <div class="cfg-user-row${u.active ? "" : " off"}" data-id="${escapeHtml(u.id)}" data-active="${u.active}">
              <div class="cfg-user-id">
                <span class="cfg-user-email">${escapeHtml(u.email)}</span>
                <span class="cfg-user-roles">${u.roles
                  .map((r) => `<span class="cfg-role-badge">${escapeHtml(roleLabel(r))}</span>`)
                  .join("")}</span>
              </div>
              <div class="cfg-user-side">
                <span class="cfg-user-state ${u.active ? "on" : "off"}">${u.active ? "Activo" : "Inactivo"}</span>
                <div class="cfg-user-actions">
                  <button class="rb-btn ghost sm" data-act="reset" type="button">Cambiar clave</button>
                  <button class="rb-btn sm${u.active ? " danger" : ""}" data-act="toggle" type="button">${u.active ? "Desactivar" : "Activar"}</button>
                </div>
              </div>
            </div>`,
            )
            .join("")}
        </div>
      `;
      wireUserRows(listEl);
    } catch (err) {
      listEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function wireUserRows(listEl: HTMLElement): void {
    listEl.querySelectorAll<HTMLButtonElement>("button[data-act]").forEach((btn) => {
      btn.addEventListener("click", async () => {
        const row = btn.closest<HTMLElement>(".cfg-user-row");
        if (!row) return;
        const id = row.dataset.id ?? "";
        const active = row.dataset.active === "true";
        if (btn.dataset.act === "toggle") {
          if (
            active &&
            !window.confirm(
              "¿Desactivar este usuario? No podrá iniciar sesión hasta que lo reactives. Su historial se conserva.",
            )
          )
            return;
          btn.disabled = true;
          try {
            await updateUser(serverUrl, id, { active: !active });
            toast(active ? "Usuario desactivado." : "Usuario activado.");
            await loadUsers();
          } catch (err) {
            toast(asMessage(err));
            btn.disabled = false;
          }
        } else if (btn.dataset.act === "reset") {
          openResetInline(row, id);
        }
      });
    });
  }

  function openResetInline(row: HTMLElement, id: string): void {
    if (row.querySelector(".cfg-reset-inline")) return;
    const box = document.createElement("div");
    box.className = "cfg-reset-inline";
    box.innerHTML = `
      <input class="rb-input" type="password" autocomplete="new-password" placeholder="Nueva contraseña (mín. 8)" aria-label="Nueva contraseña" />
      <button class="rb-btn sm" data-reset-save type="button">Guardar</button>
      <button class="rb-btn ghost sm" data-reset-cancel type="button">Cancelar</button>
      <span class="cfg-status" hidden></span>
    `;
    row.appendChild(box);
    const input = box.querySelector<HTMLInputElement>("input");
    const status = box.querySelector<HTMLElement>(".cfg-status");
    const saveBtn = box.querySelector<HTMLButtonElement>("[data-reset-save]");
    const cancelBtn = box.querySelector<HTMLButtonElement>("[data-reset-cancel]");
    if (!input || !status || !saveBtn || !cancelBtn) return;
    input.focus();
    cancelBtn.addEventListener("click", () => box.remove());
    saveBtn.addEventListener("click", async () => {
      const pw = input.value;
      if (pw.length < 8) {
        applyStatus(status, toFailed("Mínimo 8 caracteres."));
        return;
      }
      saveBtn.disabled = true;
      applyStatus(status, toSaving());
      try {
        await updateUser(serverUrl, id, { password: pw });
        toast("Contraseña actualizada.");
        box.remove();
      } catch (err) {
        applyStatus(status, toFailed(asMessage(err)));
        saveBtn.disabled = false;
      }
    });
  }

  // --- Placeholder sections (honest "próximamente", never a dead-end) --------
  const PLACEHOLDER_COPY: Record<string, { what: string; today: string }> = {
    sucursales: {
      what: "Administrar varias sucursales y puntos de caja desde una sola instalación.",
      today: "Tu instalación ya opera una sucursal. El alta de sucursales y cajas adicionales desde la UI llega pronto.",
    },
    respaldo: {
      what: "Programar respaldos automáticos, ejecutarlos ahora y restaurar con una guía paso a paso.",
      today: "Hoy el respaldo se ejecuta por terminal: <code>pharma backup</code>. El respaldo programado y la restauración guiada llegan pronto.",
    },
  };

  function renderPlaceholder(id: SectionId): void {
    const copy = PLACEHOLDER_COPY[id];
    panelEl.innerHTML = `
      ${sectionHead(id)}
      <div class="cfg-card cfg-card-soon">
        <span class="cfg-soon-chip">próximamente</span>
        <p>${copy?.what ?? "Esta sección estará disponible pronto."}</p>
        <p class="muted cfg-help">${copy?.today ?? ""}</p>
      </div>
    `;
  }

  // ===========================================================================
  // Loaders + wiring (ported from the previous flat view; logic unchanged).
  // ===========================================================================

  // --- DTE emisor (identidad tributaria, sección Negocio) --------------------
  async function loadEmisor(): Promise<void> {
    const emisorEl = panelEl.querySelector<HTMLElement>("#cfg-emisor");
    if (!emisorEl) return;
    emisorEl.innerHTML = tableSkeleton(3);
    try {
      const emisorSetting = await getSetting(serverUrl, EMISOR_KEY);
      let current: Record<string, unknown> = {};
      if (emisorSetting) {
        try { current = JSON.parse(emisorSetting.value) as Record<string, unknown>; } catch { /* corrupt → empty form */ }
      }
      emisorEl.innerHTML = `
        <div class="cfg-emisor-form">
          ${EMISOR_FIELDS.map((f) => `
            <div class="field">
              <label for="cfg-em-${f.name}">${escapeHtml(f.label)}</label>
              <input id="cfg-em-${f.name}" type="text" placeholder="${escapeHtml(f.placeholder)}"
                     value="${escapeHtml(String(current[f.name] ?? ""))}" autocomplete="off" />
              ${f.name === "rut" ? `<div id="cfg-em-rut-hint" class="field-hint" hidden></div>` : ""}
            </div>`).join("")}
          <div class="field">
            <label for="cfg-em-acteco">Código actividad SII (acteco, opcional)</label>
            <input id="cfg-em-acteco" type="number" min="0" step="1" inputmode="numeric"
                   placeholder="477301" value="${current.acteco != null ? escapeHtml(String(current.acteco)) : ""}" />
          </div>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-em-save">Guardar datos tributarios</button>
            <span class="cfg-status" id="cfg-em-status" hidden></span>
          </div>
        </div>
      `;
      wireEmisor(emisorEl);
    } catch (err) {
      emisorEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function wireEmisor(emisorEl: HTMLElement): void {
    const saveBtn = emisorEl.querySelector<HTMLButtonElement>("#cfg-em-save")!;
    const statusEl = emisorEl.querySelector<HTMLElement>("#cfg-em-status")!;
    const rutEl = emisorEl.querySelector<HTMLInputElement>("#cfg-em-rut")!;
    const rutHint = emisorEl.querySelector<HTMLElement>("#cfg-em-rut-hint")!;

    // Live mód-11 echo (same UX as the Facturas receptor); canonicalisation +
    // the authoritative gate live in config-center.validateBusinessRut on save.
    const echoRut = (): void => {
      const raw = rutEl.value.trim();
      if (!raw) {
        rutEl.classList.remove("invalid");
        rutHint.hidden = true;
        rutHint.className = "field-hint";
        return;
      }
      if (isValidRut(raw)) {
        rutEl.classList.remove("invalid");
        rutHint.hidden = false;
        rutHint.className = "field-hint ok";
        rutHint.textContent = `RUT válido — ${formatRut(raw)}`;
      } else {
        rutEl.classList.add("invalid");
        rutHint.hidden = false;
        rutHint.className = "field-hint err";
        rutHint.textContent = "RUT inválido: revisa el dígito verificador.";
      }
    };
    rutEl.addEventListener("input", echoRut);
    rutEl.addEventListener("blur", () => {
      if (isValidRut(rutEl.value)) rutEl.value = formatRut(rutEl.value);
    });
    echoRut();

    saveBtn.addEventListener("click", async () => {
      const emisor: Record<string, unknown> = {};
      for (const f of EMISOR_FIELDS) {
        const raw = emisorEl.querySelector<HTMLInputElement>(`#cfg-em-${f.name}`)!.value;
        if (f.required) {
          const r = validateRequiredText(raw, f.label);
          if (!r.ok) {
            applyStatus(statusEl, toFailed(r.message!));
            return;
          }
          emisor[f.name] = r.value;
        } else if (raw.trim()) {
          emisor[f.name] = raw.trim();
        }
      }
      const rut = validateBusinessRut(rutEl.value);
      if (!rut.ok) {
        applyStatus(statusEl, toFailed(rut.message!));
        return;
      }
      emisor.rut = rut.value;
      const acteco = validateActeco(emisorEl.querySelector<HTMLInputElement>("#cfg-em-acteco")!.value);
      if (!acteco.ok) {
        applyStatus(statusEl, toFailed(acteco.message!));
        return;
      }
      if (acteco.value !== undefined) emisor.acteco = acteco.value;

      saveBtn.disabled = true;
      applyStatus(statusEl, toSaving());
      try {
        await setSetting(serverUrl, EMISOR_KEY, JSON.stringify(emisor));
        applyStatus(statusEl, toSaved());
        toast("Datos tributarios guardados");
      } catch (err) {
        applyStatus(statusEl, toFailed(asMessage(err)));
      } finally {
        saveBtn.disabled = false;
      }
    });
  }

  // --- SII environment (sección Facturación) ---------------------------------
  async function loadSiiEnv(): Promise<void> {
    const box = panelEl.querySelector<HTMLElement>("#cfg-sii-env");
    if (!box) return;
    box.innerHTML = tableSkeleton(1);
    try {
      const envSetting = await getSetting(serverUrl, SII_ENV_KEY);
      const env = envSetting?.value === "prod" ? "prod" : "sandbox";
      box.innerHTML = `
        <div class="cfg-emisor-form">
          <div class="field">
            <label for="cfg-sii-select">Entorno</label>
            <select id="cfg-sii-select" class="rb-input">
              <option value="sandbox" ${env === "sandbox" ? "selected" : ""}>Certificación (sandbox)</option>
              <option value="prod" ${env === "prod" ? "selected" : ""}>Producción</option>
            </select>
          </div>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-sii-save">Guardar ambiente</button>
            <span class="cfg-status" id="cfg-sii-status" hidden></span>
          </div>
        </div>
      `;
      const select = box.querySelector<HTMLSelectElement>("#cfg-sii-select")!;
      const saveBtn = box.querySelector<HTMLButtonElement>("#cfg-sii-save")!;
      const statusEl = box.querySelector<HTMLElement>("#cfg-sii-status")!;
      saveBtn.addEventListener("click", async () => {
        const chosen = select.value === "prod" ? "prod" : "sandbox";
        if (chosen === "prod" && !window.confirm(
          "Vas a apuntar el envío de boletas al SII de PRODUCCIÓN. Cada DTE enviado tendrá validez tributaria real. ¿Continuar?",
        )) {
          return;
        }
        saveBtn.disabled = true;
        applyStatus(statusEl, toSaving());
        try {
          await setSetting(serverUrl, SII_ENV_KEY, chosen);
          applyStatus(statusEl, toSaved());
          toast("Ambiente SII guardado");
        } catch (err) {
          applyStatus(statusEl, toFailed(asMessage(err)));
        } finally {
          saveBtn.disabled = false;
        }
      });
    } catch (err) {
      box.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  async function loadCaf(): Promise<void> {
    const box = panelEl.querySelector<HTMLElement>("#cfg-caf");
    if (!box) return;
    box.innerHTML = tableSkeleton(2);
    try {
      const caf: CafStatus = await dteCafStatus(serverUrl, 39);
      if (!caf.cafs.length) {
        box.innerHTML = `<p class="muted cfg-help">Aún no hay folios CAF cargados para boleta electrónica. Se cargan en el servidor durante la puesta en marcha.</p>`;
        return;
      }
      const low = caf.folios_restantes <= 20;
      box.innerHTML = `
        <div class="cfg-caf-summary">
          <span class="cfg-caf-big ${low ? "low" : ""}">${caf.folios_restantes}</span>
          <span class="muted">folios restantes${low ? " — quedan pocos" : ""}</span>
        </div>
        <ul class="cfg-caf-list">
          ${caf.cafs.map((c) => `
            <li>Rango ${c.folio_desde}–${c.folio_hasta} · próximo folio ${c.next_folio} · ${c.restantes} disponibles</li>`).join("")}
        </ul>
      `;
    } catch (err) {
      box.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  // --- Licencia --------------------------------------------------------------
  async function loadLicense(): Promise<void> {
    const box = panelEl.querySelector<HTMLElement>("#cfg-license");
    if (!box) return;
    box.innerHTML = tableSkeleton(3);
    try {
      const lic: LicenseSummary = await licenseStatus(serverUrl);
      const exp = lic.expires_at ? lic.expires_at.slice(0, 10) : "sin vencimiento";
      const statusLabel: Record<string, string> = {
        active: "Activa",
        grace: "En período de gracia",
        expired: "Vencida",
      };
      const features = lic.features.length
        ? `<ul class="cfg-feature-list">${lic.features
            .map((f) => `<li>${escapeHtml(FEATURE_LABELS[f] ?? f)}</li>`)
            .join("")}</ul>`
        : `<p class="muted cfg-help">El núcleo gratuito (POS, inventario, caja, recetas, respaldo local) está siempre disponible. Aún no hay funcionalidades de pago activadas.</p>`;
      box.innerHTML = `
        <div class="cfg-license-head">
          <span class="badge tier-${escapeHtml(lic.tier.toLowerCase())}">${escapeHtml(lic.tier.toUpperCase())}</span>
          <span class="cfg-license-status">${escapeHtml(statusLabel[lic.status] ?? lic.status)}</span>
        </div>
        <dl class="cfg-deflist">
          <div><dt>Vencimiento</dt><dd>${escapeHtml(exp)}</dd></div>
          <div><dt>Asientos</dt><dd>${lic.seat_count}</dd></div>
        </dl>
        <h4 class="cfg-card-title">Funcionalidades habilitadas</h4>
        ${features}
        <p class="muted cfg-help">¿Necesitas activar un plan o una funcionalidad? La activación desde la UI llega pronto; por ahora se importa la licencia en el servidor (<code>pharma license import</code>). El núcleo gratuito nunca caduca.</p>
      `;
    } catch (err) {
      box.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  // --- Agente: federación + fidelidad ----------------------------------------
  async function loadAgent(): Promise<void> {
    const box = panelEl.querySelector<HTMLElement>("#cfg-agent");
    if (!box) return;
    box.innerHTML = tableSkeleton(2);
    try {
      const [fed, loyalty] = await Promise.all([
        getSetting(serverUrl, FEDERATION_KEY),
        getSetting(serverUrl, LOYALTY_KEY),
      ]);
      const fedOn = readBool(fed, false);
      const points = loyalty?.value ?? "0";
      box.innerHTML = `
        <div class="field">
          <label class="cfg-switch">
            <input type="checkbox" id="cfg-fed" ${fedOn ? "checked" : ""} />
            <span>Federación B2B activada</span>
          </label>
          <p class="muted cfg-help">Permite que esta sucursal reciba cotizaciones y órdenes de otros nodos del ecosistema. Requiere reiniciar los agentes para tomar efecto.</p>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-fed-save">Guardar federación</button>
            <span class="cfg-status" id="cfg-fed-status" hidden></span>
          </div>
        </div>
        <div class="field">
          <label for="cfg-loyalty">Puntos de fidelidad por peso vendido</label>
          <input type="number" id="cfg-loyalty" class="cfg-number" inputmode="numeric" min="0" step="1" value="${escapeHtml(points)}" />
          <p class="muted cfg-help">Puntos que acumula el cliente por cada $1 vendido. 0 desactiva la fidelidad.</p>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-loyalty-save">Guardar fidelidad</button>
            <span class="cfg-status" id="cfg-loyalty-status" hidden></span>
          </div>
        </div>
      `;
      const fedInput = box.querySelector<HTMLInputElement>("#cfg-fed")!;
      const fedBtn = box.querySelector<HTMLButtonElement>("#cfg-fed-save")!;
      const fedStatus = box.querySelector<HTMLElement>("#cfg-fed-status")!;
      fedBtn.addEventListener("click", async () => {
        fedBtn.disabled = true;
        applyStatus(fedStatus, toSaving());
        try {
          await setSetting(serverUrl, FEDERATION_KEY, fedInput.checked ? "true" : "false");
          applyStatus(fedStatus, toSaved());
          toast("Federación guardada");
        } catch (err) {
          applyStatus(fedStatus, toFailed(asMessage(err)));
        } finally {
          fedBtn.disabled = false;
        }
      });

      const loyaltyInput = box.querySelector<HTMLInputElement>("#cfg-loyalty")!;
      const loyaltyBtn = box.querySelector<HTMLButtonElement>("#cfg-loyalty-save")!;
      const loyaltyStatus = box.querySelector<HTMLElement>("#cfg-loyalty-status")!;
      loyaltyBtn.addEventListener("click", async () => {
        const r = validateNonNegativeInt(loyaltyInput.value, "Puntos de fidelidad");
        if (!r.ok) {
          applyStatus(loyaltyStatus, toFailed(r.message!));
          return;
        }
        loyaltyBtn.disabled = true;
        applyStatus(loyaltyStatus, toSaving());
        try {
          await setSetting(serverUrl, LOYALTY_KEY, String(r.value));
          applyStatus(loyaltyStatus, toSaved());
          toast("Fidelidad guardada");
        } catch (err) {
          applyStatus(loyaltyStatus, toFailed(asMessage(err)));
        } finally {
          loyaltyBtn.disabled = false;
        }
      });
    } catch (err) {
      box.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  // --- Telemetría (Preferencias) ---------------------------------------------
  async function loadTelemetry(): Promise<void> {
    const box = panelEl.querySelector<HTMLElement>("#cfg-telemetry");
    if (!box) return;
    box.innerHTML = tableSkeleton(1);
    try {
      const setting = await getSetting(serverUrl, TELEMETRY_KEY);
      const on = readBool(setting, false);
      box.innerHTML = `
        <label class="cfg-switch">
          <input type="checkbox" id="cfg-telemetry-toggle" ${on ? "checked" : ""} />
          <span>Enviar estadísticas anónimas de uso</span>
        </label>
        <div class="cfg-edit">
          <button class="btn-primary" id="cfg-telemetry-save">Guardar preferencia</button>
          <span class="cfg-status" id="cfg-telemetry-status" hidden></span>
        </div>
      `;
      const toggle = box.querySelector<HTMLInputElement>("#cfg-telemetry-toggle")!;
      const btn = box.querySelector<HTMLButtonElement>("#cfg-telemetry-save")!;
      const statusEl = box.querySelector<HTMLElement>("#cfg-telemetry-status")!;
      btn.addEventListener("click", async () => {
        btn.disabled = true;
        applyStatus(statusEl, toSaving());
        try {
          await setSetting(serverUrl, TELEMETRY_KEY, toggle.checked ? "true" : "false");
          applyStatus(statusEl, toSaved());
          toast("Preferencia guardada");
        } catch (err) {
          applyStatus(statusEl, toFailed(asMessage(err)));
        } finally {
          btn.disabled = false;
        }
      });
    } catch (err) {
      box.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  // --- Rubro / vertical grid (vitrina) ---------------------------------------
  let selectedVertical = "otro";

  async function loadVerticalForm(): Promise<void> {
    const verticalEl = panelEl.querySelector<HTMLElement>("#cfg-vertical");
    if (!verticalEl) return;
    verticalEl.innerHTML = tableSkeleton(2);
    try {
      const [vSetting, nameSetting] = await Promise.all([
        getSetting(serverUrl, VERTICAL_KEY),
        getSetting(serverUrl, BUSINESS_NAME_KEY),
      ]);
      const stored = (vSetting?.value ?? "").trim();
      selectedVertical = RUBRO_CATALOG.some((r) => r.value === stored) ? stored : "otro";
      const name = nameSetting?.value ?? "";
      verticalEl.innerHTML = `
        <div class="rubro-config">
          <div class="rubro-grid" role="radiogroup" aria-label="Rubro del negocio">
            ${RUBRO_CATALOG.map((r) => {
              const on = r.value === selectedVertical;
              const tag = r.seedVertical
                ? `<span class="rubro-tag demo">datos demo</span>`
                : r.value !== "otro"
                  ? `<span class="rubro-tag soon">pronto</span>`
                  : "";
              return `
              <button type="button" role="radio" class="rubro-card${on ? " selected" : ""}"
                      data-vertical="${r.value}" aria-checked="${on}" tabindex="${on ? 0 : -1}"
                      style="--rubro-accent:${r.accent}">
                <span class="rubro-icon" aria-hidden="true">${rubroIconSvg(r.iconId)}</span>
                <span class="rubro-label">${escapeHtml(r.label)}</span>
                <span class="rubro-help">${escapeHtml(r.help)}</span>
                ${tag}
              </button>`;
            }).join("")}
          </div>
          <aside class="rubro-preview" id="cfg-vert-preview" aria-live="polite"></aside>
        </div>
        <div class="cfg-emisor-form">
          <div class="field">
            <label for="cfg-vert-name">Nombre del negocio (opcional)</label>
            <input id="cfg-vert-name" type="text" placeholder="Mi Negocio"
                   value="${escapeHtml(name)}" autocomplete="off" />
            <p class="muted cfg-help">Se muestra en la barra lateral y en el panel. Si lo dejas vacío, se usa un nombre genérico.</p>
          </div>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-vert-save">Guardar rubro</button>
            <span class="cfg-status" id="cfg-vert-status" hidden></span>
          </div>
          <p class="muted cfg-help">Podés cambiar tu rubro cuando quieras. Tus datos no se borran; sólo cambia qué secciones ves en el menú.</p>
        </div>

        <div class="cfg-demo">
          <div class="cfg-demo-copy">
            <strong>Cargar datos demo</strong>
            <p class="muted cfg-help" id="cfg-demo-help"></p>
          </div>
          <div class="cfg-edit">
            <button class="btn-ghost" id="cfg-demo-btn">Cargar datos demo</button>
            <span class="cfg-status" id="cfg-demo-status" hidden></span>
          </div>
        </div>
      `;
      wireVertical(verticalEl);
      wireDemo(verticalEl);
    } catch (err) {
      verticalEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function wireVertical(verticalEl: HTMLElement): void {
    const cards = Array.from(verticalEl.querySelectorAll<HTMLButtonElement>(".rubro-card"));
    const nameEl = verticalEl.querySelector<HTMLInputElement>("#cfg-vert-name")!;
    const saveBtn = verticalEl.querySelector<HTMLButtonElement>("#cfg-vert-save")!;
    const statusEl = verticalEl.querySelector<HTMLElement>("#cfg-vert-status")!;

    const pin = (value: string): void => {
      selectedVertical = value;
      cards.forEach((c) => {
        const on = c.dataset.vertical === value;
        c.classList.toggle("selected", on);
        c.setAttribute("aria-checked", String(on));
        c.tabIndex = on ? 0 : -1;
      });
      renderPreview(verticalEl, value);
      refreshDemoHelp(verticalEl);
    };
    const peek = (value: string): void => renderPreview(verticalEl, value);
    const unpeek = (): void => renderPreview(verticalEl, selectedVertical);

    cards.forEach((c, i) => {
      const value = c.dataset.vertical!;
      c.addEventListener("click", () => pin(value));
      c.addEventListener("mouseenter", () => peek(value));
      c.addEventListener("focus", () => peek(value));
      c.addEventListener("blur", unpeek);
      c.addEventListener("keydown", (ev) => {
        const cols = 4;
        let next = -1;
        switch (ev.key) {
          case "ArrowRight": next = (i + 1) % cards.length; break;
          case "ArrowLeft": next = (i - 1 + cards.length) % cards.length; break;
          case "ArrowDown": next = Math.min(i + cols, cards.length - 1); break;
          case "ArrowUp": next = Math.max(i - cols, 0); break;
          case "Home": next = 0; break;
          case "End": next = cards.length - 1; break;
          case "Enter":
          case " ":
            ev.preventDefault();
            pin(value);
            return;
          default:
            return;
        }
        ev.preventDefault();
        cards[next].focus();
      });
    });
    verticalEl.querySelector<HTMLElement>(".rubro-grid")?.addEventListener("mouseleave", unpeek);
    renderPreview(verticalEl, selectedVertical);

    saveBtn.addEventListener("click", async () => {
      saveBtn.disabled = true;
      applyStatus(statusEl, toSaving());
      try {
        await setSetting(serverUrl, VERTICAL_KEY, selectedVertical);
        await setSetting(serverUrl, BUSINESS_NAME_KEY, nameEl.value.trim());
        applyStatus(statusEl, toSaved("Guardado — reinicia la app para aplicar a todo el menú."));
        toast("Rubro guardado");
      } catch (err) {
        applyStatus(statusEl, toFailed(asMessage(err)));
      } finally {
        saveBtn.disabled = false;
      }
    });
  }

  // Live "Vista previa de tu ERP" — reads the pure rubroPreview model (same nav
  // gate shell.ts applies) so preview and real ERP can never drift.
  function renderPreview(verticalEl: HTMLElement, value: string): void {
    const box = verticalEl.querySelector<HTMLElement>("#cfg-vert-preview");
    if (!box) return;
    const rubro = parseRubro(value);
    const card = RUBRO_CATALOG.find((r) => r.value === value);
    const p = rubroPreview(rubro);
    if (card) box.style.setProperty("--rubro-accent", card.accent);
    else box.style.removeProperty("--rubro-accent");

    const catRow = (label: string, on: boolean): string =>
      `<li class="rubro-cat ${on ? "on" : "off"}">
         <span class="rubro-cat-mark" aria-hidden="true">${on ? "✓" : "—"}</span>
         <span>${escapeHtml(label)}</span>
       </li>`;

    const nativeBlock = p.native.length
      ? `<div class="rubro-pv-block">
           <h5 class="rubro-pv-title">Específico de tu rubro</h5>
           <ul class="rubro-native">${p.native.map((t) => `<li>${escapeHtml(t)}</li>`).join("")}</ul>
         </div>`
      : `<div class="rubro-pv-block">
           <h5 class="rubro-pv-title">Específico de tu rubro</h5>
           <p class="muted rubro-pv-muted">ERP genérico: ventas, inventario y reportes, sin secciones propias de un rubro.</p>
         </div>`;

    const hiddenBlock = p.hidden.length
      ? `<div class="rubro-pv-block">
           <h5 class="rubro-pv-title">Secciones que se ocultan</h5>
           <div class="rubro-chips">${p.hidden.map((t) => `<span class="rubro-chip off">${escapeHtml(t)}</span>`).join("")}</div>
         </div>`
      : "";

    const comingBlock = p.comingSoon.length
      ? `<div class="rubro-pv-block rubro-pv-soon">
           <h5 class="rubro-pv-title">Próximamente para tu rubro</h5>
           <ul class="rubro-coming">${p.comingSoon.map((t) => `<li>${escapeHtml(t)}</li>`).join("")}</ul>
           <p class="muted rubro-pv-muted">Hoy ya funciona como ERP completo; esto llega más adelante.</p>
         </div>`
      : "";

    const demoBlock = p.hasDemo
      ? `<span class="rubro-pv-demo on">Datos de ejemplo disponibles</span>`
      : `<span class="rubro-pv-demo">Pack de datos demo próximamente</span>`;

    box.innerHTML = `
      <header class="rubro-pv-head">
        <span class="rubro-pv-icon" aria-hidden="true">${rubroIconSvg(card?.iconId, { size: 26 })}</span>
        <div>
          <h4 class="rubro-pv-name">${escapeHtml(card?.label ?? "Otro")}</h4>
          <p class="rubro-pv-tag">${escapeHtml(card?.tagline ?? "")}</p>
        </div>
      </header>
      <p class="rubro-pv-count muted">Muestra <strong>${p.visibleCount}</strong> de ${p.totalCount} secciones del menú.</p>
      <div class="rubro-pv-block">
        <h5 class="rubro-pv-title">Qué incluye</h5>
        <ul class="rubro-cats">${p.categories.map((c) => catRow(c.label, c.on)).join("")}</ul>
      </div>
      ${nativeBlock}
      ${comingBlock}
      ${hiddenBlock}
      ${demoBlock}
      <p class="rubro-pv-note muted">Boleta y factura electrónica (SII): en todos los rubros. Recetas y Libro de controlados (Ley 20.000): sólo Farmacia.</p>
    `;
  }

  function refreshDemoHelp(verticalEl: HTMLElement): void {
    const help = verticalEl.querySelector<HTMLElement>("#cfg-demo-help");
    const btn = verticalEl.querySelector<HTMLButtonElement>("#cfg-demo-btn");
    if (!help || !btn) return;
    const pack = seedVerticalFor(selectedVertical);
    if (pack) {
      help.textContent =
        "Llena el negocio con un catálogo de ejemplo (productos, lotes y ventas) para probar la app. Idempotente; puedes regenerarlo.";
      btn.hidden = false;
      btn.disabled = false;
    } else {
      help.textContent =
        "Aún no hay datos demo para este rubro. Podés empezar igual: importá tu catálogo (CSV) o créalo a mano en Inventario.";
      btn.hidden = true;
    }
  }

  function wireDemo(verticalEl: HTMLElement): void {
    const btn = verticalEl.querySelector<HTMLButtonElement>("#cfg-demo-btn")!;
    const statusEl = verticalEl.querySelector<HTMLElement>("#cfg-demo-status")!;
    refreshDemoHelp(verticalEl);

    const run = async (pack: NonNullable<SeedVertical>, force: boolean): Promise<void> => {
      statusEl.hidden = true;
      btn.disabled = true;
      const prev = btn.textContent;
      btn.textContent = "Cargando…";
      try {
        const s = await seedDemo(serverUrl, pack, force);
        statusEl.textContent = `Listo: ${s.products_created} productos, ${s.batches_created} lotes, ${s.movements_emitted} ventas${s.wiped > 0 ? ` (se reemplazaron ${s.wiped})` : ""}.`;
        statusEl.className = "cfg-status cfg-status-ok";
        statusEl.hidden = false;
        toast("Datos demo cargados");
      } catch (err) {
        if (err === SEED_ALREADY_EXISTS) {
          if (window.confirm(
            "Ya hay datos demo cargados. ¿Regenerarlos? Se borrará el pack demo anterior y se volverá a sembrar.",
          )) {
            await run(pack, true);
            return;
          }
          statusEl.hidden = true;
        } else {
          statusEl.textContent = asMessage(err);
          statusEl.className = "cfg-status cfg-status-err";
          statusEl.hidden = false;
        }
      } finally {
        btn.textContent = prev;
        refreshDemoHelp(verticalEl);
      }
    };

    btn.addEventListener("click", () => {
      const pack = seedVerticalFor(selectedVertical);
      if (!pack) return;
      if (!window.confirm(
        `Vas a cargar DATOS DE EJEMPLO para el rubro seleccionado. Úsalo solo en una instalación de prueba, no sobre datos reales. ¿Continuar?`,
      )) {
        return;
      }
      void run(pack, false);
    });
  }

  // --- Conexión al servidor + tema (Preferencias) ----------------------------
  function wireServerAndTheme(): void {
    const urlEl = panelEl.querySelector<HTMLInputElement>("#cfg-server-url")!;
    const testBtn = panelEl.querySelector<HTMLButtonElement>("#cfg-server-test")!;
    const saveBtn = panelEl.querySelector<HTMLButtonElement>("#cfg-server-save")!;
    const statusEl = panelEl.querySelector<HTMLElement>("#cfg-server-status")!;

    testBtn.addEventListener("click", async () => {
      const url = urlEl.value.trim();
      if (!url) {
        applyStatus(statusEl, toFailed("Escribe una URL primero."));
        return;
      }
      testBtn.disabled = true;
      statusEl.textContent = "Probando…";
      statusEl.className = "cfg-status";
      statusEl.hidden = false;
      try {
        const h = await serverHealth(url);
        const ok = h.reachable && h.status === "ok" && h.db === "ok";
        statusEl.textContent = ok
          ? "✓ Servidor accesible."
          : `Servidor responde (estado: ${h.status}, db: ${h.db}).`;
        statusEl.className = `cfg-status ${ok ? "cfg-status-ok" : "cfg-status-err"}`;
      } catch (err) {
        statusEl.textContent = asMessage(err);
        statusEl.className = "cfg-status cfg-status-err";
      } finally {
        testBtn.disabled = false;
      }
    });

    saveBtn.addEventListener("click", () => {
      const url = urlEl.value.trim();
      if (!url) {
        applyStatus(statusEl, toFailed("La URL no puede estar vacía."));
        return;
      }
      rememberServerUrl(url);
      applyStatus(statusEl, toSaved("Guardado — se usará al volver a iniciar sesión."));
      toast("URL del servidor guardada");
    });

    const themeEl = panelEl.querySelector<HTMLSelectElement>("#cfg-theme")!;
    themeEl.value = currentTheme();
    themeEl.addEventListener("change", () => {
      applyTheme(themeEl.value === "light" ? "light" : "dark");
      toast("Tema actualizado");
    });
  }

  // Initial paint.
  selectSection(activeSection);
}

/** localStorage key for the operator's UI theme preference. */
const THEME_KEY = "pharma:theme";

/** The persisted theme, defaulting to dark (the app's native palette). */
export function currentTheme(): "dark" | "light" {
  try {
    return localStorage.getItem(THEME_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

/** Apply + persist the UI theme (sets `data-theme` for the RutBusiness tokens). */
export function applyTheme(theme: "dark" | "light"): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* noop */
  }
}
