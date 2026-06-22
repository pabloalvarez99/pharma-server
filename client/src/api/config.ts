import { invoke } from "@tauri-apps/api/core";

// --- config center: usuarios y roles ----------------------------------------
//
// Maps the `crates/api/src/v1/config.rs` user-management surface. All endpoints
// are admin/owner gated server-side; a 403 surfaces as the Spanish role error.
// `roles` are the canonical English keys (`cashier`/`pharmacist`/`admin`/
// `owner`) — the UI renders Spanish labels via config-center `roleLabel`.

/** A back-office user (`config.rs::UserDto`). `password`/`tenant` never cross
 *  the wire. `roles` is the canonical English key set. */
export interface ConfigUser {
  id: string;
  email: string;
  roles: string[];
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** GET /api/v1/config/users (Bearer, admin+) — every user of the tenant. */
export function listConfigUsers(serverUrl: string): Promise<ConfigUser[]> {
  return invoke<ConfigUser[]>("config_list_users", { serverUrl });
}

/** POST /api/v1/config/users (Bearer, admin+) — create a user. The server
 *  rejects a duplicate email (409) and an `owner` role assigned by a non-owner
 *  (403); both surface as Spanish strings. */
export function createConfigUser(
  serverUrl: string,
  email: string,
  password: string,
  roles: string[],
): Promise<ConfigUser> {
  return invoke<ConfigUser>("config_create_user", { serverUrl, email, password, roles });
}

/** PATCH /api/v1/config/users/{id} (Bearer, admin+) — replace a user's roles.
 *  The server guards the last active owner (409) and owner-grant (403). */
export function updateUserRoles(
  serverUrl: string,
  id: string,
  roles: string[],
): Promise<ConfigUser> {
  return invoke<ConfigUser>("config_update_user_roles", { serverUrl, id, roles });
}

/** POST /api/v1/config/users/{id}/{disable|enable} (Bearer, admin+) — toggle a
 *  user's `active` flag. The server refuses to disable the sole active owner. */
export function setUserActive(
  serverUrl: string,
  id: string,
  active: boolean,
): Promise<ConfigUser> {
  return invoke<ConfigUser>("config_set_user_active", { serverUrl, id, active });
}

// --- config center: sucursales y cajas --------------------------------------
//
// `crates/api/src/v1/branches.rs` — tenant-scoped branch (sucursal) + register
// (caja) CRUD. Reads need any bearer token; writes are admin/owner.

/** A branch / sucursal (`branches::model::BranchDto`). */
export interface Branch {
  id: string;
  name: string;
  code: string | null;
  address: string | null;
  comuna: string | null;
  phone: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** A register / caja (`branches::model::RegisterDto`). `branch` is the owning
 *  sucursal record id (`branch:<key>`) or null when unassigned. */
export interface Register {
  id: string;
  branch: string | null;
  name: string;
  code: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** Fields the sucursal form sends. `name` required; the rest optional. */
export interface BranchInput {
  name?: string;
  code?: string;
  address?: string;
  comuna?: string;
  phone?: string;
  active?: boolean;
}

/** Fields the caja form sends. `name` required; `branch` is an optional sucursal
 *  record id; `active` only meaningful on update. */
export interface RegisterInput {
  name?: string;
  branch?: string;
  code?: string;
  active?: boolean;
}

/** GET /api/v1/sucursales (Bearer). `active=false` includes inactive ones. */
export function listBranches(serverUrl: string, active?: boolean): Promise<Branch[]> {
  return invoke<Branch[]>("list_branches", { serverUrl, active });
}

/** POST /api/v1/sucursales (Bearer, admin+) — create a branch. */
export function createBranch(serverUrl: string, input: BranchInput): Promise<Branch> {
  return invoke<Branch>("create_branch", {
    serverUrl,
    name: input.name,
    code: input.code,
    address: input.address,
    comuna: input.comuna,
    phone: input.phone,
  });
}

/** PATCH /api/v1/sucursales/{id} (Bearer, admin+) — edit a branch. Only the
 *  provided fields are forwarded; `active` toggles activar/desactivar. */
export function updateBranch(
  serverUrl: string,
  id: string,
  patch: BranchInput,
): Promise<Branch> {
  return invoke<Branch>("update_branch", {
    serverUrl,
    id,
    name: patch.name,
    code: patch.code,
    address: patch.address,
    comuna: patch.comuna,
    phone: patch.phone,
    active: patch.active,
  });
}

/** DELETE /api/v1/sucursales/{id} (Bearer, admin+) — soft-delete (deactivate). */
export function deleteBranch(serverUrl: string, id: string): Promise<void> {
  return invoke<void>("delete_branch", { serverUrl, id });
}

/** GET /api/v1/cajas (Bearer). `active` / `branch` (sucursal id) optional. */
export function listRegisters(
  serverUrl: string,
  opts: { active?: boolean; branch?: string } = {},
): Promise<Register[]> {
  return invoke<Register[]>("list_registers", {
    serverUrl,
    active: opts.active,
    branch: opts.branch,
  });
}

/** POST /api/v1/cajas (Bearer, admin+) — create a register. `branch` (if set)
 *  must belong to the tenant (404 otherwise). */
export function createRegister(serverUrl: string, input: RegisterInput): Promise<Register> {
  return invoke<Register>("create_register", {
    serverUrl,
    name: input.name,
    branch: input.branch,
    code: input.code,
  });
}

/** PATCH /api/v1/cajas/{id} (Bearer, admin+) — edit a register. */
export function updateRegister(
  serverUrl: string,
  id: string,
  patch: RegisterInput,
): Promise<Register> {
  return invoke<Register>("update_register", {
    serverUrl,
    id,
    name: patch.name,
    branch: patch.branch,
    code: patch.code,
    active: patch.active,
  });
}

/** DELETE /api/v1/cajas/{id} (Bearer, admin+) — soft-delete (deactivate). */
export function deleteRegister(serverUrl: string, id: string): Promise<void> {
  return invoke<void>("delete_register", { serverUrl, id });
}

// --- config center: respaldo (backup) ---------------------------------------
//
// `crates/api/src/v1/backup.rs`. NOT tenant-scoped — a snapshot covers the whole
// install. Admin/owner only.

/** Result of an on-demand backup (`backup.rs::BackupReport`). `bytes` is the
 *  artifact size; `path` is the absolute `.tar.gz` location on the server box. */
export interface BackupReport {
  path: string;
  bytes: number;
  sha256: string;
  started_at: string;
  duration_ms: number;
}

/** One audited backup attempt (`db::backup_log::BackupLogEntry`). Artifact
 *  fields are null on a failed attempt, where `error` carries the reason. A
 *  listed `path` may no longer exist on disk (artifacts are pruned separately). */
export interface BackupLogEntry {
  status: string; // "ok" | "failed"
  source: string; // "scheduled" | "manual" | "cli"
  path: string | null;
  bytes: number | null;
  sha256: string | null;
  duration_ms: number | null;
  error: string | null;
  started_at: string;
}

/** POST /api/v1/admin/backup (Bearer, admin+) — tar+gzip the data dir now. */
export function runBackup(serverUrl: string): Promise<BackupReport> {
  return invoke<BackupReport>("run_backup", { serverUrl });
}

/** GET /api/v1/admin/backup?limit=N (Bearer, admin+) — the snapshot audit log,
 *  newest first (default 50, max 500). */
export function listBackups(serverUrl: string, limit?: number): Promise<BackupLogEntry[]> {
  return invoke<BackupLogEntry[]>("list_backups", { serverUrl, limit });
}
