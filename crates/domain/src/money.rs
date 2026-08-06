//! Money types and per-currency rounding.
//!
//! Use `rust_decimal::Decimal` for prices, totals, taxes, costs. JSON serializes
//! as string to avoid float drift in clients.
//!
//! # Por qué hay una moneda por tenant
//!
//! Hasta v0.2.0 la moneda era la constante `CURRENCY_CLP` y el redondeo de todo
//! el sistema era `round_dp(0)` — correcto para CLP, que no tiene centavos, y
//! silenciosamente equivocado para cualquier moneda que sí los tenga: en USD un
//! `round_dp(0)` convierte `12.50` en `12` y el negocio pierde plata en cada
//! línea. Los decimales NO son una preferencia de formato, son parte de la
//! aritmética: por eso viven acá, junto al tipo, y no en la capa de UI.
//!
//! [`Currency`] conoce el exponente ISO-4217 de cada código; [`MoneyConfig`] es
//! la moneda + la tasa de impuesto del tenant, y es lo que las capas de negocio
//! deben pedir en vez de asumir Chile. La resolución desde `admin_setting` vive
//! en [`crate::settings`] (acá no hay DB: este módulo es puro y testeable).
//!
//! El default sigue siendo CLP / 19%, así que un tenant chileno que ya existe y
//! nunca configuró nada se comporta exactamente igual que antes.

pub use rust_decimal::Decimal;

use crate::errors::{DomainError, DomainResult};

/// ISO-4217 code applied when the tenant has no `money.currency` setting.
pub const DEFAULT_CURRENCY_CODE: &str = "CLP";

/// Tax rate (percent) applied when the tenant has no `money.tax_rate` setting.
/// 19 = IVA Chile, el mercado inicial.
pub const DEFAULT_TAX_PERCENT: u8 = 19;

/// `admin_setting` key holding the tenant's ISO-4217 currency code.
pub const CURRENCY_SETTING_KEY: &str = "money.currency";

/// `admin_setting` key holding the tenant's tax rate, in percent (`"19"`,
/// `"21"`, `"8.25"`).
pub const TAX_RATE_SETTING_KEY: &str = "money.tax_rate";

/// Upper bound for a sane tax rate. Above this the value is a typo, not a
/// policy (nadie cobra 150% de impuesto al consumo).
const MAX_TAX_PERCENT: i64 = 100;

/// ISO-4217 code que se usaba como constante única hasta v0.2.0.
///
/// Sigue exportado porque es parte de la API pública del crate y hay tests que
/// lo nombran. Código nuevo: usar [`Currency`] resuelta por tenant.
pub const CURRENCY_CLP: &str = "CLP";

/// IVA Chile, 19%. Nombre histórico de [`DEFAULT_TAX_PERCENT`]; "IVA" es el
/// nombre chileno del impuesto, así que las capas genéricas usan `tax`.
pub const IVA_DEFAULT_PERCENT: u8 = DEFAULT_TAX_PERCENT;

// --- Currency --------------------------------------------------------------

/// An ISO-4217 currency code plus the one thing the arithmetic needs from it:
/// how many decimal places its minor unit has.
///
/// Construir con [`Currency::parse`] (valida) o [`Currency::from_code_or_default`]
/// (tolera basura y cae al default, para lecturas de settings viejos).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Currency {
    code: String,
}

impl Default for Currency {
    fn default() -> Self {
        Self {
            code: DEFAULT_CURRENCY_CODE.to_string(),
        }
    }
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}

impl Currency {
    /// Parse an ISO-4217 alphabetic code. Acepta minúsculas y espacios
    /// alrededor; rechaza cualquier cosa que no sean 3 letras.
    pub fn parse(code: &str) -> DomainResult<Self> {
        let trimmed = code.trim();
        if trimmed.len() != 3 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(DomainError::Invalid(format!(
                "moneda inválida '{code}': debe ser un código ISO-4217 de 3 letras (ej. CLP, USD, EUR)"
            )));
        }
        Ok(Self {
            code: trimmed.to_ascii_uppercase(),
        })
    }

    /// Same as [`Currency::parse`] but falls back to the default currency
    /// instead of failing. Para leer un `admin_setting` que puede estar vacío o
    /// mal escrito sin voltear una venta.
    pub fn from_code_or_default(code: Option<&str>) -> Self {
        match code {
            Some(c) => Self::parse(c).unwrap_or_default(),
            None => Self::default(),
        }
    }

    /// The uppercase ISO-4217 code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Decimal places of the currency's minor unit (ISO-4217 exponent).
    ///
    /// La tabla lista las excepciones; **todo lo demás es 2**, que es el caso
    /// de la enorme mayoría de las monedas (USD, EUR, MXN, ARS, PEN, BRL, COP…).
    /// Una moneda desconocida cae en 2 y no rompe: peor error sería asumir 0 y
    /// borrar centavos.
    pub fn decimals(&self) -> u32 {
        match self.code.as_str() {
            // Exponente 0 — sin unidad menor. CLP es el caso de casa.
            "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF"
            | "UGX" | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
            // Exponente 1.
            "MGA" | "MRU" => 1,
            // Exponente 3 — dinares del Golfo y del Magreb.
            "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
            _ => 2,
        }
    }

    /// Round an amount to this currency's minor unit.
    ///
    /// Estrategia: la misma de `Decimal::round_dp` (banker's rounding), que es
    /// la que el sistema venía usando para CLP. Con `decimals() == 0` esto es
    /// literalmente el `round_dp(0)` de antes, byte por byte.
    pub fn round(&self, amount: Decimal) -> Decimal {
        amount.round_dp(self.decimals())
    }

    /// `true` when the currency has no minor unit (CLP, JPY, PYG…). Los tests
    /// y los mensajes al usuario lo preguntan seguido.
    pub fn is_zero_decimal(&self) -> bool {
        self.decimals() == 0
    }
}

// --- MoneyConfig -----------------------------------------------------------

/// A tenant's money rules: which currency it charges in and at what tax rate.
///
/// Es lo que toda operación de plata debería recibir. `Default` = CLP 19%,
/// o sea el comportamiento previo a los country packs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoneyConfig {
    pub currency: Currency,
    /// Tax rate in percent (`19` = 19%). Decimal porque hay países con tasas
    /// fraccionarias (8.25%, 12.5%) y con `u8` no se pueden expresar.
    pub tax_percent: Decimal,
}

impl Default for MoneyConfig {
    fn default() -> Self {
        Self {
            currency: Currency::default(),
            tax_percent: Decimal::from(DEFAULT_TAX_PERCENT),
        }
    }
}

impl MoneyConfig {
    pub fn new(currency: Currency, tax_percent: Decimal) -> Self {
        Self {
            currency,
            tax_percent,
        }
    }

    /// Round an amount to the tenant's currency.
    pub fn round(&self, amount: Decimal) -> Decimal {
        self.currency.round(amount)
    }

    /// Decimal places of the tenant's currency.
    pub fn decimals(&self) -> u32 {
        self.currency.decimals()
    }

    /// Split a **tax-inclusive** total into `(net, tax)` using this tenant's
    /// rate and currency. Ver [`crate::invariants::tax_breakdown`].
    pub fn tax_breakdown(&self, total_inclusive: Decimal) -> (Decimal, Decimal) {
        crate::invariants::tax_breakdown(total_inclusive, self.tax_percent, self.decimals())
    }
}

/// Parse a tax rate written by an operator (`"19"`, `"21.0"`, `"8.25"`).
///
/// Rechaza negativos y cualquier cosa sobre [`MAX_TAX_PERCENT`]: una tasa
/// absurda no es una política, es un error de tipeo que se cobraría a un
/// cliente real.
pub fn parse_tax_percent(raw: &str) -> DomainResult<Decimal> {
    let trimmed = raw.trim();
    let value: Decimal = trimmed.parse().map_err(|_| {
        DomainError::Invalid(format!(
            "tasa de impuesto inválida '{raw}': se espera un porcentaje (ej. 19 o 8.25)"
        ))
    })?;
    if value.is_sign_negative() || value > Decimal::from(MAX_TAX_PERCENT) {
        return Err(DomainError::Invalid(format!(
            "tasa de impuesto fuera de rango '{raw}': debe estar entre 0 y {MAX_TAX_PERCENT}"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn default_currency_is_clp_with_zero_decimals() {
        let c = Currency::default();
        assert_eq!(c.code(), "CLP");
        assert_eq!(c.decimals(), 0);
        assert!(c.is_zero_decimal());
    }

    #[test]
    fn decimals_follow_iso4217_exponent() {
        assert_eq!(Currency::parse("CLP").unwrap().decimals(), 0);
        assert_eq!(Currency::parse("JPY").unwrap().decimals(), 0);
        assert_eq!(Currency::parse("USD").unwrap().decimals(), 2);
        assert_eq!(Currency::parse("EUR").unwrap().decimals(), 2);
        assert_eq!(Currency::parse("MXN").unwrap().decimals(), 2);
        assert_eq!(Currency::parse("KWD").unwrap().decimals(), 3);
    }

    #[test]
    fn unknown_currency_defaults_to_two_decimals_not_zero() {
        // Perder centavos es peor que arrastrarlos: una moneda que no está en
        // la tabla NUNCA debe caer en 0 decimales.
        assert_eq!(Currency::parse("ZZZ").unwrap().decimals(), 2);
    }

    #[test]
    fn parse_normalizes_case_and_whitespace() {
        assert_eq!(Currency::parse(" usd ").unwrap().code(), "USD");
    }

    #[test]
    fn parse_rejects_non_iso_codes() {
        for bad in ["", "US", "USDD", "US$", "12 3"] {
            assert!(Currency::parse(bad).is_err(), "debió rechazar {bad:?}");
        }
    }

    #[test]
    fn from_code_or_default_never_fails() {
        assert_eq!(Currency::from_code_or_default(None).code(), "CLP");
        assert_eq!(Currency::from_code_or_default(Some("")).code(), "CLP");
        assert_eq!(Currency::from_code_or_default(Some("basura")).code(), "CLP");
        assert_eq!(Currency::from_code_or_default(Some("usd")).code(), "USD");
    }

    #[test]
    fn rounding_follows_the_currency_not_the_country() {
        let clp = Currency::parse("CLP").unwrap();
        let usd = Currency::parse("USD").unwrap();
        // El caso que rompe los ERPs mal portados: 12.50 en USD son 12 dólares
        // con 50 centavos, no 12 dólares.
        assert_eq!(usd.round(dec("12.50")), dec("12.50"));
        assert_eq!(clp.round(dec("12.50")), dec("12"));
    }

    #[test]
    fn rounding_strategy_is_bankers_the_same_as_before() {
        // `round_dp` redondea al par en el empate, y así venía redondeando el
        // sistema desde que la moneda era una constante. NO cambiarlo: cambiar
        // la estrategia movería centavos en tenants chilenos que ya operan.
        let clp = Currency::parse("CLP").unwrap();
        assert_eq!(clp.round(dec("12.5")), dec("12")); // 12 es par
        assert_eq!(clp.round(dec("13.5")), dec("14")); // 14 es par
        assert_eq!(clp.round(dec("12.6")), dec("13")); // sin empate, normal

        let usd = Currency::parse("USD").unwrap();
        assert_eq!(usd.round(dec("12.505")), dec("12.50"));
        assert_eq!(usd.round(dec("12.515")), dec("12.52"));
    }

    #[test]
    fn parse_tax_percent_accepts_fractional_rates() {
        assert_eq!(parse_tax_percent("19").unwrap(), Decimal::from(19));
        assert_eq!(parse_tax_percent(" 8.25 ").unwrap(), dec("8.25"));
        assert_eq!(parse_tax_percent("0").unwrap(), Decimal::ZERO);
    }

    #[test]
    fn parse_tax_percent_rejects_nonsense() {
        for bad in ["", "diecinueve", "-1", "101"] {
            assert!(parse_tax_percent(bad).is_err(), "debió rechazar {bad:?}");
        }
    }

    #[test]
    fn default_money_config_is_the_pre_country_pack_behaviour() {
        let cfg = MoneyConfig::default();
        assert_eq!(cfg.currency.code(), CURRENCY_CLP);
        assert_eq!(cfg.tax_percent, Decimal::from(IVA_DEFAULT_PERCENT));
        assert_eq!(cfg.decimals(), 0);
    }
}
