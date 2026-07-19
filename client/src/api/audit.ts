// Auditoría / audit-log wrapper (client/src-tauri/src/commands/audit.rs).
import { invoke } from "@tauri-apps/api/core";

/** One row of the immutable audit trail (`AuditItem`). `before`/`after`/
 *  `metadata`/`record_id`/`table` may be null (server schema gap — today the row
 *  records method/path/status/ip). `status` is the HTTP status of the request. */
export interface AuditEntry {
  id: string;
  created_at: string;
  user: string | null;
  user_email: string | null;
  table: string | null;
  record_id: string | null;
  action: string; // "create" | "update" | "delete" | "other"
  method: string;
  path: string;
  status: number | null;
  ip: string | null;
  user_agent: string | null;
  payload_hash: string | null;
  before: unknown;
  after: unknown;
  metadata: unknown;
}

/** Paginated audit-log response (`AuditResponse`). */
export interface AuditPage {
  total: number;
  items: AuditEntry[];
  limit: number;
  offset: number;
}

/** Filters for {@link queryAuditLog}. Dates are `YYYY-MM-DD`; `action` is
 *  `create|update|delete`; `user` is a record id. */
export interface AuditFilters {
  from?: string;
  to?: string;
  user?: string;
  table?: string;
  action?: string;
  limit?: number;
  offset?: number;
}

/** GET /api/v1/admin/audit-log (Bearer, admin/owner) — immutable audit trail.
 *  Rejects with a Spanish string (e.g. "Permiso denegado…" on a non-admin 403). */
export function queryAuditLog(serverUrl: string, filters: AuditFilters = {}): Promise<AuditPage> {
  return invoke<AuditPage>("query_audit_log", {
    serverUrl,
    from: filters.from,
    to: filters.to,
    user: filters.user,
    table: filters.table,
    action: filters.action,
    limit: filters.limit,
    offset: filters.offset,
  });
}
