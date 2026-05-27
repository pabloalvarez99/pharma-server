//! Audit-log query endpoint — `GET /api/v1/admin/audit-log`.
//!
//! Read-only, admin/owner only, tenant-scoped. Reads rows recorded by the
//! global `audit::layer` middleware (see `crates/api/src/middleware/audit.rs`
//! and migration `0002_audit_log.surql`).
//!
//! SCHEMA GAP (TODO future migration): the existing `audit_log` table records
//! `method`, `path`, `status`, `payload_hash`, `ip`, `user_agent` — it does NOT
//! store `table`, `record_id`, `before`, `after`, `metadata`. The handler
//! therefore:
//!   * derives `action` from `method` (POST→create, PUT/PATCH→update, DELETE→delete),
//!   * derives `table` from the URL path (best-effort substring match),
//!   * returns `before`/`after`/`metadata`/`record_id` as `null` until a future
//!     migration adds those columns and the middleware populates them.
//!
//! Tenant filter ALWAYS comes from the JWT claim, never from the query string.

use std::collections::HashSet;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

const ADMIN_ROLES: &[&str] = &["admin", "owner"];
const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;
const SENSITIVE_KEYS: &[&str] = &["password_hash", "jwt_secret", "pfx_encrypted"];

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route("/api/v1/admin/audit-log", get(query_audit_log))
}

#[derive(Debug, Deserialize, Default)]
pub struct AuditQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub user: Option<String>,
    pub table: Option<String>,
    pub action: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct AuditItem {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub user: Option<String>,
    pub user_email: Option<String>,
    pub table: Option<String>,
    pub record_id: Option<String>,
    pub action: String,
    pub method: String,
    pub path: String,
    pub status: Option<i64>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub payload_hash: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct AuditResponse {
    pub total: i64,
    pub items: Vec<AuditItem>,
    pub limit: u32,
    pub offset: u32,
}

fn require_admin(claims: &auth::Claims) -> Result<(), ApiError> {
    if claims
        .roles
        .iter()
        .any(|r| ADMIN_ROLES.contains(&r.as_str()))
    {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn parse_date(label: &str, raw: &str) -> Result<NaiveDate, ApiError> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| ApiError::invalid(format!("Parámetro '{label}' debe ser YYYY-MM-DD.")))
}

/// Map `action=create|update|delete` to one or more HTTP methods recorded by
/// the audit middleware. Returns `Err` for any other value (we don't silently
/// drop unknown filters — they would yield a misleading empty result).
fn action_to_methods(action: &str) -> Result<&'static [&'static str], ApiError> {
    match action.to_ascii_lowercase().as_str() {
        "create" => Ok(&["POST"]),
        "update" => Ok(&["PUT", "PATCH"]),
        "delete" => Ok(&["DELETE"]),
        other => Err(ApiError::invalid(format!(
            "Parámetro 'action' inválido: '{other}'. Use create|update|delete."
        ))),
    }
}

/// Map an HTTP method back to the public `action` label used in the response.
fn method_to_action(method: &str) -> &'static str {
    match method.to_ascii_uppercase().as_str() {
        "POST" => "create",
        "PUT" | "PATCH" => "update",
        "DELETE" => "delete",
        _ => "other",
    }
}

/// Best-effort: pull the first segment after `/api/v1/` as the table name.
/// e.g. `/api/v1/products/123` → `products`. Returns `None` for paths that
/// don't match the convention.
fn table_from_path(path: &str) -> Option<String> {
    let trimmed = path.trim_start_matches('/');
    let mut parts = trimmed.split('/');
    // expect "api" / "v1" / "<table>" / ...
    let a = parts.next()?;
    let b = parts.next()?;
    if a == "api" && b == "v1" {
        parts.next().map(|s| s.to_string())
    } else {
        Some(b.to_string())
    }
}

/// Redact any JSON object field whose key is in [`SENSITIVE_KEYS`] to the
/// string `"[REDACTED]"`. Recurses through nested objects/arrays. Pure fn.
fn redact_sensitive(v: &mut Value) {
    let sensitive: HashSet<&str> = SENSITIVE_KEYS.iter().copied().collect();
    fn walk(v: &mut Value, sensitive: &HashSet<&str>) {
        match v {
            Value::Object(map) => {
                for (k, val) in map.iter_mut() {
                    if sensitive.contains(k.as_str()) {
                        *val = Value::String("[REDACTED]".to_string());
                    } else {
                        walk(val, sensitive);
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    walk(item, sensitive);
                }
            }
            _ => {}
        }
    }
    walk(v, &sensitive);
}

#[derive(Debug, Deserialize)]
struct RawRow {
    id: Thing,
    created_at: DateTime<Utc>,
    user: Option<Thing>,
    user_email: Option<String>,
    method: String,
    path: String,
    status: Option<i64>,
    payload_hash: Option<String>,
    ip: Option<String>,
    user_agent: Option<String>,
    /// Forward-compat: surfaced as `null` until a future migration adds the
    /// column. Surreal returns the field as `None` if absent.
    #[serde(default)]
    before: Option<Value>,
    #[serde(default)]
    after: Option<Value>,
    #[serde(default)]
    metadata: Option<Value>,
    #[serde(default)]
    record_id: Option<String>,
    #[serde(default)]
    table_name: Option<String>,
}

async fn query_audit_log(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, ApiError> {
    require_admin(&claims)?;
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    // --- Validate + normalize filters -----------------------------------
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::invalid(format!(
            "Parámetro 'limit' fuera de rango (1..={MAX_LIMIT})."
        )));
    }
    let offset = q.offset.unwrap_or(0);

    let from_dt: Option<DateTime<Utc>> = match q.from.as_deref() {
        Some(s) => Some(Utc.from_utc_datetime(&parse_date("from", s)?.and_time(NaiveTime::MIN))),
        None => None,
    };
    let to_dt: Option<DateTime<Utc>> = match q.to.as_deref() {
        Some(s) => {
            // Exclusive upper bound: next-day 00:00 UTC.
            let d = parse_date("to", s)?;
            let plus = d + Duration::days(1);
            Some(Utc.from_utc_datetime(&plus.and_time(NaiveTime::MIN)))
        }
        None => None,
    };

    let user_thing: Option<Thing> = match q.user.as_deref() {
        Some(s) => Some(
            surrealdb::sql::thing(s)
                .map_err(|_| ApiError::invalid("Parámetro 'user' no es un record id válido."))?,
        ),
        None => None,
    };

    let methods: Option<&'static [&'static str]> = match q.action.as_deref() {
        Some(a) => Some(action_to_methods(a)?),
        None => None,
    };

    // --- Build WHERE clause --------------------------------------------
    let mut conds: Vec<&'static str> = vec!["tenant = $tenant"];
    if from_dt.is_some() {
        conds.push("created_at >= $from");
    }
    if to_dt.is_some() {
        conds.push("created_at < $to");
    }
    if user_thing.is_some() {
        conds.push("user = $user");
    }
    if q.table.is_some() {
        // Table mapping is path-based (schema gap). Match either segment-based
        // `/api/v1/<table>` or any path that contains `/<table>` (covers
        // legacy non-v1 routes). Both bound to the same `$table` token.
        conds.push(
            "(string::starts_with(path, $table_prefix) OR string::contains(path, $table_seg))",
        );
    }
    if let Some(methods) = methods {
        // Methods is small fixed set → inline as IN literal (no injection,
        // these come from `action_to_methods` whitelist).
        match methods.len() {
            1 => conds.push("method = $method0"),
            2 => conds.push("method IN [$method0, $method1]"),
            _ => unreachable!(),
        }
    }
    let where_clause = conds.join(" AND ");

    // --- Execute (one transaction: count + select) ----------------------
    let sql = format!(
        "SELECT count() AS c FROM audit_log WHERE {where_clause} GROUP ALL;\
         SELECT id, created_at, user, user_email, method, path, status, payload_hash, ip, user_agent, before, after, metadata, record_id, table_name \
         FROM audit_log WHERE {where_clause} ORDER BY created_at DESC LIMIT $limit START $offset;"
    );

    let mut qb = db.query(&sql).bind(("tenant", tenant));
    if let Some(from) = from_dt {
        qb = qb.bind(("from", surrealdb::sql::Datetime::from(from)));
    }
    if let Some(to) = to_dt {
        qb = qb.bind(("to", surrealdb::sql::Datetime::from(to)));
    }
    if let Some(u) = user_thing {
        qb = qb.bind(("user", u));
    }
    if let Some(t) = q.table.as_ref() {
        qb = qb.bind(("table_prefix", format!("/api/v1/{t}")));
        qb = qb.bind(("table_seg", format!("/{t}")));
    }
    if let Some(methods) = methods {
        qb = qb.bind(("method0", methods[0].to_string()));
        if methods.len() > 1 {
            qb = qb.bind(("method1", methods[1].to_string()));
        }
    }
    qb = qb
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64));

    let mut res = qb.await.map_err(|e| {
        tracing::error!(error = %e, "audit-log: query failed");
        ApiError::internal("Error consultando audit_log.")
    })?;

    #[derive(Deserialize)]
    struct CountRow {
        c: i64,
    }
    let count_rows: Vec<CountRow> = res.take(0).map_err(|e| {
        tracing::error!(error = %e, "audit-log: count decode failed");
        ApiError::internal("Error decodificando audit_log count.")
    })?;
    let total = count_rows.first().map(|r| r.c).unwrap_or(0);

    let rows: Vec<RawRow> = res.take(1).map_err(|e| {
        tracing::error!(error = %e, "audit-log: rows decode failed");
        ApiError::internal("Error decodificando audit_log rows.")
    })?;

    let items: Vec<AuditItem> = rows
        .into_iter()
        .map(|r| {
            let mut before = r.before;
            let mut after = r.after;
            let mut metadata = r.metadata;
            if let Some(v) = before.as_mut() {
                redact_sensitive(v);
            }
            if let Some(v) = after.as_mut() {
                redact_sensitive(v);
            }
            if let Some(v) = metadata.as_mut() {
                redact_sensitive(v);
            }
            let derived_table = r.table_name.clone().or_else(|| table_from_path(&r.path));
            let action = method_to_action(&r.method).to_string();
            AuditItem {
                id: r.id.to_string(),
                created_at: r.created_at,
                user: r.user.map(|t| t.to_string()),
                user_email: r.user_email,
                table: derived_table,
                record_id: r.record_id,
                action,
                method: r.method,
                path: r.path,
                status: r.status,
                ip: r.ip,
                user_agent: r.user_agent,
                payload_hash: r.payload_hash,
                before,
                after,
                metadata,
            }
        })
        .collect();

    // Silence unused-in-some-configs.
    let _ = json!(null);

    Ok(Json(AuditResponse {
        total,
        items,
        limit,
        offset,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_mapping_create() {
        assert_eq!(action_to_methods("create").unwrap(), &["POST"]);
    }

    #[test]
    fn action_mapping_update_covers_put_and_patch() {
        let m = action_to_methods("update").unwrap();
        assert!(m.contains(&"PUT") && m.contains(&"PATCH"));
    }

    #[test]
    fn action_mapping_delete() {
        assert_eq!(action_to_methods("delete").unwrap(), &["DELETE"]);
    }

    #[test]
    fn action_mapping_unknown_rejected() {
        let err = action_to_methods("PURGE").unwrap_err();
        assert_eq!(err.code, "INVALID_INPUT");
    }

    #[test]
    fn method_to_action_table() {
        assert_eq!(method_to_action("POST"), "create");
        assert_eq!(method_to_action("PUT"), "update");
        assert_eq!(method_to_action("patch"), "update");
        assert_eq!(method_to_action("DELETE"), "delete");
        assert_eq!(method_to_action("GET"), "other");
    }

    #[test]
    fn table_from_path_basic() {
        assert_eq!(
            table_from_path("/api/v1/products/p:1").as_deref(),
            Some("products")
        );
        assert_eq!(
            table_from_path("/api/v1/admin/audit-log").as_deref(),
            Some("admin")
        );
        assert_eq!(table_from_path("/").as_deref(), None);
    }

    #[test]
    fn redact_walks_nested() {
        let mut v = json!({
            "password_hash": "abc",
            "nested": {
                "jwt_secret": "xyz",
                "ok": "keep"
            },
            "list": [{"pfx_encrypted": "p"}, {"safe": "no"}]
        });
        redact_sensitive(&mut v);
        assert_eq!(v["password_hash"], "[REDACTED]");
        assert_eq!(v["nested"]["jwt_secret"], "[REDACTED]");
        assert_eq!(v["nested"]["ok"], "keep");
        assert_eq!(v["list"][0]["pfx_encrypted"], "[REDACTED]");
        assert_eq!(v["list"][1]["safe"], "no");
    }

    #[test]
    fn parse_date_rejects_garbage() {
        assert!(parse_date("from", "not-a-date").is_err());
        assert!(parse_date("from", "2026-05-23").is_ok());
    }
}
