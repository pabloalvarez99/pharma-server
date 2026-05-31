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
      <div id="cfg-toast" class="toast" hidden></div>
    </section>
  `;

  const bodyEl = host.querySelector<HTMLElement>("#cfg-body")!;
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

  void load();
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
