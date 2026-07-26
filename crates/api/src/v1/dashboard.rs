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

use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
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

/// Roles allowed to read the executive overview. Excludes `cajero`: the
/// dashboard surfaces revenue/margin figures meant for management, matching the
/// role posture of the other report endpoints.
const DASHBOARD_ROLES: &[&str] = &["admin", "owner", "quimico"];

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/reports/dashboard", get(dashboard))
        .route_layer(crate::role::layer(state, DASHBOARD_ROLES))
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
) -> Result<Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;

    // "today" bounds computed once and reused by the sales, top-products and
    // margin queries (was previously recomputed three times).
    let tr = today_range();
    let (t_from, t_to) = (tr.from, tr.to);

    // Fan out the independent read-only aggregates concurrently. All four
    // futures hold only shared borrows of `db`/`tenant`; the first error
    // short-circuits. Wall-clock is ~max(calls) instead of the sum.
    let (today, month, stats, top, expiring) = tokio::try_join!(
        reports::sales_daily(
            db.as_ref(),
            &tenant,
            SalesReportFilters {
                from: t_from,
                to: t_to,
            },
        ),
        reports::sales_daily(db.as_ref(), &tenant, month_range()),
        catalog::stats(db.as_ref(), &tenant),
        reports::top_products(
            db.as_ref(),
            &tenant,
            TopProductsFilters {
                from: t_from,
                to: t_to,
                limit: Some(5),
            },
        ),
        reports::near_expiry(
            db.as_ref(),
            &tenant,
            NearExpiryFilters {
                days: Some(30),
                // Global a propósito: el dashboard es la foto del negocio entero;
                // el filtro por local vive en Inventario.
                branch: None,
            }
        ),
    )?;

    // `sales_daily` buckets by UTC day; fold the (1 today / ~31 month) rows.
    let (mut t_orders, mut t_rev, mut t_cash, mut t_card) =
        (0i64, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for row in &today {
        t_orders += row.orders;
        t_rev += row.revenue;
        t_cash += row.cash;
        t_card += row.card;
    }
    let (mut m_orders, mut m_rev) = (0i64, Decimal::ZERO);
    for row in &month {
        m_orders += row.orders;
        m_rev += row.revenue;
    }

    // Emit the domain's full ranking rows (rank, revenue_pct, abc_class — the
    // ABC bucket is computed domain-side over the returned ranking) so the
    // Tauri client renders ABC badges without recomputing anything.
    let top_productos = serde_json::to_value(&top).unwrap_or_else(|_| serde_json::json!([]));
    let por_vencer = expiring.len() as i64;

    // --- margen_hoy (license-gated; degrade to null, never 402) -----------
    let lic = state.license.load();
    let margen_hoy: serde_json::Value = if license::entitled(&lic, "reports.margins_daily") {
        let rows = reports::margins_daily(
            db.as_ref(),
            &tenant,
            SalesReportFilters {
                from: t_from,
                to: t_to,
            },
        )
        .await?;
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

    let body = serde_json::json!({
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
        "top_productos": top_productos,
        "por_vencer": por_vencer,
        "margen_hoy": margen_hoy,
    });

    // Short private cache so repeated tab navigations don't re-fan-out the
    // aggregate queries on every render.
    Ok(([(header::CACHE_CONTROL, "private, max-age=30")], Json(body)).into_response())
}
