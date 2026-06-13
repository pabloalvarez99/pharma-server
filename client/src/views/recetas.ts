// Recetas view — prescription registry + controlled-drug ledger (Ley 20.000).
//   Search by patient RUT → list of prescriptions (paciente, médico, folio,
//   tipo). A "Solo controlados" toggle narrows the main list. Below it, the
//   "Libro de recetas" section shows the controlled-only ledger the ISP/DEIS
//   require, with a CSV export. "Nueva receta" opens a create modal — a
//   controlled entry forces médico name + RUT (mirrors the server rule, caught
//   client-side before the POST).
//
// Prescriptions are immutable per Ley 20.000: create + read only, no edit/delete.
// Spanish throughout. Same skeleton → fetch → swap pattern as the other views;
// reuses the shared `.modal` chrome and inventory.ts table helpers.
import {
  listPrescriptions,
  libroRecetas,
  createPrescription,
  exportLibroRecetas,
  type Prescription,
  type NewPrescriptionInput,
} from "../api";
import { isValidRut, canonicalRut, formatRut } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";

const PAGE_LIMIT = 100;

export function renderRecetas(host: HTMLElement, serverUrl: string): void {
  host.innerHTML = `
    <section class="view view-recetas">
      <div class="view-head">
        <div>
          <h2>Recetas</h2>
          <p class="muted">Registro de recetas y libro de controlados (Ley 20.000).</p>
        </div>
        <div class="cli-head-actions">
          <div class="view-search">
            <input id="rec-search" type="search" placeholder="Filtrar por RUT del paciente…" autocomplete="off" />
          </div>
          <label class="rec-toggle">
            <input id="rec-controlled" type="checkbox" /> Solo controlados
          </label>
          <button id="rec-new" type="button" class="btn-ghost">+ Nueva receta</button>
        </div>
      </div>

      <div id="rec-modal-host"></div>

      <div class="table-card">
        <h3 class="section-title">Recetas</h3>
        <div id="rec-table">${tableSkeleton()}</div>
      </div>

      <div class="table-card">
        <div class="view-head">
          <h3 class="section-title">Libro de recetas · controlados</h3>
          <button id="rec-export" type="button" class="btn-ghost">Exportar CSV</button>
        </div>
        <div id="rec-libro">${tableSkeleton(5)}</div>
      </div>
    </section>
  `;

  const searchEl = host.querySelector<HTMLInputElement>("#rec-search")!;
  const controlledEl = host.querySelector<HTMLInputElement>("#rec-controlled")!;
  const tableHost = host.querySelector<HTMLElement>("#rec-table")!;
  const libroHost = host.querySelector<HTMLElement>("#rec-libro")!;
  const modalHost = host.querySelector<HTMLElement>("#rec-modal-host")!;
  const newBtn = host.querySelector<HTMLButtonElement>("#rec-new")!;
  const exportBtn = host.querySelector<HTMLButtonElement>("#rec-export")!;

  const rut = () => searchEl.value.trim() || undefined;

  async function reloadList(): Promise<void> {
    tableHost.innerHTML = tableSkeleton();
    try {
      const rows = await listPrescriptions(
        serverUrl,
        rut(),
        controlledEl.checked || undefined,
        PAGE_LIMIT,
      );
      renderTable(tableHost, rows, false);
    } catch (err) {
      tableHost.innerHTML = renderError(err);
    }
  }

  async function reloadLibro(): Promise<void> {
    libroHost.innerHTML = tableSkeleton(5);
    try {
      const rows = await libroRecetas(serverUrl, rut(), PAGE_LIMIT);
      renderTable(libroHost, rows, true);
    } catch (err) {
      libroHost.innerHTML = renderError(err);
    }
  }

  const reloadBoth = () => {
    void reloadList();
    void reloadLibro();
  };

  let timer: number | undefined;
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(reloadBoth, 240);
  });
  controlledEl.addEventListener("change", () => void reloadList());

  newBtn.addEventListener("click", () => {
    openPrescriptionModal(modalHost, serverUrl, () => reloadBoth());
  });

  exportBtn.addEventListener("click", () => void doExport(serverUrl, rut(), exportBtn, libroHost));

  reloadBoth();
}

function renderTable(host: HTMLElement, rows: Prescription[], ledger: boolean): void {
  if (rows.length === 0) {
    host.innerHTML = `<p class="empty">${
      ledger ? "Sin recetas controladas registradas." : "Sin recetas para el filtro seleccionado."
    }</p>`;
    return;
  }
  // The ledger is controlled-only, so the "Tipo" column is redundant there.
  const tipoHead = ledger ? "" : "<th>Tipo</th>";
  host.innerHTML = `
    <table class="data-table">
      <thead>
        <tr>
          <th>Fecha</th>
          <th>Paciente</th>
          <th>RUT</th>
          <th>Médico</th>
          <th>Folio</th>
          ${tipoHead}
        </tr>
      </thead>
      <tbody>${rows.map((r) => prescriptionRow(r, ledger)).join("")}</tbody>
    </table>
    <p class="table-foot muted">${rows.length} receta(s)${
      rows.length === PAGE_LIMIT ? ` · mostrando las primeras ${PAGE_LIMIT}` : ""
    }</p>
  `;
}

function prescriptionRow(r: Prescription, ledger: boolean): string {
  const doctor = [r.doctor_name, r.doctor_rut].filter(Boolean).join(" · ") || "—";
  const tipo = r.controlled
    ? `<span class="pill pill-danger">Controlado</span>`
    : `<span class="pill pill-ok">Normal</span>`;
  const tipoCell = ledger ? "" : `<td>${tipo}</td>`;
  return `
    <tr>
      <td><span class="muted">${escapeHtml(fmtDate(r.dispensed_at))}</span></td>
      <td><div class="cell-main">${escapeHtml(r.patient_name)}</div></td>
      <td><span class="muted">${escapeHtml(r.patient_rut)}</span></td>
      <td><div class="cell-sub muted">${escapeHtml(doctor)}</div></td>
      <td><span class="muted">${escapeHtml(r.folio ?? "—")}</span></td>
      ${tipoCell}
    </tr>
  `;
}

/** "Nueva receta" modal. A controlled entry forces médico name + RUT (the server
 *  rejects otherwise — we catch it client-side first). `onSaved` fires after a
 *  successful create. */
function openPrescriptionModal(
  modalHost: HTMLElement,
  serverUrl: string,
  onSaved: () => void,
): void {
  modalHost.innerHTML = `
    <div class="modal-backdrop" id="rec-modal-backdrop">
      <div class="modal">
        <div class="modal-title">Nueva receta</div>
        <div class="modal-field">
          <label class="modal-label" for="rec-f-patient">Paciente</label>
          <input id="rec-f-patient" type="text" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="rec-f-rut">RUT del paciente</label>
          <input id="rec-f-rut" type="text" autocomplete="off" />
          <div id="rec-f-rut-hint" class="field-hint" hidden></div>
        </div>
        <label class="rec-toggle modal-field">
          <input id="rec-f-controlled" type="checkbox" /> Medicamento controlado (Ley 20.000)
        </label>
        <div class="modal-field">
          <label class="modal-label" for="rec-f-doctor">Médico</label>
          <input id="rec-f-doctor" type="text" autocomplete="off" />
        </div>
        <div class="modal-field">
          <label class="modal-label" for="rec-f-doctor-rut">RUT del médico</label>
          <input id="rec-f-doctor-rut" type="text" autocomplete="off" />
          <div id="rec-f-doctor-rut-hint" class="field-hint" hidden></div>
        </div>
        <div class="modal-field">
          <label class="modal-label" for="rec-f-folio">Folio</label>
          <input id="rec-f-folio" type="text" autocomplete="off" />
        </div>
        <div id="rec-f-error" class="form-error" hidden></div>
        <div class="modal-actions">
          <button type="button" class="btn-ghost" id="rec-f-cancel">Cancelar</button>
          <button type="button" class="btn-primary modal-confirm" id="rec-f-save">
            <span class="btn-label">Crear</span>
          </button>
        </div>
      </div>
    </div>
  `;

  const close = () => (modalHost.innerHTML = "");
  const patientEl = modalHost.querySelector<HTMLInputElement>("#rec-f-patient")!;
  const rutEl = modalHost.querySelector<HTMLInputElement>("#rec-f-rut")!;
  const controlledEl = modalHost.querySelector<HTMLInputElement>("#rec-f-controlled")!;
  const doctorEl = modalHost.querySelector<HTMLInputElement>("#rec-f-doctor")!;
  const doctorRutEl = modalHost.querySelector<HTMLInputElement>("#rec-f-doctor-rut")!;
  const folioEl = modalHost.querySelector<HTMLInputElement>("#rec-f-folio")!;
  const errEl = modalHost.querySelector<HTMLElement>("#rec-f-error")!;
  const saveBtn = modalHost.querySelector<HTMLButtonElement>("#rec-f-save")!;

  modalHost.querySelector<HTMLButtonElement>("#rec-f-cancel")!.addEventListener("click", close);
  modalHost.querySelector<HTMLElement>("#rec-modal-backdrop")!.addEventListener("click", (e) => {
    if (e.target === e.currentTarget) close();
  });

  // Advisory mód-11 check on both RUTs. Unlike the DTE emisor/receptor (where a
  // bad RUT is blocked), a prescription is registry data — a patient might be a
  // foreigner without a Chilean RUT — so we only *warn*, never block the save.
  const rutHint = modalHost.querySelector<HTMLElement>("#rec-f-rut-hint")!;
  const doctorRutHint = modalHost.querySelector<HTMLElement>("#rec-f-doctor-rut-hint")!;
  const wireRutAdvice = (input: HTMLInputElement, hint: HTMLElement): void => {
    const check = () => {
      const raw = input.value.trim();
      if (!raw) {
        input.classList.remove("invalid");
        hint.hidden = true;
        hint.className = "field-hint";
        return;
      }
      if (isValidRut(raw)) {
        input.classList.remove("invalid");
        hint.hidden = false;
        hint.className = "field-hint ok";
        hint.textContent = `RUT válido — ${formatRut(raw)}`;
      } else {
        input.classList.remove("invalid");
        hint.hidden = false;
        hint.className = "field-hint err";
        hint.textContent = "El dígito verificador no calza; revísalo (puedes guardar igual).";
      }
    };
    input.addEventListener("input", check);
    input.addEventListener("blur", () => {
      if (isValidRut(input.value)) input.value = formatRut(input.value);
    });
  };
  wireRutAdvice(rutEl, rutHint);
  wireRutAdvice(doctorRutEl, doctorRutHint);

  patientEl.focus();

  saveBtn.addEventListener("click", async () => {
    const patientName = patientEl.value.trim();
    const patientRut = rutEl.value.trim();
    const controlled = controlledEl.checked;
    const doctorName = doctorEl.value.trim();
    const doctorRut = doctorRutEl.value.trim();
    const folio = folioEl.value.trim();

    const fail = (msg: string) => {
      errEl.hidden = false;
      errEl.textContent = msg;
    };
    if (patientName === "") return fail("El nombre del paciente es obligatorio.");
    if (patientRut === "") return fail("El RUT del paciente es obligatorio.");
    if (controlled && (doctorName === "" || doctorRut === "")) {
      return fail("Una receta controlada exige nombre y RUT del médico.");
    }

    errEl.hidden = true;
    saveBtn.classList.add("loading");
    saveBtn.disabled = true;
    // Store the canonical `NNNNNNNN-D` form for valid RUTs (so the ledger and
    // patient search are consistent); keep whatever was typed otherwise.
    const input: NewPrescriptionInput = {
      patientName,
      patientRut: isValidRut(patientRut) ? canonicalRut(patientRut) : patientRut,
      controlled,
      doctorName: doctorName || undefined,
      doctorRut: doctorRut ? (isValidRut(doctorRut) ? canonicalRut(doctorRut) : doctorRut) : undefined,
      folio: folio || undefined,
    };
    try {
      await createPrescription(serverUrl, input);
      close();
      onSaved();
    } catch (err) {
      saveBtn.classList.remove("loading");
      saveBtn.disabled = false;
      fail(asMessage(err));
    }
  });
}

/** Fetch the controlled-drug ledger CSV and trigger a browser download. */
async function doExport(
  serverUrl: string,
  patientRut: string | undefined,
  btn: HTMLButtonElement,
  libroHost: HTMLElement,
): Promise<void> {
  btn.classList.add("loading");
  btn.disabled = true;
  try {
    const csv = await exportLibroRecetas(serverUrl, patientRut);
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "libro-recetas.csv";
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  } catch (err) {
    libroHost.innerHTML = renderError(err);
  } finally {
    btn.classList.remove("loading");
    btn.disabled = false;
  }
}

function renderError(err: unknown): string {
  const msg = asMessage(err);
  if (msg.includes("403") || msg.includes("denegado")) {
    return `
      <div class="caja-empty">
        <div class="caja-empty-mark">●</div>
        <h3>Sin acceso a recetas</h3>
        <p class="muted">Tu rol no tiene permiso para registrar o ver recetas. Contacta al administrador.</p>
      </div>
    `;
  }
  return `<div class="view-error">${escapeHtml(msg)}</div>`;
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("es-CL", { day: "2-digit", month: "2-digit", year: "2-digit" });
}
