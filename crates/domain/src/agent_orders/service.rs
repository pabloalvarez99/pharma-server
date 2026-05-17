//! Tenant-scoped reads + decision transitions for inbound `agent_order`s.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use surrealdb::sql::{thing, Thing};

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

#[derive(Debug, Deserialize)]
struct Row {
    id: Thing,
    peer_did: String,
    status: String,
    total: f64,
    currency: String,
    price_adjusted: bool,
    buyer_note: Option<String>,
    lines_json: String,
    created_at: DateTime<Utc>,
}

impl From<Row> for AgentOrderDto {
    fn from(r: Row) -> Self {
        let lines = serde_json::from_str(&r.lines_json).unwrap_or(serde_json::Value::Null);
        Self {
            id: r.id.to_string(),
            peer_did: r.peer_did,
            status: r.status,
            total: r.total,
            currency: r.currency,
            price_adjusted: r.price_adjusted,
            buyer_note: r.buyer_note,
            lines,
            created_at: r.created_at,
        }
    }
}

const SELECT_COLS: &str = "id, peer_did, status, <float> total AS total, currency, \
     price_adjusted, buyer_note, lines_json, created_at";

pub async fn list(
    db: &Db,
    tenant: &Thing,
    f: AgentOrderFilters,
) -> DomainResult<Vec<AgentOrderDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.status.is_some() {
        conds.push("status = $s".to_string());
    }
    let limit = f.limit.unwrap_or(100).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let sql = format!(
        "SELECT {SELECT_COLS} FROM agent_order WHERE {} \
         ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(s) = f.status {
        qb = qb.bind(("s", s));
    }
    let rows: Vec<Row> = qb.await?.check()?.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get(db: &Db, tenant: &Thing, id: &str) -> DomainResult<AgentOrderDto> {
    let oid =
        thing(id).map_err(|_| DomainError::Invalid(format!("agent_order id inválido: {id}")))?;
    if oid.tb != "agent_order" {
        return Err(DomainError::Invalid("id no es un agent_order".into()));
    }
    let mut r = db
        .query(format!(
            "SELECT {SELECT_COLS} FROM agent_order WHERE id = $i AND tenant = $t LIMIT 1"
        ))
        .bind(("i", oid))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let row: Option<Row> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Move an inbound order to `accepted` or `rejected`. Only legal from
/// `received` — re-deciding a settled order is a conflict, not idempotent.
pub async fn decide(
    db: &Db,
    tenant: &Thing,
    id: &str,
    new_status: &str,
) -> DomainResult<AgentOrderDto> {
    if new_status != "accepted" && new_status != "rejected" {
        return Err(DomainError::Invalid(format!(
            "transición inválida: {new_status} (esperaba accepted|rejected)"
        )));
    }
    let current = get(db, tenant, id).await?;
    if current.status != "received" {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{}' no puede pasar a '{new_status}' (solo desde 'received')",
            current.status
        )));
    }
    let oid =
        thing(id).map_err(|_| DomainError::Invalid(format!("agent_order id inválido: {id}")))?;
    db.query("UPDATE agent_order SET status = $st WHERE id = $i AND tenant = $t")
        .bind(("st", new_status.to_string()))
        .bind(("i", oid))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    get(db, tenant, id).await
}
