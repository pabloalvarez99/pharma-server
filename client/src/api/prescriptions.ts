// Prescriptions / recetas wrappers, Ley 20.000
// (client/src-tauri/src/commands/prescriptions.rs).
import { invoke } from "@tauri-apps/api/core";

/** A prescription row (`PrescriptionDto`). Immutable per Ley 20.000 — the server
 *  exposes only create/get/list (never update/delete). `product`/`customer` are
 *  optional record ids; `controlled = true` marks a controlled-drug entry, which
 *  the server requires `doctor_name` + `doctor_rut` for. Dates are RFC3339. */
export interface Prescription {
  id: string;
  product: string | null;
  customer: string | null;
  patient_name: string;
  patient_rut: string;
  doctor_name: string | null;
  doctor_rut: string | null;
  controlled: boolean;
  folio: string | null;
  dispensed_at: string;
  created_at: string;
}

/** Fields the "Nueva receta" form collects. `patientName`/`patientRut` are
 *  required; `doctorName`/`doctorRut` are required by the server when
 *  `controlled` is true. Empty optionals are dropped server-side. */
export interface NewPrescriptionInput {
  patientName: string;
  patientRut: string;
  controlled: boolean;
  doctorName?: string;
  doctorRut?: string;
  product?: string;
  customer?: string;
  folio?: string;
}

/** GET /api/v1/prescriptions (Bearer). Optional `patientRut` / `controlled`
 *  (true → only controlled) / `limit` filters. Newest first server-side. */
export function listPrescriptions(
  serverUrl: string,
  patientRut?: string,
  controlled?: boolean,
  limit?: number,
): Promise<Prescription[]> {
  return invoke<Prescription[]>("list_prescriptions", {
    serverUrl,
    patientRut,
    controlled,
    limit,
  });
}

/** GET /api/v1/prescriptions/{id} (Bearer) — single prescription detail. */
export function getPrescription(serverUrl: string, id: string): Promise<Prescription> {
  return invoke<Prescription>("get_prescription", { serverUrl, id });
}

/** POST /api/v1/prescriptions (Bearer, pharmacist+) — register a prescription.
 *  Rejects with a Spanish string ("Permiso denegado…" on a non-pharmacist 403). */
export function createPrescription(
  serverUrl: string,
  input: NewPrescriptionInput,
): Promise<Prescription> {
  return invoke<Prescription>("create_prescription", {
    serverUrl,
    patientName: input.patientName,
    patientRut: input.patientRut,
    controlled: input.controlled,
    doctorName: input.doctorName,
    doctorRut: input.doctorRut,
    product: input.product,
    customer: input.customer,
    folio: input.folio,
  });
}

/** GET /api/v1/libro-recetas (Bearer) — controlled-only ledger (Ley 20.000).
 *  Same shape as {@link listPrescriptions} but `controlled = true` is enforced
 *  server-side. Optional `patientRut` / `limit` filters. */
export function libroRecetas(
  serverUrl: string,
  patientRut?: string,
  limit?: number,
): Promise<Prescription[]> {
  return invoke<Prescription[]>("libro_recetas", { serverUrl, patientRut, limit });
}

/** GET /api/v1/libro-recetas/export (Bearer) — raw CSV text of the controlled
 *  ledger (ISP/DEIS). The webview wraps it in a Blob for download. */
export function exportLibroRecetas(serverUrl: string, patientRut?: string): Promise<string> {
  return invoke<string>("export_libro_recetas", { serverUrl, patientRut });
}
