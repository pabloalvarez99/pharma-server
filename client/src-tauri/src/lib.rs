//! RutBusiness Client — Tauri 2 backend.
//!
//! Thin HTTP client over the running `pharma-server` (`crates/api`). The
//! command surface is split per domain under [`commands`]; shared machinery:
//!   - [`state`]  — in-memory session (JWT as `SecretString`, never on disk)
//!   - [`http`]   — shared `reqwest::Client` (timeouts), Spanish error mapping
//!   - [`types`]  — wire types mirroring the real server contract
//!
//! The JWT lives ONLY in `SessionState` (in-memory). It is never written to
//! disk — losing it on quit is intentional (re-login each launch, LoL-style).
//!
//! All user-facing error strings are in Spanish (project rule); identifiers and
//! `code` values stay English.

mod commands;
mod escpos;
mod http;
mod state;
mod types;

use tauri::Manager;

use commands::{
    assist, audit, auth, cash, catalog, customers, dte, expenses, license, pos, prescriptions,
    print, purchases, reports, rubro, seed, settings,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state::SessionState::default())
        .setup(|app| {
            // Touch the state so `Manager` import is used even if commands are
            // tree-shaken in a future refactor; also a cheap sanity init.
            let _ = app.state::<state::SessionState>();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::login,
            auth::setup_status,
            auth::setup_account,
            auth::server_health,
            auth::logout,
            license::license_status,
            catalog::list_products,
            catalog::inventory_summary,
            reports::sales_daily,
            reports::top_products,
            pos::pos_sale,
            pos::create_refund,
            pos::list_refunds,
            audit::query_audit_log,
            cash::cash_sessions,
            cash::open_cash_session,
            cash::cash_arqueo,
            cash::close_cash_session,
            settings::get_setting,
            settings::set_setting,
            customers::customer_search,
            customers::customer_detail,
            customers::customer_history,
            customers::create_customer,
            customers::update_customer,
            purchases::list_purchase_orders,
            purchases::get_purchase_order,
            purchases::list_suppliers,
            purchases::create_supplier,
            purchases::create_purchase_order,
            purchases::send_purchase_order,
            purchases::receive_purchase_order,
            purchases::get_po_payments,
            purchases::create_po_payment,
            expenses::list_expenses,
            expenses::create_expense,
            reports::margins_daily,
            reports::stock_rotation,
            reports::dashboard_report,
            prescriptions::list_prescriptions,
            prescriptions::get_prescription,
            prescriptions::create_prescription,
            prescriptions::libro_recetas,
            prescriptions::export_libro_recetas,
            pos::get_receipt,
            catalog::create_product,
            catalog::import_products,
            catalog::import_products_preview,
            catalog::export_products,
            catalog::product_detail,
            catalog::product_by_barcode,
            catalog::list_product_variants,
            catalog::create_product_variant,
            catalog::adjust_product_stock,
            catalog::list_batches,
            catalog::create_batch,
            catalog::near_expiry,
            dte::list_dtes,
            dte::dte_caf_status,
            dte::dte_xml,
            dte::dte_libro_ventas,
            dte::dte_libro_ventas_signed,
            dte::emit_documento,
            dte::emit_boleta,
            dte::send_dte,
            dte::poll_dte,
            dte::cancel_dte,
            seed::seed_demo,
            rubro::rubro_pack,
            print::print_ticket,
            print::open_cash_drawer,
            assist::assist_ask,
            assist::assist_act
        ])
        .run(tauri::generate_context!())
        .expect("error while running rutbusiness-client");
}
