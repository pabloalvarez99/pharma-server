// Auditoría view — read-only window onto the immutable audit trail (admin/owner).
//   Filters (rango de fechas · acción · tabla) → paginated table of audited
//   requests: fecha/hora, usuario, acción, tabla, ruta, estado HTTP, IP.
//
// The product pillar is "cada cambio queda en log inmutable"; this surfaces it.
// The server records method/path/status/ip today (before/after/metadata land
// with a future migration), so the table shows the who/what/when/where/result.
//
// DEGRADES GRACEFULLY: the endpoint is admin/owner only — a 403 shows a friendly
// note instead of a hard error. Spanish throughout; same skeleton → fetch → swap.
import { queryAuditLog, type AuditEntry, type AuditFilters } from "../api";
import { num, fechaHora } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";
import { emptyState, errorState } from "./ui";

const PAGE_SIZE = 50;

const ACTION_OPTS: { value: string; label: string }[] = [
  { value: "", label: "Todas las acciones" },
  { value: "create", label: "Creación" },
  { value: "update", label: "Modificación" },
  { value: "delete", label: "Eliminación" },
];

export function renderAuditoria(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-auditoria">
      <div class="view-head">
        <div>
          <h2 class="rb-display">Auditoría</h2>
          <p class="muted">Registro inmutable de cambios. Sólo administradores.</p>
        </div>
        <div class="audit-filters">
          <label class="audit-field">Desde<input id="aud-from" type="date" /></label>
          <label class="audit-field">Hasta<input id="aud-to" type="date" /></label>
          <select id="aud-action" class="view-select">
            ${ACTION_OPTS.map((o) => `<option value="${escapeHtml(o.value)}">${escapeHtml(o.label)}</option>`).join("")}
          </select>
          <input id="aud-table" type="search" class="audit-table-input" placeholder="Tabla (ej. products)" autocomplete="off" />
          <button id="aud-search" type="button" class="btn-ghost rb-btn ghost">Buscar</button>
        </div>
      </div>

      <div class="table-card rb-card">
        <div id="aud-table-host">${tableSkeleton(7)}</div>
      </div>
    </section>
  `;

  const fromEl = host.querySelector<HTMLInputElement>("#aud-from")!;
  const toEl = host.querySelector<HTMLInputElement>("#aud-to")!;
  const actionEl = host.querySelector<HTMLSelectElement>("#aud-action")!;
  const tableEl = host.querySelector<HTMLInputElement>("#aud-table")!;
  const searchBtn = host.querySelector<HTMLButtonElement>("#aud-search")!;
  const tableHost = host.querySelector<HTMLElement>("#aud-table-host")!;

  const baseFilters = (): AuditFilters => ({
    from: fromEl.value || undefined,
    to: toEl.value || undefined,
    action: actionEl.value || undefined,
    table: tableEl.value.trim() || undefined,
    limit: PAGE_SIZE,
  });

  async function load(offset: number): Promise<void> {
    tableHost.innerHTML = tableSkeleton(7);
    try {
      const page = await queryAuditLog(serverUrl, { ...baseFilters(), offset });
      renderTable(tableHost, page.items, page.total, offset, (next) => void load(next));
    } catch (err) {
      tableHost.innerHTML = renderError(err);
    }
  }

  searchBtn.addEventListener("click", () => void load(0));
  [fromEl, toEl, actionEl].forEach((el) =>
    el.addEventListener("change", () => void load(0)),
  );
  tableEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter") void load(0);
  });

  void load(0);
}

function renderTable(
  host: HTMLElement,
  rows: AuditEntry[],
  total: number,
  offset: number,
  go: (offset: number) => void,
): void {
  if (rows.length === 0) {
    host.innerHTML = emptyState({
      title: "Sin registros de auditoría",
      hint: "No hay cambios para el filtro seleccionado. Ajusta el rango de fechas, la acción o la tabla.",
    });
    return;
  }
  const from = offset + 1;
  const to = offset + rows.length;
  const hasPrev = offset > 0;
  const hasNext = offset + rows.length < total;

  host.innerHTML = `
    <table class="data-table rb-table">
      <thead>
        <tr>
          <th>Fecha</th>
          <th>Usuario</th>
          <th>Acción</th>
          <th>Tabla</th>
          <th>Ruta</th>
          <th class="num">Estado</th>
          <th>IP</th>
        </tr>
      </thead>
      <tbody>${rows.map(auditRow).join("")}</tbody>
    </table>
    <div class="audit-pager">
      <span class="muted">${num(from)}–${num(to)} de ${num(total)}</span>
      <div class="audit-pager-btns">
        <button type="button" class="btn-ghost rb-btn ghost" id="aud-prev" ${hasPrev ? "" : "disabled"}>‹ Anteriores</button>
        <button type="button" class="btn-ghost rb-btn ghost" id="aud-next" ${hasNext ? "" : "disabled"}>Siguientes ›</button>
      </div>
    </div>
  `;

  const prev = host.querySelector<HTMLButtonElement>("#aud-prev")!;
  const next = host.querySelector<HTMLButtonElement>("#aud-next")!;
  if (hasPrev) prev.addEventListener("click", () => go(Math.max(0, offset - PAGE_SIZE)));
  if (hasNext) next.addEventListener("click", () => go(offset + PAGE_SIZE));
}

function auditRow(a: AuditEntry): string {
  const who = a.user_email || (a.user ? a.user.replace(/^user:/, "") : "—");
  const tabla = a.table || "—";
  const status = a.status === null ? "—" : String(a.status);
  const statusTone =
    a.status === null ? "" : a.status >= 500 ? "pill-danger" : a.status >= 400 ? "pill-warn" : "pill-ok";
  return `
    <tr>
      <td><span class="muted">${escapeHtml(fechaHora(a.created_at))}</span></td>
      <td><div class="cell-main">${escapeHtml(who)}</div></td>
      <td>${actionPill(a.action)}</td>
      <td><div class="cell-sub muted">${escapeHtml(tabla)}</div></td>
      <td><code class="audit-path">${escapeHtml(a.path)}</code></td>
      <td class="num">${statusTone ? `<span class="pill ${statusTone}">${escapeHtml(status)}</span>` : "—"}</td>
      <td><span class="muted">${escapeHtml(a.ip || "—")}</span></td>
    </tr>
  `;
}

function actionPill(action: string): string {
  switch (action.toLowerCase()) {
    case "create":
      return `<span class="pill pill-ok">Creación</span>`;
    case "update":
      return `<span class="pill pill-warn">Modificación</span>`;
    case "delete":
      return `<span class="pill pill-danger">Eliminación</span>`;
    default:
      return `<span class="pill">${escapeHtml(action)}</span>`;
  }
}

function renderError(err: unknown): string {
  const msg = asMessage(err);
  if (msg.includes("403") || msg.includes("denegado") || msg.includes("Permiso")) {
    return emptyState({
      title: "Auditoría restringida",
      hint: "El registro de auditoría sólo está disponible para administradores. Contacta al dueño o administrador del negocio.",
    });
  }
  return errorState(msg);
}
