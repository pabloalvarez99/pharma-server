//! Audit-log query endpoint (BACKLOG #8).
//!
//! `GET /api/v1/audit-log` — tenant-scoped, paginated, filterable read over
//! the append-only `audit_log` table written by [`crate::middleware::audit`].
//!
//! Role gate: `admin` / `owner` only. Cashiers and pharmacists never see the
//! log (sensitive operator data: paths, payload hashes, IP, UA).
//!
//! ## Schema mapping
//!
//! The migration (`migrations/0002_audit_log.surql`) is HTTP-request-shaped,
//! not row-level (no `action` / `table_name` / `record_id` columns); each
//! audit row captures one mutating HTTP call. We expose filters that align
//! with that reality:
//!
//! | Query param | DB column      | Semantics                              |
//! |-------------|---------------|-----------------------------------------|
//! | `from`      | `created_at`   | inclusive lower bound                  |
//! | `to`        | `created_at`   | exclusive upper bound                  |
//! | `user_id`   | `user`         | record link, exact match               |
//! | `method`    | `method`       | HTTP verb (`POST`/`PATCH`/`DELETE`)    |
//! | `path`      | `path`         | URL path, exact match (e.g. `/api/v1/pos/sale`) |
//! | `status`    | `status`       | response code (e.g. 200, 422)          |
//! | `limit`     | —              | 1..=500, default 100                   |
//! | `offset`    | —              | ≥0, default 0                          |
//!
//! All filters AND together. Tenant scope (`tenant = $t`) is mandatory and
//! injected first — never replaced by a caller-supplied param.
//!
//! ## Safety
//!
//! Column names are literal strings inside this module (never interpolated
//! from user input); values bind via SurrealDB `$param` placeholders. `LIMIT`
//! and `START` are clamped integers safe to inline.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

const READ_ROLES: &[&str] = &["admin", "owner"];
const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn require_admin(claims: &auth::Claims) -> Result<(), ApiError> {
    if claims
        .roles
        .iter()
        .any(|r| READ_ROLES.contains(&r.as_str()))
    {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route("/api/v1/audit-log", get(list_audit_log))
}

#[derive(Debug, Deserialize)]
pub struct AuditFilters {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// JWT user record id (e.g. `user:abc`) — exact match on `audit_log.user`.
    pub user_id: Option<String>,
    /// HTTP method (`POST`, `PATCH`, `PUT`, `DELETE`).
    pub method: Option<String>,
    /// Exact-match URL path (e.g. `/api/v1/pos/sale`).
    pub path: Option<String>,
    /// HTTP status code returned by the original request (e.g. 200, 422).
    pub status: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Row as decoded from `audit_log`. `Thing` fields are stringified for the
/// wire response.
#[derive(Debug, Deserialize)]
struct AuditRow {
    id: Thing,
    tenant: Thing,
    user: Option<Thing>,
    user_email: Option<String>,
    method: String,
    path: String,
    status: Option<i64>,
    payload_hash: Option<String>,
    ip: Option<String>,
    user_agent: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub tenant: String,
    pub user: Option<String>,
    pub user_email: Option<String>,
    pub method: String,
    pub path: String,
    pub status: Option<i64>,
    pub payload_hash: Option<String>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<AuditRow> for AuditLogEntry {
    fn from(r: AuditRow) -> Self {
        Self {
            id: r.id.to_string(),
            tenant: r.tenant.to_string(),
            user: r.user.map(|t| t.to_string()),
            user_email: r.user_email,
            method: r.method,
            path: r.path,
            status: r.status,
            payload_hash: r.payload_hash,
            ip: r.ip,
            user_agent: r.user_agent,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AuditLogResponse {
    pub items: Vec<AuditLogEntry>,
    /// Total rows matching the *filters* for this tenant (i.e. the count the
    /// pagination is slicing). Lets the UI render "page X of Y".
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    c: i64,
}

/// Parse `user_id` into a `Thing`. Empty or malformed → 400 (a typo
/// shouldn't silently widen the result to "all users").
fn parse_user_id(s: &str) -> Result<Thing, ApiError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(ApiError::invalid("user_id vacío"));
    }
    let thing =
        surrealdb::sql::thing(trimmed).map_err(|_| ApiError::invalid("user_id inválido"))?;
    if thing.tb != "user" {
        return Err(ApiError::invalid("user_id debe ser un record<user>"));
    }
    Ok(thing)
}

/// Validate HTTP method. Audit only stores mutations, but we accept any
/// uppercase verb to keep the filter flexible for future ops.
fn normalize_method(m: &str) -> Result<String, ApiError> {
    let up = m.trim().to_ascii_uppercase();
    if up.is_empty() {
        return Err(ApiError::invalid("method vacío"));
    }
    // ASCII letters only — keeps the parameter free of injection vectors
    // even though we bind it.
    if !up.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(ApiError::invalid("method inválido"));
    }
    Ok(up)
}

async fn list_audit_log(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<AuditFilters>,
) -> Result<Json<AuditLogResponse>, ApiError> {
    require_admin(&claims)?;
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    // Sanitize / validate inputs (also catches typoed user_ids early).
    let user_thing: Option<Thing> = filters.user_id.as_deref().map(parse_user_id).transpose()?;
    let method: Option<String> = filters
        .method
        .as_deref()
        .map(normalize_method)
        .transpose()?;
    if filters.path.as_deref().map(str::is_empty).unwrap_or(false) {
        return Err(ApiError::invalid("path vacío"));
    }
    if let (Some(f), Some(t)) = (filters.from, filters.to) {
        if f >= t {
            return Err(ApiError::invalid("rango de fechas inválido (from >= to)"));
        }
    }

    let limit = filters.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let offset = filters.offset.unwrap_or(0);

    // Build the WHERE clause. Tenant condition is the only one that's never
    // optional. Column names are local literals — never user-controlled.
    let mut conds: Vec<&'static str> = vec!["tenant = $t"];
    if filters.from.is_some() {
        conds.push("created_at >= $a");
    }
    if filters.to.is_some() {
        conds.push("created_at < $b");
    }
    if user_thing.is_some() {
        conds.push("user = $u");
    }
    if method.is_some() {
        conds.push("method = $m");
    }
    if filters.path.is_some() {
        conds.push("path = $p");
    }
    if filters.status.is_some() {
        conds.push("status = $s");
    }
    let where_clause = conds.join(" AND ");

    // Single round-trip: rows + count. `LIMIT`/`START` are inlined as
    // already-clamped integers (SurrealKv 2.x has been flaky with bound
    // params there, and the values are not user-influenced after clamping).
    let sql = format!(
        "SELECT id, tenant, user, user_email, method, path, status, \
         payload_hash, ip, user_agent, created_at \
         FROM audit_log WHERE {where_clause} \
         ORDER BY created_at DESC LIMIT {limit} START {offset}; \
         SELECT count() AS c FROM audit_log WHERE {where_clause} GROUP ALL;"
    );

    let mut qb = db.query(sql).bind(("t", tenant));
    if let Some(a) = filters.from {
        qb = qb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = filters.to {
        qb = qb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    if let Some(u) = user_thing {
        qb = qb.bind(("u", u));
    }
    if let Some(m) = method {
        qb = qb.bind(("m", m));
    }
    if let Some(p) = filters.path {
        qb = qb.bind(("p", p));
    }
    if let Some(s) = filters.status {
        qb = qb.bind(("s", s));
    }

    let mut resp = qb.await.map_err(|e| {
        tracing::error!(error = %e, "audit-log query failed");
        ApiError::internal("Error consultando el audit-log.")
    })?;
    let rows: Vec<AuditRow> = resp.take(0).map_err(|e| {
        tracing::error!(error = %e, "audit-log decode failed");
        ApiError::internal("Error decodificando el audit-log.")
    })?;
    let counts: Vec<CountRow> = resp.take(1).map_err(|e| {
        tracing::error!(error = %e, "audit-log count decode failed");
        ApiError::internal("Error contando el audit-log.")
    })?;
    let total = counts.first().map(|r| r.c.max(0) as u32).unwrap_or(0);

    Ok(Json(AuditLogResponse {
        items: rows.into_iter().map(Into::into).collect(),
        total,
        limit,
        offset,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_method_uppercases() {
        assert_eq!(normalize_method("post").unwrap(), "POST");
        assert_eq!(normalize_method(" PATCH ").unwrap(), "PATCH");
    }

    #[test]
    fn normalize_method_rejects_garbage() {
        assert!(normalize_method("").is_err());
        assert!(normalize_method("POST123").is_err());
        assert!(normalize_method("POST OR 1=1").is_err());
    }

    #[test]
    fn parse_user_id_accepts_user_thing() {
        let t = parse_user_id("user:abc").unwrap();
        assert_eq!(t.tb, "user");
    }

    #[test]
    fn parse_user_id_rejects_other_table() {
        assert!(parse_user_id("tenant:abc").is_err());
        assert!(parse_user_id("").is_err());
        assert!(parse_user_id("not-a-thing").is_err());
    }
}
