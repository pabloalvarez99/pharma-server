//! Customers persistence. Pure async fns over `&Db`. Every query is
//! tenant-scoped: callers pass the tenant `Thing` extracted from the JWT.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Map, Value};
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

// --- DB rows ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CustomerRow {
    id: Thing,
    name: String,
    rut: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    loyalty_points: i64,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CustomerRow> for CustomerDto {
    fn from(r: CustomerRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            rut: r.rut,
            phone: r.phone,
            email: r.email,
            loyalty_points: r.loyalty_points,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LoyaltyTxRow {
    id: Thing,
    customer: Thing,
    delta: i64,
    reason: String,
    #[serde(rename = "ref")]
    ref_id: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<LoyaltyTxRow> for LoyaltyTxDto {
    fn from(r: LoyaltyTxRow) -> Self {
        Self {
            id: r.id.to_string(),
            customer: r.customer.to_string(),
            delta: r.delta,
            reason: r.reason,
            ref_id: r.ref_id,
            created_at: r.created_at,
        }
    }
}

// --- customers -------------------------------------------------------------

pub async fn rut_exists(
    db: &Db,
    tenant: &Thing,
    rut: &str,
    except: Option<&Thing>,
) -> DomainResult<bool> {
    let mut r = if let Some(id) = except {
        db.query("SELECT id FROM customer WHERE tenant = $t AND rut = $rut AND id != $id LIMIT 1")
            .bind(("t", tenant.clone()))
            .bind(("rut", rut.to_string()))
            .bind(("id", id.clone()))
            .await?
    } else {
        db.query("SELECT id FROM customer WHERE tenant = $t AND rut = $rut LIMIT 1")
            .bind(("t", tenant.clone()))
            .bind(("rut", rut.to_string()))
            .await?
    };
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn create_customer(
    db: &Db,
    tenant: &Thing,
    input: &NewCustomer,
) -> DomainResult<CustomerDto> {
    let mut r = db
        .query(
            "CREATE customer SET tenant = $t, name = $name, rut = $rut, \
             phone = $phone, email = $email RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", input.name.clone()))
        .bind(("rut", input.rut.clone()))
        .bind(("phone", input.phone.clone()))
        .bind(("email", input.email.clone()))
        .await?;
    let row: Option<CustomerRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_customers(
    db: &Db,
    tenant: &Thing,
    f: &CustomerFilters,
) -> DomainResult<Vec<CustomerDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.search.is_some() {
        conds.push(
            "(string::lowercase(name) CONTAINS $q OR rut = $raw_q OR phone = $raw_q)".to_string(),
        );
    }
    if f.active.is_some() {
        conds.push("active = $active".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM customer WHERE {} ORDER BY name LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind((
            "q",
            f.search
                .as_deref()
                .map(|s| s.to_lowercase())
                .unwrap_or_default(),
        ))
        .bind(("raw_q", f.search.clone().unwrap_or_default()))
        .bind(("active", f.active.unwrap_or(true)))
        .await?;
    let rows: Vec<CustomerRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_customer(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<CustomerDto>> {
    let mut r = db
        .query("SELECT * FROM customer WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<CustomerRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn update_customer(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateCustomer,
) -> DomainResult<CustomerDto> {
    let mut m = Map::new();
    if let Some(v) = &patch.name {
        m.insert("name".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.rut {
        m.insert("rut".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.phone {
        m.insert("phone".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.email {
        m.insert("email".into(), Value::String(v.clone()));
    }
    if let Some(v) = patch.active {
        m.insert("active".into(), Value::Bool(v));
    }
    if m.is_empty() {
        return get_customer(db, tenant, id)
            .await?
            .ok_or(DomainError::NotFound);
    }
    let mut r = db
        .query("UPDATE customer MERGE $p WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("p", Value::Object(m)))
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<CustomerRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn soft_delete_customer(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("UPDATE customer SET active = false WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

// --- loyalty (read-only here; sales Fase 4 writes) -------------------------

pub async fn list_loyalty(
    db: &Db,
    tenant: &Thing,
    f: &LoyaltyFilters,
) -> DomainResult<Vec<LoyaltyTxDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.customer.is_some() {
        conds.push("customer = $c".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM loyalty_transaction WHERE {} ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let customer = f
        .customer
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("c", customer))
        .await?;
    let rows: Vec<LoyaltyTxRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn loyalty_stats(db: &Db, tenant: &Thing) -> DomainResult<LoyaltyStats> {
    #[derive(Deserialize)]
    struct Agg {
        members: i64,
        points_outstanding: Option<i64>,
    }
    let mut r = db
        .query(
            "SELECT count() AS members, \
             math::sum(loyalty_points) AS points_outstanding \
             FROM customer WHERE tenant = $t AND active = true GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .await?;
    let agg: Option<Agg> = r.take(0)?;
    let agg = agg.unwrap_or(Agg {
        members: 0,
        points_outstanding: None,
    });

    let mut top_r = db
        .query(
            "SELECT id, name, loyalty_points AS points FROM customer \
             WHERE tenant = $t AND active = true AND loyalty_points > 0 \
             ORDER BY loyalty_points DESC LIMIT 5",
        )
        .bind(("t", tenant.clone()))
        .await?;
    #[derive(Deserialize)]
    struct TopRow {
        id: Thing,
        name: String,
        points: i64,
    }
    let tops: Vec<TopRow> = top_r.take(0)?;

    Ok(LoyaltyStats {
        members: agg.members,
        points_outstanding: agg.points_outstanding.unwrap_or(0),
        points_earned_30d: 0,
        points_redeemed_30d: 0,
        top_customers: tops
            .into_iter()
            .map(|t| LoyaltyTopCustomer {
                id: t.id.to_string(),
                name: t.name,
                points: t.points,
            })
            .collect(),
        pending_sales_integration: true,
    })
}
