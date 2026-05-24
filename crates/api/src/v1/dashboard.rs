//! Executive dashboard aggregate (`GET /api/v1/reports/dashboard`).
//!
//! ONE auth+tenant-scoped call that the Tauri client Dashboard renders without
//! fanning out to the individual report endpoints. It does NOT introduce new
//! query logic: every figure is composed from the same `domain` service
//! functions the standalone reports use (`catalog::service::stats`,
//! `expenses::service::{sales_daily,top_products,near_expiry,margins_daily}`),
//! so the SurrealQL lives in exactly one place.
//!
//! `margen_hoy` honours the same license gate as `/reports/margins-daily`
//! (`reports.margins_daily`), but degrades the *single field* to `null` on the
//! Free tier instead of 402-ing the whole overview — the rest of the dashboard
//! is core/free and must always render.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use chrono::{Datelike, TimeZone, Utc};
use rust_decimal::Decimal;
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::catalog::service as catalog;
use domain::expenses::model::{NearExpiryFilters, SalesReportFilters, TopProductsFilters};
use domain::expenses::service as reports;

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let _ = state;
    Router::new().route("/api/v1/reports/dashboard", get(dashboard))
}

/// `[from, to]` covering "today" in UTC: `[00:00:00, 23:59:59.999999999]`.
fn today_range() -> SalesReportFilters {
    let now = Utc::now();
    let d = now.date_naive();
    let from = Utc
        .with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0)
        .single()
        .unwrap_or(now);
    let to = from + chrono::Duration::days(1) - chrono::Duration::nanoseconds(1);
    SalesReportFilters {
        from: Some(from),
        to: Some(to),
    }
}

/// `[from, to]` covering month-to-date in UTC: first of the month 00:00 → now.
fn month_range() -> SalesReportFilters {
    let now = Utc::now();
    let d = now.date_naive();
    let from = Utc
        .with_ymd_and_hms(d.year(), d.month(), 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    SalesReportFilters {
        from: Some(from),
        to: Some(now),
    }
}

async fn dashboard(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    // --- ventas_hoy / ventas_mes ------------------------------------------
    // `sales_daily` buckets by UTC day; with a single-day window it yields at
    // most one row, with a month window up to ~31 — fold them into one total.
    let today = reports::sales_daily(db.as_ref(), &tenant, today_range()).await?;
    let (mut t_orders, mut t_rev, mut t_cash, mut t_card) =
        (0i64, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for row in &today {
        t_orders += row.orders;
        t_rev += row.revenue;
        t_cash += row.cash;
        t_card += row.card;
    }

    let month = reports::sales_daily(db.as_ref(), &tenant, month_range()).await?;
    let (mut m_orders, mut m_rev) = (0i64, Decimal::ZERO);
    for row in &month {
        m_orders += row.orders;
        m_rev += row.revenue;
    }

    // --- inventario -------------------------------------------------------
    let stats = catalog::stats(db.as_ref(), &tenant).await?;

    // --- top_productos (today, 5) -----------------------------------------
    let top = reports::top_products(
        db.as_ref(),
        &tenant,
        TopProductsFilters {
            from: today_range().from,
            to: today_range().to,
            limit: Some(5),
        },
    )
    .await?;
    let top_json: Vec<serde_json::Value> = top
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "name": r.product_name,
                "qty_sold": r.qty_sold,
                "revenue": r.revenue.to_string(),
            })
        })
        .collect();

    // --- por_vencer (count near-expiry <=30d) -----------------------------
    let expiring =
        reports::near_expiry(db.as_ref(), &tenant, NearExpiryFilters { days: Some(30) }).await?;
    let por_vencer = expiring.len() as i64;

    // --- margen_hoy (license-gated; degrade to null, never 402) -----------
    let lic = state.license.load();
    let margen_hoy: serde_json::Value = if license::entitled(&lic, "reports.margins_daily") {
        let rows = reports::margins_daily(db.as_ref(), &tenant, today_range()).await?;
        let (mut rev, mut cost) = (Decimal::ZERO, Decimal::ZERO);
        for row in &rows {
            rev += row.revenue;
            cost += row.cost;
        }
        let margin = rev - cost;
        let margin_pct = if rev.is_zero() {
            Decimal::ZERO
        } else {
            (margin / rev * Decimal::from(100)).round_dp(2)
        };
        serde_json::json!({
            "revenue": rev.to_string(),
            "cost": cost.to_string(),
            "margin_pct": margin_pct.to_string(),
        })
    } else {
        serde_json::Value::Null
    };

    Ok(Json(serde_json::json!({
        "ventas_hoy": {
            "orders": t_orders,
            "revenue": t_rev.to_string(),
            "cash": t_cash.to_string(),
            "card": t_card.to_string(),
        },
        "ventas_mes": {
            "orders": m_orders,
            "revenue": m_rev.to_string(),
        },
        "inventario": {
            "total_skus": stats.total,
            "active_skus": stats.active,
            "low_stock": stats.low_stock,
            "out_of_stock": stats.out_of_stock,
            "value": stats.inventory_value.to_string(),
        },
        "top_productos": top_json,
        "por_vencer": por_vencer,
        "margen_hoy": margen_hoy,
    })))
}
