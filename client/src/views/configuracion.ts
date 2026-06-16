// Configuración view — admin read/write of the server's known `admin_setting`
// keys (GET/PUT /api/v1/settings/{key}). Mutation is admin+ server-side; a
// non-admin save surfaces the server's 403 inline. Each known key is described
// by a small catalog (label, help, control type) so the view renders a typed
// editor per setting rather than a raw key/value grid. Unset keys (404 → null)
// fall back to a documented default. Same skeleton → fetch → swap pattern as the
// other views; Spanish throughout.
import {
  getSetting,
  setSetting,
  serverHealth,
  seedDemo,
  storedServerUrl,
  rememberServerUrl,
  SEED_ALREADY_EXISTS,
} from "../api";
import { isValidRut, canonicalRut, formatRut } from "../format";
import {
  VERTICAL_KEY,
  BUSINESS_NAME_KEY,
  RUBRO_CATALOG,
  seedVerticalFor,
  parseRubro,
} from "../vertical";
import { rubroPreview } from "./first-run";
import { rubroIconSvg } from "../brand/rubro-icons";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";

type SettingKind = "boolean" | "number";

interface SettingDef {
  key: string;
  label: string;
  help: string;
  kind: SettingKind;
  /** Value used when the key is unset server-side (404). */
  fallback: string;
}

// Closed catalog of keys the server actually reads (grep crates: agent.rs reads
// `federation_enabled`; sales/service.rs reads `loyalty_points_per_clp`).
const SETTINGS: readonly SettingDef[] = [
  {
    key: "federation_enabled",
    label: "Federación B2B",
    help: "Permite que esta sucursal reciba cotizaciones y órdenes de otros nodos del ecosistema. Requiere reinicio de los agentes para tomar efecto.",
    kind: "boolean",
    fallback: "false",
  },
  {
    key: "loyalty_points_per_clp",
    label: "Puntos de fidelidad por CLP",
    help: "Puntos otorgados al cliente por cada peso vendido. 0 desactiva la acumulación de fidelidad.",
    kind: "number",
    fallback: "0",
  },
] as const;

// --- DTE emisor (dte.emisor + dte.sii_env) ----------------------------------
// The DTE wiring (crates/api/src/v1/dte.rs) reads the emitter identity from the
// `dte.emisor` admin_setting as a JSON `EmisorConfig` (rut, razon_social, giro,
// direccion, comuna + optional ciudad/acteco), and the SII environment from
// `dte.sii_env` ("sandbox" | "prod"). This form edits both — it's the client
// counterpart of the "Falta configurar el emisor DTE" 400 the emission throws.

const EMISOR_KEY = "dte.emisor";
const SII_ENV_KEY = "dte.sii_env";

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

export function renderConfiguracion(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-configuracion">
      <div class="view-head">
        <div>
          <h2>Configuración</h2>
          <p class="muted">Parámetros del servidor. Sólo administradores pueden guardar cambios.</p>
        </div>
      </div>

      <h3 class="section-title">Conexión al servidor</h3>
      <p class="muted">Apunta este terminal al servidor de tu local. <code>localhost</code> si el servidor corre en este mismo equipo; la IP de red (ej: <code>http://192.168.1.50:8080</code>) si usas una tablet u otro equipo como caja. Se aplica al volver a iniciar sesión.</p>
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

      <h3 class="section-title">Apariencia</h3>
      <p class="muted">Tema de la interfaz. Se guarda en este equipo.</p>
      <div class="cfg-emisor-form rb">
        <div class="field">
          <label class="rb-label" for="cfg-theme">Tema</label>
          <select id="cfg-theme" class="rb-input">
            <option value="dark">Oscuro</option>
            <option value="light">Claro</option>
          </select>
        </div>
      </div>

      <h3 class="section-title">Rubro del negocio</h3>
      <p class="muted">Elige el rubro de tu negocio. Define qué secciones del ERP se muestran: el módulo de Recetas y el Libro de controlados (Ley 20.000) sólo aparecen en Farmacia. Las boletas y facturas electrónicas (SII) funcionan en todos los rubros.</p>
      <div id="cfg-vertical">${tableSkeleton(2)}</div>

      <h3 class="section-title">Parámetros del servidor</h3>
      <div id="cfg-body">${tableSkeleton(SETTINGS.length)}</div>

      <h3 class="section-title">Boleta electrónica — datos del emisor</h3>
      <p class="muted">Identidad tributaria con la que se firman las boletas (SII). Requerido antes de emitir el primer DTE.</p>
      <div id="cfg-emisor">${tableSkeleton(3)}</div>

      <div id="cfg-toast" class="toast" hidden></div>
    </section>
  `;

  const verticalEl = host.querySelector<HTMLElement>("#cfg-vertical")!;
  const bodyEl = host.querySelector<HTMLElement>("#cfg-body")!;
  const emisorEl = host.querySelector<HTMLElement>("#cfg-emisor")!;
  const toastEl = host.querySelector<HTMLElement>("#cfg-toast")!;

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2800);
  }

  async function load(): Promise<void> {
    bodyEl.innerHTML = tableSkeleton(SETTINGS.length);
    try {
      const current = await Promise.all(
        SETTINGS.map((s) => getSetting(serverUrl, s.key)),
      );
      bodyEl.innerHTML = `<div class="cfg-list">${SETTINGS.map((def, i) =>
        rowHtml(def, current[i]?.value ?? def.fallback, current[i]?.updated_at ?? null),
      ).join("")}</div>`;
      SETTINGS.forEach((def) => wireRow(def));
    } catch (err) {
      bodyEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function rowHtml(def: SettingDef, value: string, updatedAt: string | null): string {
    const stamp = updatedAt
      ? `actualizado ${fmtDate(updatedAt)}`
      : "valor por defecto (sin guardar)";
    const control =
      def.kind === "boolean"
        ? `<label class="cfg-switch">
             <input type="checkbox" id="cfg-${def.key}" ${value === "true" ? "checked" : ""} />
             <span>Activado</span>
           </label>`
        : `<input type="number" id="cfg-${def.key}" class="cfg-number" inputmode="numeric"
                 min="0" step="1" value="${escapeHtml(value)}" />`;
    return `
      <div class="cfg-row" data-key="${def.key}">
        <div class="cfg-meta">
          <strong class="cfg-label">${escapeHtml(def.label)}</strong>
          <code class="cfg-key">${escapeHtml(def.key)}</code>
          <p class="muted cfg-help">${escapeHtml(def.help)}</p>
          <span class="cfg-stamp muted">${stamp}</span>
        </div>
        <div class="cfg-edit">
          ${control}
          <button class="btn-primary cfg-save" id="cfg-save-${def.key}">Guardar</button>
          <span class="cfg-status" id="cfg-status-${def.key}" hidden></span>
        </div>
      </div>
    `;
  }

  function wireRow(def: SettingDef): void {
    const saveBtn = bodyEl.querySelector<HTMLButtonElement>(`#cfg-save-${def.key}`)!;
    const statusEl = bodyEl.querySelector<HTMLElement>(`#cfg-status-${def.key}`)!;
    const input = bodyEl.querySelector<HTMLInputElement>(`#cfg-${def.key}`)!;

    saveBtn.addEventListener("click", async () => {
      const value =
        def.kind === "boolean"
          ? input.checked
            ? "true"
            : "false"
          : String(Math.trunc(Number(input.value || "0")));
      if (def.kind === "number" && (!Number.isFinite(Number(input.value)) || Number(input.value) < 0)) {
        statusEl.textContent = "Ingresa un número válido (0 o más).";
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
        return;
      }
      saveBtn.disabled = true;
      statusEl.hidden = true;
      try {
        const saved = await setSetting(serverUrl, def.key, value);
        statusEl.textContent = "Guardado";
        statusEl.className = "cfg-status cfg-status-ok";
        statusEl.hidden = false;
        const stamp = bodyEl.querySelector<HTMLElement>(`.cfg-row[data-key="${def.key}"] .cfg-stamp`);
        if (stamp) stamp.textContent = `actualizado ${fmtDate(saved.updated_at)}`;
        toast(`${def.label} guardado`);
      } catch (err) {
        statusEl.textContent = asMessage(err);
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
      } finally {
        saveBtn.disabled = false;
      }
    });
  }

  // --- DTE emisor form -------------------------------------------------------

  async function loadEmisor(): Promise<void> {
    emisorEl.innerHTML = tableSkeleton(3);
    try {
      const [emisorSetting, envSetting] = await Promise.all([
        getSetting(serverUrl, EMISOR_KEY),
        getSetting(serverUrl, SII_ENV_KEY),
      ]);
      let current: Record<string, unknown> = {};
      if (emisorSetting) {
        try { current = JSON.parse(emisorSetting.value) as Record<string, unknown>; } catch { /* corrupt → empty form */ }
      }
      const env = envSetting?.value === "prod" ? "prod" : "sandbox";
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
          <div class="field">
            <label for="cfg-em-env">Entorno SII</label>
            <select id="cfg-em-env">
              <option value="sandbox" ${env === "sandbox" ? "selected" : ""}>Certificación (sandbox)</option>
              <option value="prod" ${env === "prod" ? "selected" : ""}>Producción</option>
            </select>
          </div>
          <div class="cfg-edit">
            <button class="btn-primary" id="cfg-em-save">Guardar emisor</button>
            <span class="cfg-status" id="cfg-em-status" hidden></span>
          </div>
        </div>
      `;
      wireEmisor();
    } catch (err) {
      emisorEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function wireEmisor(): void {
    const saveBtn = emisorEl.querySelector<HTMLButtonElement>("#cfg-em-save")!;
    const statusEl = emisorEl.querySelector<HTMLElement>("#cfg-em-status")!;
    const rutEl = emisorEl.querySelector<HTMLInputElement>("#cfg-em-rut")!;
    const rutHint = emisorEl.querySelector<HTMLElement>("#cfg-em-rut-hint")!;

    // The emisor RUT identifies the pharmacy to the SII — a wrong one breaks
    // every boleta/factura. Validate mód-11 live (same UX as the Facturas
    // receptor) and store the canonical `NNNNNNNN-D` form on save.
    const validateRut = (): boolean => {
      const raw = rutEl.value.trim();
      if (!raw) {
        rutEl.classList.remove("invalid");
        rutHint.hidden = true;
        rutHint.className = "field-hint";
        return false;
      }
      if (isValidRut(raw)) {
        rutEl.classList.remove("invalid");
        rutHint.hidden = false;
        rutHint.className = "field-hint ok";
        rutHint.textContent = `RUT válido — ${formatRut(raw)}`;
        return true;
      }
      rutEl.classList.add("invalid");
      rutHint.hidden = false;
      rutHint.className = "field-hint err";
      rutHint.textContent = "RUT inválido: revisa el dígito verificador.";
      return false;
    };
    rutEl.addEventListener("input", validateRut);
    rutEl.addEventListener("blur", () => {
      if (isValidRut(rutEl.value)) rutEl.value = formatRut(rutEl.value);
    });
    validateRut();

    saveBtn.addEventListener("click", async () => {
      statusEl.hidden = true;
      const emisor: Record<string, unknown> = {};
      for (const f of EMISOR_FIELDS) {
        const v = emisorEl.querySelector<HTMLInputElement>(`#cfg-em-${f.name}`)!.value.trim();
        if (f.required && !v) {
          statusEl.textContent = `Completa el campo "${f.label}".`;
          statusEl.className = "cfg-status cfg-status-err";
          statusEl.hidden = false;
          return;
        }
        if (v) emisor[f.name] = v;
      }
      if (!validateRut()) {
        statusEl.textContent = "El RUT de la empresa no es válido (revisa el dígito verificador mód-11).";
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
        return;
      }
      emisor.rut = canonicalRut(rutEl.value);
      const actecoRaw = emisorEl.querySelector<HTMLInputElement>("#cfg-em-acteco")!.value.trim();
      if (actecoRaw) {
        const n = Number(actecoRaw);
        if (!Number.isInteger(n) || n < 0) {
          statusEl.textContent = "El código acteco debe ser un número entero.";
          statusEl.className = "cfg-status cfg-status-err";
          statusEl.hidden = false;
          return;
        }
        emisor.acteco = n;
      }
      const env = emisorEl.querySelector<HTMLSelectElement>("#cfg-em-env")!.value;
      if (env === "prod" && !window.confirm(
        "Vas a apuntar el envío de boletas al SII de PRODUCCIÓN. Cada DTE enviado tendrá validez tributaria real. ¿Continuar?",
      )) {
        return;
      }
      saveBtn.disabled = true;
      try {
        await setSetting(serverUrl, EMISOR_KEY, JSON.stringify(emisor));
        await setSetting(serverUrl, SII_ENV_KEY, env);
        statusEl.textContent = "Guardado";
        statusEl.className = "cfg-status cfg-status-ok";
        statusEl.hidden = false;
        toast("Emisor DTE guardado");
      } catch (err) {
        statusEl.textContent = asMessage(err);
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
      } finally {
        saveBtn.disabled = false;
      }
    });
  }

  // --- Rubro / vertical (grid del catálogo) ----------------------------------
  //
  // The onboarding "elige tu rubro" grid (docs/strategy/rubro-catalog.md). The
  // chosen card is persisted as `business.vertical`; the seed button below uses
  // it (es→en mapped) to fill the tenant with the matching DEMO pack.

  // Currently selected card value (kept in closure so the demo button + save
  // act on the operator's pick before they hit "Guardar rubro").
  let selectedVertical = "otro";

  async function loadVerticalForm(): Promise<void> {
    verticalEl.innerHTML = tableSkeleton(2);
    try {
      const [vSetting, nameSetting] = await Promise.all([
        getSetting(serverUrl, VERTICAL_KEY),
        getSetting(serverUrl, BUSINESS_NAME_KEY),
      ]);
      // Highlight the stored raw value (not parseVertical) so catalog extras
      // beyond the gated trio still show as selected.
      const stored = (vSetting?.value ?? "").trim();
      selectedVertical = RUBRO_CATALOG.some((r) => r.value === stored) ? stored : "otro";
      const name = nameSetting?.value ?? "";
      verticalEl.innerHTML = `
        <div class="rubro-config">
          <div class="rubro-grid" role="radiogroup" aria-label="Rubro del negocio">
            ${RUBRO_CATALOG.map((r) => {
              const on = r.value === selectedVertical;
              // Tag: an accent "datos demo" chip when a seed pack exists, else a
              // muted "pronto" for the future rubros (still fully selectable —
              // no dead-end), and nothing for the generic `otro`.
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
      wireVertical();
      wireDemo();
    } catch (err) {
      verticalEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  function wireVertical(): void {
    const cards = Array.from(verticalEl.querySelectorAll<HTMLButtonElement>(".rubro-card"));
    const nameEl = verticalEl.querySelector<HTMLInputElement>("#cfg-vert-name")!;
    const saveBtn = verticalEl.querySelector<HTMLButtonElement>("#cfg-vert-save")!;
    const statusEl = verticalEl.querySelector<HTMLElement>("#cfg-vert-status")!;

    // Pin = the operator's confirmed choice (drives save + demo). Peek = the card
    // they're hovering/focusing — the preview shows peek if any, else the pin, so
    // they can browse the ERP of each rubro before committing ("mostrar, no contar").
    const pin = (value: string): void => {
      selectedVertical = value;
      cards.forEach((c) => {
        const on = c.dataset.vertical === value;
        c.classList.toggle("selected", on);
        c.setAttribute("aria-checked", String(on));
        c.tabIndex = on ? 0 : -1; // roving tabindex: only the selected card is tab-reachable
      });
      renderPreview(value);
      refreshDemoHelp();
    };
    const peek = (value: string): void => renderPreview(value);
    const unpeek = (): void => renderPreview(selectedVertical);

    cards.forEach((c, i) => {
      const value = c.dataset.vertical!;
      c.addEventListener("click", () => pin(value));
      c.addEventListener("mouseenter", () => peek(value));
      c.addEventListener("focus", () => peek(value));
      c.addEventListener("blur", unpeek);
      c.addEventListener("keydown", (ev) => {
        // Grid keyboard model: arrows move focus (roving), Enter/Espacio selects.
        const cols = 4; // visual grid width; arrow Down/Up jump a row, Left/Right one card
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
    // Grid leaves → snap preview back to the pinned rubro.
    verticalEl.querySelector<HTMLElement>(".rubro-grid")?.addEventListener("mouseleave", unpeek);
    renderPreview(selectedVertical);

    saveBtn.addEventListener("click", async () => {
      statusEl.hidden = true;
      saveBtn.disabled = true;
      try {
        await setSetting(serverUrl, VERTICAL_KEY, selectedVertical);
        await setSetting(serverUrl, BUSINESS_NAME_KEY, nameEl.value.trim());
        statusEl.textContent = "Guardado — reinicia la app para aplicar a todo el menú.";
        statusEl.className = "cfg-status cfg-status-ok";
        statusEl.hidden = false;
        toast("Rubro guardado");
      } catch (err) {
        statusEl.textContent = asMessage(err);
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
      } finally {
        saveBtn.disabled = false;
      }
    });
  }

  // Live "Vista previa de tu ERP": renders the EXACT ERP a rubro gives — section
  // categories on/off, rubro-native capabilities, hidden sections, demo + SII
  // compliance notes. Reads the pure `rubroPreview` model (same nav gate shell.ts
  // applies), so preview and real ERP can never drift. `value` is the peeked or
  // pinned card; the panel updates <100ms with no layout shift.
  function renderPreview(value: string): void {
    const box = verticalEl.querySelector<HTMLElement>("#cfg-vert-preview");
    if (!box) return;
    const rubro = parseRubro(value);
    const card = RUBRO_CATALOG.find((r) => r.value === value);
    const p = rubroPreview(rubro);
    // Theme the whole preview panel to the rubro's accent so the header, the
    // category checks and the native bullets read as that rubro's identity.
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
           <ul class="rubro-native">${p.native
             .map((t) => `<li>${escapeHtml(t)}</li>`)
             .join("")}</ul>
         </div>`
      : `<div class="rubro-pv-block">
           <h5 class="rubro-pv-title">Específico de tu rubro</h5>
           <p class="muted rubro-pv-muted">ERP genérico: ventas, inventario y reportes, sin secciones propias de un rubro.</p>
         </div>`;

    const hiddenBlock = p.hidden.length
      ? `<div class="rubro-pv-block">
           <h5 class="rubro-pv-title">Secciones que se ocultan</h5>
           <div class="rubro-chips">${p.hidden
             .map((t) => `<span class="rubro-chip off">${escapeHtml(t)}</span>`)
             .join("")}</div>
         </div>`
      : "";

    // Roadmap "Próximamente": documented direction per rubro, shown sober (muted,
    // dashed) so it reads as where the rubro is headed — NOT a feature claim and
    // never a dead-end (the rubro already gives a working ERP today).
    const comingBlock = p.comingSoon.length
      ? `<div class="rubro-pv-block rubro-pv-soon">
           <h5 class="rubro-pv-title">Próximamente para tu rubro</h5>
           <ul class="rubro-coming">${p.comingSoon
             .map((t) => `<li>${escapeHtml(t)}</li>`)
             .join("")}</ul>
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

  // --- Datos demo ------------------------------------------------------------

  function refreshDemoHelp(): void {
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
      // No demo pack for this rubro yet — never a dead button: the operator still
      // gets a working empty ERP, so point them at the real paths forward instead
      // of leaving a greyed, inert control (ULTRA-PLAN §8: "no dead-end").
      help.textContent =
        "Aún no hay datos demo para este rubro. Podés empezar igual: importá tu catálogo (CSV) o créalo a mano en Inventario.";
      btn.hidden = true;
    }
  }

  function wireDemo(): void {
    const btn = verticalEl.querySelector<HTMLButtonElement>("#cfg-demo-btn")!;
    const statusEl = verticalEl.querySelector<HTMLElement>("#cfg-demo-status")!;
    refreshDemoHelp();

    const run = async (pack: "pharmacy" | "minimarket", force: boolean): Promise<void> => {
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
          if (
            window.confirm(
              "Ya hay datos demo cargados. ¿Regenerarlos? Se borrará el pack demo anterior y se volverá a sembrar.",
            )
          ) {
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
        refreshDemoHelp();
      }
    };

    btn.addEventListener("click", () => {
      const pack = seedVerticalFor(selectedVertical);
      if (!pack) return;
      if (
        !window.confirm(
          `Vas a cargar DATOS DE EJEMPLO para el rubro seleccionado. Úsalo solo en una instalación de prueba, no sobre datos reales. ¿Continuar?`,
        )
      ) {
        return;
      }
      void run(pack, false);
    });
  }

  // --- Conexión al servidor + tema -------------------------------------------

  function wireServerAndTheme(): void {
    const urlEl = host.querySelector<HTMLInputElement>("#cfg-server-url")!;
    const testBtn = host.querySelector<HTMLButtonElement>("#cfg-server-test")!;
    const saveBtn = host.querySelector<HTMLButtonElement>("#cfg-server-save")!;
    const statusEl = host.querySelector<HTMLElement>("#cfg-server-status")!;

    testBtn.addEventListener("click", async () => {
      const url = urlEl.value.trim();
      if (!url) {
        statusEl.textContent = "Escribe una URL primero.";
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
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
        statusEl.textContent = "La URL no puede estar vacía.";
        statusEl.className = "cfg-status cfg-status-err";
        statusEl.hidden = false;
        return;
      }
      rememberServerUrl(url);
      statusEl.textContent = "Guardado — se usará al volver a iniciar sesión.";
      statusEl.className = "cfg-status cfg-status-ok";
      statusEl.hidden = false;
      toast("URL del servidor guardada");
    });

    const themeEl = host.querySelector<HTMLSelectElement>("#cfg-theme")!;
    themeEl.value = currentTheme();
    themeEl.addEventListener("change", () => {
      applyTheme(themeEl.value === "light" ? "light" : "dark");
      toast("Tema actualizado");
    });
  }

  wireServerAndTheme();
  void loadVerticalForm();
  void load();
  void loadEmisor();
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

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("es-CL", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
