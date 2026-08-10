//! Settings context: tenant-scoped key/value (`admin_setting`).
//!
//! Los tipos puros de plata viven en [`crate::money`]; acá está la resolución
//! contra la base. La tabla `admin_setting` ya existía y la usan `dte.emisor`,
//! `business.vertical`, `web.*`, `loyalty_points_per_clp` — country pack no
//! inventa un mecanismo nuevo, se cuelga del que hay.
//!
//! Reglas de lectura, en orden:
//!
//! 1. Si el tenant no configuró nada → default CLP / 19% (o sea: un tenant
//!    chileno que ya existe no cambia en nada).
//! 2. Si configuró algo válido → eso.
//! 3. Si lo que hay guardado es basura → default + `warn!`, nunca un error.
//!    Una venta en el mostrador no puede fallar porque alguien escribió mal un
//!    setting hace tres meses. La validación va en la escritura
//!    ([`set_currency`], [`set_tax_percent`]), que es donde el operador todavía
//!    está mirando la pantalla.

use surrealdb::sql::Thing;

use crate::errors::DomainResult;
use crate::money::{
    parse_tax_percent, Currency, MoneyConfig, CURRENCY_SETTING_KEY, TAX_RATE_SETTING_KEY,
};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Resolve the tenant's currency + tax rate. Ver reglas en el módulo.
pub async fn money_config(db: &Db, tenant: &Thing) -> DomainResult<MoneyConfig> {
    let currency = currency(db, tenant).await?;

    let default_tax = MoneyConfig::default().tax_percent;
    let tax_percent = match read(db, tenant, TAX_RATE_SETTING_KEY).await? {
        Some(raw) => match parse_tax_percent(&raw) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    tenant = %tenant,
                    key = TAX_RATE_SETTING_KEY,
                    error = %e,
                    "tasa de impuesto del tenant inválida; usando el default"
                );
                default_tax
            }
        },
        None => default_tax,
    };

    Ok(MoneyConfig::new(currency, tax_percent))
}

/// Just the currency — para los lugares que solo necesitan la moneda (totales
/// del POS, respuestas del storefront, cabecera de una orden de compra) y no la
/// tasa de impuesto. Lee **una** key: la venta es la ruta caliente y no hay por
/// qué pagar dos lecturas para tirar una.
pub async fn currency(db: &Db, tenant: &Thing) -> DomainResult<Currency> {
    Ok(match read(db, tenant, CURRENCY_SETTING_KEY).await? {
        Some(raw) => match Currency::parse(&raw) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    tenant = %tenant,
                    key = CURRENCY_SETTING_KEY,
                    error = %e,
                    "moneda del tenant inválida; usando el default"
                );
                Currency::default()
            }
        },
        None => Currency::default(),
    })
}

/// Set the tenant's currency. Valida acá, no en la lectura.
pub async fn set_currency(db: &Db, tenant: &Thing, code: &str) -> DomainResult<Currency> {
    let parsed = Currency::parse(code)?;
    write(db, tenant, CURRENCY_SETTING_KEY, parsed.code()).await?;
    Ok(parsed)
}

/// Set the tenant's tax rate, in percent (`"19"`, `"21"`, `"8.25"`).
pub async fn set_tax_percent(
    db: &Db,
    tenant: &Thing,
    raw: &str,
) -> DomainResult<rust_decimal::Decimal> {
    let parsed = parse_tax_percent(raw)?;
    write(db, tenant, TAX_RATE_SETTING_KEY, &parsed.normalize().to_string()).await?;
    Ok(parsed)
}

// --- pie de boleta ---------------------------------------------------------

/// Key del pie de boleta. Vive en `admin_setting` como todo lo demás.
pub const RECEIPT_FOOTER_SETTING_KEY: &str = "receipt.footer_note";

/// Tope del pie de boleta. Una impresora térmica de 58 mm tiene 32 columnas;
/// 80 caracteres son dos líneas y media, que es todo lo que un pie puede ser
/// sin convertirse en un panfleto que se come el papel de cada venta.
pub const RECEIPT_FOOTER_MAX_LEN: usize = 80;

/// Lo que se imprime abajo de la boleta cuando el tenant no configuró nada.
///
/// Sale del **nombre del negocio que está vendiendo**, nunca de una constante.
/// Antes era `"Gracias por su compra · Tu Farmacia"` fijo: Tu Farmacia es otro
/// producto, así que cada feriante que imprimía una boleta estaba repartiendo
/// publicidad ajena, con su propio papel y su propia venta.
///
/// Sin nombre de negocio queda sólo el agradecimiento. Un pie a medio armar
/// («Gracias por su compra · ») se ve como un error de la app.
pub fn default_receipt_footer(business_name: &str) -> String {
    let name = business_name.trim();
    if name.is_empty() {
        "Gracias por su compra".to_string()
    } else {
        format!("Gracias por su compra · {name}")
    }
}

/// El pie de boleta de este tenant: lo que configuró, o el default derivado de
/// [`default_receipt_footer`]. Nunca falla — una boleta no se puede quedar sin
/// imprimir porque un setting esté raro.
pub async fn receipt_footer_note(
    db: &Db,
    tenant: &Thing,
    business_name: &str,
) -> DomainResult<String> {
    // `read` ya filtra el vacío, así que un valor borrado vuelve al default en
    // lugar de imprimir una línea en blanco.
    Ok(match read(db, tenant, RECEIPT_FOOTER_SETTING_KEY).await? {
        Some(raw) => raw.trim().to_string(),
        None => default_receipt_footer(business_name),
    })
}

/// Configura el pie de boleta. Vacío = volver al default (no se guarda un pie
/// en blanco: se borra la personalización).
///
/// Devuelve lo que va a imprimirse de ahora en más, ya resuelto, para que la
/// pantalla muestre el resultado y no lo que el operador tipeó.
pub async fn set_receipt_footer_note(
    db: &Db,
    tenant: &Thing,
    raw: &str,
    business_name: &str,
) -> DomainResult<String> {
    let note = raw.trim();
    if note.chars().count() > RECEIPT_FOOTER_MAX_LEN {
        return Err(crate::errors::DomainError::Invalid(format!(
            "el pie de boleta no puede pasar de {RECEIPT_FOOTER_MAX_LEN} caracteres"
        )));
    }
    write(db, tenant, RECEIPT_FOOTER_SETTING_KEY, note).await?;
    Ok(if note.is_empty() {
        default_receipt_footer(business_name)
    } else {
        note.to_string()
    })
}

async fn read(db: &Db, tenant: &Thing, key: &str) -> DomainResult<Option<String>> {
    Ok(crate::sales::repo::get_setting(db, tenant, key)
        .await?
        .map(|s| s.value)
        .filter(|v| !v.trim().is_empty()))
}

async fn write(db: &Db, tenant: &Thing, key: &str, value: &str) -> DomainResult<()> {
    crate::sales::repo::upsert_setting(db, tenant, key, value).await?;
    Ok(())
}
