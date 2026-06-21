//! The default, offline-first [`AssistProvider`]: every answer is composed from
//! existing read-only `domain` services over the tenant's own data. No network,
//! no model, no mutation (ADR-0005 invariant 2, ADR-0016).

use async_trait::async_trait;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use rust_decimal::Decimal;
use surrealdb::sql::Thing;

use db::Db;
use domain::cash_register::{model::SessionFilters, service as caja};
use domain::catalog::{model::ProductFilters, service as catalog};
use domain::customers::model::CustomerSearchQuery;
use domain::customers::service as customers;
use domain::expenses::model::{
    ExpenseFilters, NearExpiryFilters, SalesReportFilters, TopProductsFilters,
};
use domain::expenses::service as reports;
use domain::inventory::service as inventory;
use domain::DomainResult;

use crate::intent::Intent;
use crate::provider::{Answer, AssistProvider, AssistQuery};

/// Deterministic, local provider. Stateless — holds no config.
#[derive(Debug, Default, Clone, Copy)]
pub struct Deterministic;

#[async_trait]
impl AssistProvider for Deterministic {
    async fn answer(&self, q: &AssistQuery<'_>) -> DomainResult<Answer> {
        match &q.intent {
            Intent::VentasHoy => ventas(q.db, q.tenant, &q.intent, today_range()).await,
            Intent::VentasAyer => ventas(q.db, q.tenant, &q.intent, yesterday_range()).await,
            Intent::VentasMes => ventas(q.db, q.tenant, &q.intent, month_range()).await,
            Intent::VentasMesPasado => ventas(q.db, q.tenant, &q.intent, last_month_range()).await,
            Intent::ComparativaDia => {
                comparativa(q.db, q.tenant, &q.intent, today_range(), yesterday_range()).await
            }
            Intent::ComparativaMes => {
                comparativa(q.db, q.tenant, &q.intent, month_range(), last_month_range()).await
            }
            Intent::VentasPorMetodo => ventas_por_metodo(q.db, q.tenant, &q.intent).await,
            Intent::PorVencer => por_vencer(q.db, q.tenant, &q.intent, 30).await,
            Intent::PorVencerSemana => por_vencer(q.db, q.tenant, &q.intent, 7).await,
            Intent::StockProducto(term) => stock_producto(q.db, q.tenant, &q.intent, term).await,
            Intent::CajaActual => caja_actual(q.db, q.tenant, &q.intent).await,
            Intent::TopProductos => top_productos(q.db, q.tenant, &q.intent).await,
            Intent::ClientesTop => clientes_top(q.db, q.tenant, &q.intent).await,
            Intent::BuscarCliente(term) => buscar_cliente(q.db, q.tenant, &q.intent, term).await,
            Intent::ResumenClientes => resumen_clientes(q.db, q.tenant, &q.intent).await,
            Intent::PrecioProducto(term) => precio_producto(q.db, q.tenant, &q.intent, term).await,
            Intent::MargenMes => margen_mes(q.db, q.tenant, &q.intent).await,
            Intent::MargenProducto(term) => margen_producto(q.db, q.tenant, &q.intent, term).await,
            Intent::GastosMes => gastos_mes(q.db, q.tenant, &q.intent).await,
            Intent::StockBajo => stock_bajo(q.db, q.tenant, &q.intent).await,
            Intent::ResumenInventario => resumen_inventario(q.db, q.tenant, &q.intent).await,
            Intent::Ayuda => Ok(ayuda(&q.intent)),
            Intent::Unknown => Ok(unknown(&q.intent)),
        }
    }
}

// ---- executors ---------------------------------------------------------------

async fn ventas(
    db: &Db,
    tenant: &Thing,
    intent: &Intent,
    range: SalesReportFilters,
) -> DomainResult<Answer> {
    let rows = reports::sales_daily(db, tenant, range).await?;
    let (mut orders, mut rev, mut cash, mut card) =
        (0i64, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for r in &rows {
        orders += r.orders;
        rev += r.revenue;
        cash += r.cash;
        card += r.card;
    }
    let (periodo, past) = match intent {
        Intent::VentasMes => ("este mes", false),
        Intent::VentasMesPasado => ("el mes pasado", true),
        Intent::VentasAyer => ("ayer", true),
        _ => ("hoy", false),
    };
    let text = if orders == 0 {
        if past {
            format!("No registraste ventas {periodo}.")
        } else {
            format!("No registras ventas {periodo} todavía.")
        }
    } else {
        let verb = if past { "tuviste" } else { "llevas" };
        format!(
            "{periodo} {verb} {orders} {} por {} (efectivo {}, tarjeta {}).",
            plural(orders, "venta", "ventas"),
            clp(rev),
            clp(cash),
            clp(card),
        )
    };
    Ok(
        Answer::new(intent, capitalize(&text)).with_data(serde_json::json!({
            "orders": orders,
            "revenue": rev.to_string(),
            "cash": cash.to_string(),
            "card": card.to_string(),
        })),
    )
}

async fn por_vencer(db: &Db, tenant: &Thing, intent: &Intent, days: i64) -> DomainResult<Answer> {
    let rows = reports::near_expiry(db, tenant, NearExpiryFilters { days: Some(days) }).await?;
    let ventana = if days == 7 {
        "esta semana".to_string()
    } else {
        format!("los próximos {days} días")
    };
    if rows.is_empty() {
        return Ok(
            Answer::new(intent, format!("Nada por vencer en {ventana}. 👍"))
                .with_data(serde_json::json!({ "count": 0, "days": days })),
        );
    }
    let expired = rows.iter().filter(|r| r.expired).count();
    let mut detail: Vec<String> = rows
        .iter()
        .take(3)
        .map(|r| {
            if r.expired {
                format!("{} (vencido, {} u.)", r.product_name, r.stock)
            } else {
                format!(
                    "{} (en {} {}, {} u.)",
                    r.product_name,
                    r.days_to_expiry,
                    plural(r.days_to_expiry, "día", "días"),
                    r.stock
                )
            }
        })
        .collect();
    if rows.len() > 3 {
        detail.push(format!("y {} más", rows.len() - 3));
    }
    let alerta = if expired > 0 {
        format!(
            " {expired} ya {}.",
            plural(expired as i64, "está vencido", "están vencidos")
        )
    } else {
        String::new()
    };
    let text = format!(
        "Tienes {} {} por vencer en {}: {}.{}",
        rows.len(),
        plural(rows.len() as i64, "lote", "lotes"),
        ventana,
        detail.join(", "),
        alerta,
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "count": rows.len(),
        "expired": expired,
        "days": days,
    })))
}

async fn stock_producto(
    db: &Db,
    tenant: &Thing,
    intent: &Intent,
    term: &str,
) -> DomainResult<Answer> {
    let products = catalog::list_products(
        db,
        tenant,
        ProductFilters {
            search: Some(term.to_string()),
            category: None,
            active: None,
            low_stock: None,
            limit: Some(5),
            offset: None,
        },
    )
    .await?;
    if products.is_empty() {
        return Ok(Answer::new(
            intent,
            format!("No encontré ningún producto que coincida con «{term}»."),
        ));
    }
    let best = &products[0];
    let mut text = format!(
        "{}: {} {} en stock.",
        best.name,
        best.stock,
        plural(best.stock, "unidad", "unidades")
    );
    if products.len() > 1 {
        let otros: Vec<String> = products[1..]
            .iter()
            .map(|p| format!("{} ({})", p.name, p.stock))
            .collect();
        text.push_str(&format!(" También coinciden: {}.", otros.join(", ")));
    }
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "matches": products.iter().map(|p| serde_json::json!({
            "name": p.name,
            "stock": p.stock,
            "price": p.price.to_string(),
        })).collect::<Vec<_>>(),
    })))
}

async fn caja_actual(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let sessions = caja::list_sessions(
        db,
        tenant,
        SessionFilters {
            status: Some("open".to_string()),
            user: None,
            limit: Some(1),
            offset: None,
        },
    )
    .await?;
    let Some(session) = sessions.into_iter().next() else {
        return Ok(
            Answer::new(intent, "No hay ninguna caja abierta en este momento.")
                .with_data(serde_json::json!({ "open": false })),
        );
    };
    let (s, cash_sales, m_in, m_out, expected) =
        caja::compute_summary(db, tenant, &session.id).await?;
    let text = format!(
        "En la caja «{}» deberían haber {} (apertura {} + ventas efectivo {} + ingresos {} − retiros {}).",
        s.register_name,
        clp(expected),
        clp(s.opening_cash),
        clp(cash_sales),
        clp(m_in),
        clp(m_out),
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "open": true,
        "register": s.register_name,
        "expected": expected.to_string(),
        "opening": s.opening_cash.to_string(),
        "cash_sales": cash_sales.to_string(),
        "movements_in": m_in.to_string(),
        "movements_out": m_out.to_string(),
    })))
}

async fn top_productos(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let rows = reports::top_products(
        db,
        tenant,
        TopProductsFilters {
            from: month_range().from,
            to: month_range().to,
            limit: Some(5),
        },
    )
    .await?;
    if rows.is_empty() {
        return Ok(Answer::new(
            intent,
            "Aún no hay ventas este mes para armar un ranking.",
        ));
    }
    let listado: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "{}. {} ({} u., {})",
                r.rank,
                r.product_name,
                r.qty_sold,
                clp(r.revenue)
            )
        })
        .collect();
    let text = format!("Más vendidos este mes: {}.", listado.join("; "));
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "top": rows.iter().map(|r| serde_json::json!({
            "rank": r.rank,
            "name": r.product_name,
            "qty": r.qty_sold,
            "revenue": r.revenue.to_string(),
        })).collect::<Vec<_>>(),
    })))
}

async fn margen_mes(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let rows = reports::margins_daily(db, tenant, month_range()).await?;
    let (mut rev, mut cost) = (Decimal::ZERO, Decimal::ZERO);
    for r in &rows {
        rev += r.revenue;
        cost += r.cost;
    }
    let margin = rev - cost;
    let pct = if rev.is_zero() {
        Decimal::ZERO
    } else {
        (margin / rev * Decimal::from(100)).round_dp(1)
    };
    let text = if rev.is_zero() {
        "No hay ventas este mes para calcular margen.".to_string()
    } else {
        format!(
            "Margen del mes: {} sobre ventas de {} (costo {}), un {}%.",
            clp(margin),
            clp(rev),
            clp(cost),
            pct,
        )
    };
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "revenue": rev.to_string(),
        "cost": cost.to_string(),
        "margin": margin.to_string(),
        "margin_pct": pct.to_string(),
    })))
}

async fn stock_bajo(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let report = inventory::reorder_suggestions(db, tenant).await?;
    if report.rows.is_empty() {
        return Ok(Answer::new(intent, "Ningún producto bajo el mínimo. 👍")
            .with_data(serde_json::json!({ "count": 0 })));
    }
    let mut detail: Vec<String> = report
        .rows
        .iter()
        .take(5)
        .map(|r| format!("{} ({} u., sugerido +{})", r.name, r.stock, r.suggested_qty))
        .collect();
    if report.rows.len() > 5 {
        detail.push(format!("y {} más", report.rows.len() - 5));
    }
    let text = format!(
        "{} {} bajo el mínimo: {}.",
        report.rows.len(),
        plural(report.rows.len() as i64, "producto", "productos"),
        detail.join(", "),
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "count": report.rows.len(),
    })))
}

async fn resumen_inventario(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let s = catalog::stats(db, tenant).await?;
    let text = format!(
        "Tienes {} productos ({} activos), {} con stock bajo y {} agotados. Valor de inventario: {}.",
        s.total,
        s.active,
        s.low_stock,
        s.out_of_stock,
        clp(s.inventory_value),
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "total": s.total,
        "active": s.active,
        "low_stock": s.low_stock,
        "out_of_stock": s.out_of_stock,
        "value": s.inventory_value.to_string(),
    })))
}

/// Sum the daily-sales rollup over `range` into `(orders, revenue, cash, card)`.
async fn sum_sales(
    db: &Db,
    tenant: &Thing,
    range: SalesReportFilters,
) -> DomainResult<(i64, Decimal, Decimal, Decimal)> {
    let rows = reports::sales_daily(db, tenant, range).await?;
    let (mut orders, mut rev, mut cash, mut card) =
        (0i64, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for r in &rows {
        orders += r.orders;
        rev += r.revenue;
        cash += r.cash;
        card += r.card;
    }
    Ok((orders, rev, cash, card))
}

async fn comparativa(
    db: &Db,
    tenant: &Thing,
    intent: &Intent,
    current: SalesReportFilters,
    previous: SalesReportFilters,
) -> DomainResult<Answer> {
    let (cur_orders, cur_rev, _, _) = sum_sales(db, tenant, current).await?;
    let (prev_orders, prev_rev, _, _) = sum_sales(db, tenant, previous).await?;
    let (actual, anterior) = if matches!(intent, Intent::ComparativaMes) {
        ("este mes", "el mes pasado")
    } else {
        ("hoy", "ayer")
    };
    let delta = cur_rev - prev_rev;
    let pct = if prev_rev.is_zero() {
        None
    } else {
        Some((delta / prev_rev * Decimal::from(100)).round_dp(1))
    };
    let tendencia = match pct {
        Some(p) if p > Decimal::ZERO => format!("{} más (+{}%)", clp(delta), p),
        Some(p) if p < Decimal::ZERO => format!("{} menos ({}%)", clp(delta.abs()), p),
        Some(_) => "lo mismo (0%)".to_string(),
        None if cur_rev.is_zero() => "sin datos para comparar".to_string(),
        None => format!("{} más (no había ventas {anterior})", clp(delta)),
    };
    let text = format!(
        "{} llevas {} en {} {}; {} fueron {} en {} {}. Vas {}.",
        capitalize(actual),
        clp(cur_rev),
        cur_orders,
        plural(cur_orders, "venta", "ventas"),
        anterior,
        clp(prev_rev),
        prev_orders,
        plural(prev_orders, "venta", "ventas"),
        tendencia,
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "current_revenue": cur_rev.to_string(),
        "current_orders": cur_orders,
        "previous_revenue": prev_rev.to_string(),
        "previous_orders": prev_orders,
        "delta": delta.to_string(),
        "delta_pct": pct.map(|p| p.to_string()),
    })))
}

async fn ventas_por_metodo(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let (orders, rev, cash, card) = sum_sales(db, tenant, month_range()).await?;
    if orders == 0 {
        return Ok(Answer::new(
            intent,
            "No hay ventas este mes para desglosar por método de pago.",
        )
        .with_data(serde_json::json!({ "orders": 0 })));
    }
    let share = |part: Decimal| -> Decimal {
        if rev.is_zero() {
            Decimal::ZERO
        } else {
            (part / rev * Decimal::from(100)).round_dp(1)
        }
    };
    let text = format!(
        "Este mes: efectivo {} ({}%) y tarjeta {} ({}%), sobre {} en total.",
        clp(cash),
        share(cash),
        clp(card),
        share(card),
        clp(rev),
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "revenue": rev.to_string(),
        "cash": cash.to_string(),
        "card": card.to_string(),
        "cash_pct": share(cash).to_string(),
        "card_pct": share(card).to_string(),
    })))
}

async fn clientes_top(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let stats = customers::loyalty_stats(db, tenant).await?;
    if stats.top_customers.is_empty() {
        return Ok(Answer::new(
            intent,
            "Todavía no tengo un ranking de clientes. Aparecerá cuando registres \
             ventas asociadas a clientes con puntos de fidelidad.",
        )
        .with_data(serde_json::json!({ "count": 0 })));
    }
    let listado: Vec<String> = stats
        .top_customers
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, c)| format!("{}. {} ({} pts)", i + 1, c.name, c.points))
        .collect();
    let text = format!("Tus mejores clientes: {}.", listado.join("; "));
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "count": stats.top_customers.len(),
        "members": stats.members,
        "top": stats.top_customers.iter().take(5).map(|c| serde_json::json!({
            "name": c.name,
            "points": c.points,
        })).collect::<Vec<_>>(),
    })))
}

async fn buscar_cliente(
    db: &Db,
    tenant: &Thing,
    intent: &Intent,
    term: &str,
) -> DomainResult<Answer> {
    let matches = customers::search_customers(
        db,
        tenant,
        CustomerSearchQuery {
            q: Some(term.to_string()),
        },
    )
    .await?;
    if matches.is_empty() {
        return Ok(Answer::new(
            intent,
            format!("No encontré ningún cliente que coincida con «{term}»."),
        )
        .with_data(serde_json::json!({ "count": 0 })));
    }
    let best = &matches[0];
    let mut bits = vec![best.name.clone()];
    if let Some(rut) = &best.rut {
        bits.push(format!("RUT {rut}"));
    }
    if let Some(phone) = &best.phone {
        bits.push(format!("tel. {phone}"));
    }
    bits.push(format!("{} pts", best.loyalty_points));
    let mut text = format!("{}.", bits.join(", "));
    if matches.len() > 1 {
        let otros: Vec<String> = matches[1..]
            .iter()
            .take(3)
            .map(|c| c.name.clone())
            .collect();
        text.push_str(&format!(" También coinciden: {}.", otros.join(", ")));
    }
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "count": matches.len(),
        "matches": matches.iter().take(5).map(|c| serde_json::json!({
            "id": c.id,
            "name": c.name,
            "rut": c.rut,
            "phone": c.phone,
            "loyalty_points": c.loyalty_points,
        })).collect::<Vec<_>>(),
    })))
}

async fn resumen_clientes(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let stats = customers::loyalty_stats(db, tenant).await?;
    let text = if stats.members == 0 {
        "Todavía no tienes clientes registrados.".to_string()
    } else {
        format!(
            "Tienes {} {} ({} puntos de fidelidad acumulados).",
            stats.members,
            plural(stats.members, "cliente", "clientes"),
            stats.points_outstanding,
        )
    };
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "members": stats.members,
        "points_outstanding": stats.points_outstanding,
    })))
}

async fn precio_producto(
    db: &Db,
    tenant: &Thing,
    intent: &Intent,
    term: &str,
) -> DomainResult<Answer> {
    let products = catalog::list_products(
        db,
        tenant,
        ProductFilters {
            search: Some(term.to_string()),
            limit: Some(5),
            ..Default::default()
        },
    )
    .await?;
    if products.is_empty() {
        return Ok(Answer::new(
            intent,
            format!("No encontré ningún producto que coincida con «{term}»."),
        ));
    }
    let best = &products[0];
    let mut text = format!("{} se vende a {}.", best.name, clp(best.price));
    if products.len() > 1 {
        let otros: Vec<String> = products[1..]
            .iter()
            .map(|p| format!("{} ({})", p.name, clp(p.price)))
            .collect();
        text.push_str(&format!(" También coinciden: {}.", otros.join(", ")));
    }
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "matches": products.iter().map(|p| serde_json::json!({
            "name": p.name,
            "price": p.price.to_string(),
            "stock": p.stock,
        })).collect::<Vec<_>>(),
    })))
}

async fn margen_producto(
    db: &Db,
    tenant: &Thing,
    intent: &Intent,
    term: &str,
) -> DomainResult<Answer> {
    let products = catalog::list_products(
        db,
        tenant,
        ProductFilters {
            search: Some(term.to_string()),
            category: None,
            active: None,
            low_stock: None,
            limit: Some(1),
            offset: None,
        },
    )
    .await?;
    let Some(p) = products.into_iter().next() else {
        return Ok(Answer::new(
            intent,
            format!("No encontré ningún producto que coincida con «{term}»."),
        ));
    };
    let Some(cost) = p.cost_price else {
        return Ok(Answer::new(
            intent,
            format!(
                "{} se vende a {}, pero no tiene costo registrado, así que no puedo \
                 calcular el margen. Agrega el costo en el producto.",
                p.name,
                clp(p.price)
            ),
        )
        .with_data(serde_json::json!({ "name": p.name, "price": p.price.to_string() })));
    };
    let margin = p.price - cost;
    let pct = if p.price.is_zero() {
        Decimal::ZERO
    } else {
        (margin / p.price * Decimal::from(100)).round_dp(1)
    };
    let text = format!(
        "{}: se vende a {}, cuesta {}; margen unitario {} ({}%).",
        p.name,
        clp(p.price),
        clp(cost),
        clp(margin),
        pct,
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "name": p.name,
        "price": p.price.to_string(),
        "cost": cost.to_string(),
        "margin": margin.to_string(),
        "margin_pct": pct.to_string(),
    })))
}

async fn gastos_mes(db: &Db, tenant: &Thing, intent: &Intent) -> DomainResult<Answer> {
    let range = month_range();
    let rows = reports::list_expenses(
        db,
        tenant,
        ExpenseFilters {
            category: None,
            payment_method: None,
            from: range.from,
            to: range.to,
            limit: Some(500),
            offset: None,
        },
    )
    .await?;
    if rows.is_empty() {
        return Ok(Answer::new(intent, "No registras gastos este mes.")
            .with_data(serde_json::json!({ "count": 0, "total": "0" })));
    }
    let total: Decimal = rows.iter().map(|e| e.amount).sum();
    // Aggregate by category, keep the top 3 for the prose.
    let mut by_cat: std::collections::HashMap<String, Decimal> = std::collections::HashMap::new();
    for e in &rows {
        *by_cat.entry(e.category.clone()).or_insert(Decimal::ZERO) += e.amount;
    }
    let mut cats: Vec<(String, Decimal)> = by_cat.into_iter().collect();
    cats.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
    let detalle: Vec<String> = cats
        .iter()
        .take(3)
        .map(|(c, v)| format!("{c} {}", clp(*v)))
        .collect();
    let text = format!(
        "Este mes llevas {} en gastos ({} {}). Principales: {}.",
        clp(total),
        rows.len(),
        plural(rows.len() as i64, "gasto", "gastos"),
        detalle.join(", "),
    );
    Ok(Answer::new(intent, text).with_data(serde_json::json!({
        "count": rows.len(),
        "total": total.to_string(),
        "by_category": cats.iter().map(|(c, v)| serde_json::json!({
            "category": c,
            "amount": v.to_string(),
        })).collect::<Vec<_>>(),
    })))
}

fn ayuda(intent: &Intent) -> Answer {
    Answer::new(
        intent,
        "Puedes preguntarme, por ejemplo: «¿cuánto vendí hoy?», «ventas de ayer», \
         «ventas de hoy vs ayer», «ventas del mes pasado», «ventas por método de pago», \
         «gastos del mes», «mejores clientes», «¿cuántos clientes tengo?», «busca el \
         cliente Juan», «precio de ibuprofeno», «margen de paracetamol», «¿qué se vence \
         esta semana?», «stock de ibuprofeno», «¿cuánto hay en caja?», «top productos», \
         «margen del mes», «¿qué tengo que reponer?» o «resumen de inventario». También \
         puedo registrar acciones: «registra un gasto de 5000 en arriendo», «crea un \
         cliente Juan Pérez», «crea un producto Aspirina a $1000», «cambia el precio de \
         paracetamol a $1500» o «crea una orden de compra de 10 paracetamol a Farmaltda \
         a $500».",
    )
}

fn unknown(intent: &Intent) -> Answer {
    Answer::new(
        intent,
        "No entendí la pregunta. Probá con algo como «ventas hoy», «qué se vence», \
         «stock de <producto>» o «cuánto hay en caja». Escribe «ayuda» para ver todo.",
    )
}

// ---- helpers -----------------------------------------------------------------

/// `[from, to]` covering "today" in UTC.
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

/// `[from, to]` covering all of "yesterday" in UTC.
fn yesterday_range() -> SalesReportFilters {
    let d = (Utc::now() - chrono::Duration::days(1)).date_naive();
    let from = Utc
        .with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0)
        .single()
        .unwrap_or_else(Utc::now);
    let to = from + chrono::Duration::days(1) - chrono::Duration::nanoseconds(1);
    SalesReportFilters {
        from: Some(from),
        to: Some(to),
    }
}

/// `[from, to]` covering the previous calendar month (full month) in UTC.
fn last_month_range() -> SalesReportFilters {
    let now = Utc::now();
    let (year, month) = (now.year(), now.month());
    let (py, pm) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    let from = Utc
        .with_ymd_and_hms(py, pm, 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    // First instant of the current month, minus a nanosecond = end of last month.
    let this_month_start = Utc
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    let to = this_month_start - chrono::Duration::nanoseconds(1);
    SalesReportFilters {
        from: Some(from),
        to: Some(to),
    }
}

/// `[from, to]` covering month-to-date in UTC.
fn month_range() -> SalesReportFilters {
    let now = Utc::now();
    let d = now.date_naive();
    let from: DateTime<Utc> = Utc
        .with_ymd_and_hms(d.year(), d.month(), 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    SalesReportFilters {
        from: Some(from),
        to: Some(now),
    }
}

/// Format a Decimal as Chilean pesos: rounded to whole pesos with `.` thousands
/// separators, e.g. `Decimal(1234567) -> "$1.234.567"`.
pub(crate) fn clp(v: Decimal) -> String {
    let rounded = v.round();
    let neg = rounded.is_sign_negative();
    let digits = rounded.abs().trunc().to_string();
    let mut out = String::new();
    let bytes = digits.as_bytes();
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(*ch as char);
    }
    format!("{}${}", if neg { "-" } else { "" }, out)
}

fn plural<'a>(n: i64, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 {
        one
    } else {
        many
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clp_formats_thousands() {
        assert_eq!(clp(Decimal::from(0)), "$0");
        assert_eq!(clp(Decimal::from(999)), "$999");
        assert_eq!(clp(Decimal::from(1234)), "$1.234");
        assert_eq!(clp(Decimal::from(1234567)), "$1.234.567");
        assert_eq!(clp(Decimal::from(-5000)), "-$5.000");
    }

    #[test]
    fn plural_picks_form() {
        assert_eq!(plural(1, "venta", "ventas"), "venta");
        assert_eq!(plural(0, "venta", "ventas"), "ventas");
        assert_eq!(plural(3, "venta", "ventas"), "ventas");
    }

    #[test]
    fn capitalize_first() {
        assert_eq!(capitalize("hoy llevas"), "Hoy llevas");
        assert_eq!(capitalize(""), "");
    }
}
