// Configuración view — admin read/write of the server's known `admin_setting`
// keys (GET/PUT /api/v1/settings/{key}). Mutation is admin+ server-side; a
// non-admin save surfaces the server's 403 inline. Each known key is described
// by a small catalog (label, help, control type) so the view renders a typed
// editor per setting rather than a raw key/value grid. Unset keys (404 → null)
// fall back to a documented default. Same skeleton → fetch → swap pattern as the
// other views; Spanish throughout.
import { getSetting, setSetting } from "../api";
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
  { name: "razon_social", label: "Razón social", placeholder: "Farmacia Ejemplo SpA", required: true },
  { name: "giro", label: "Giro", placeholder: "Venta al por menor de productos farmacéuticos", required: true },
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

      <div id="cfg-body">${tableSkeleton(SETTINGS.length)}</div>

      <h3 class="section-title">Boleta electrónica — datos del emisor</h3>
      <p class="muted">Identidad tributaria con la que se firman las boletas (SII). Requerido antes de emitir el primer DTE.</p>
      <div id="cfg-emisor">${tableSkeleton(3)}</div>

      <div id="cfg-toast" class="toast" hidden></div>
    </section>
  `;

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

  void load();
  void loadEmisor();
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
