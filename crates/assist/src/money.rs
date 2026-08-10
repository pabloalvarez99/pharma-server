//! Cómo el agente escribe la plata.
//!
//! El agente redacta prosa, así que cada monto que menciona lleva una etiqueta
//! de moneda pegada. Hasta acá esa etiqueta era la constante `$` con formato
//! chileno: separador de miles `.`, cero decimales. Correcto para CLP y
//! **mentiroso** para cualquier otro tenant desde que la moneda pasó a ser por
//! negocio ([`domain::money`]): una farmacia en USD veía «$12» donde su sistema
//! cobró 12,50 dólares. El número que sale de la base está bien; lo que estaba
//! mal es lo que dice al lado. En una app que muestra plata, eso no es un
//! detalle de formato — el dueño la lee para decidir.
//!
//! [`Money`] es el formateador, y se construye una sola vez por pregunta desde
//! la config del tenant. No hace aritmética de negocio: redondea al exponente
//! ISO-4217 de la moneda (lo mismo que [`domain::money::Currency::round`]) y
//! escribe.

use domain::money::{Currency, Decimal, MoneyConfig, DEFAULT_CURRENCY_CODE};

/// Formatea montos en la moneda del tenant.
#[derive(Debug, Clone, Default)]
pub struct Money {
    currency: Currency,
}

impl Money {
    pub fn new(currency: Currency) -> Self {
        Self { currency }
    }

    /// El ISO-4217 del tenant, para las respuestas estructuradas (`data`) que
    /// hoy mandan el monto sin decir de qué moneda es.
    pub fn code(&self) -> &str {
        self.currency.code()
    }

    /// Escribe `v` en la moneda del tenant.
    ///
    /// * CLP: `$1.234.567` — idéntico, byte por byte, a lo que escribía el
    ///   helper fijo. Un tenant chileno no ve ningún cambio.
    /// * Cualquier otra: `USD 1.234,56`, con los decimales que la moneda tenga.
    ///   Va el código ISO y no un símbolo porque `$` ya está tomado: en un país
    ///   que escribe `$` para su peso, un monto en dólares con `$` adelante es
    ///   exactamente el bug que esto arregla. El código nunca es ambiguo.
    ///
    /// Separadores es-CL (miles `.`, decimales `,`) en las dos ramas: el agente
    /// contesta en español y el dueño lee en español.
    pub fn fmt(&self, v: Decimal) -> String {
        let decimals = self.currency.decimals();
        let rounded = self.currency.round(v);
        let neg = rounded.is_sign_negative();
        let abs = rounded.abs();

        let integer = group_thousands(&abs.trunc().to_string());
        let body = if decimals == 0 {
            integer
        } else {
            // La parte decimal se escribe como entero con ceros a la izquierda:
            // 12,5 con 2 decimales tiene que salir «12,50» y 0,05 no puede
            // salir «0,5». Va por `u64` y no formateando el `Decimal` directo
            // porque el `Display` de `Decimal` no honra el relleno con ceros.
            let scaled = ((abs - abs.trunc()) * Decimal::from(10u64.pow(decimals))).round();
            let frac = u64::try_from(scaled).unwrap_or(0);
            format!("{integer},{frac:0width$}", width = decimals as usize)
        };

        let sign = if neg { "-" } else { "" };
        if self.currency.code() == DEFAULT_CURRENCY_CODE {
            format!("{sign}${body}")
        } else {
            format!("{sign}{} {body}", self.currency.code())
        }
    }
}

impl From<&MoneyConfig> for Money {
    fn from(cfg: &MoneyConfig) -> Self {
        Self::new(cfg.currency.clone())
    }
}

impl From<Currency> for Money {
    fn from(currency: Currency) -> Self {
        Self::new(currency)
    }
}

/// `1234567` -> `1.234.567`.
fn group_thousands(digits: &str) -> String {
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(*ch as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn money(code: &str) -> Money {
        Money::new(Currency::parse(code).unwrap())
    }

    #[test]
    fn clp_is_byte_for_byte_what_it_always_was() {
        let m = money("CLP");
        assert_eq!(m.fmt(Decimal::from(0)), "$0");
        assert_eq!(m.fmt(Decimal::from(999)), "$999");
        assert_eq!(m.fmt(Decimal::from(1234)), "$1.234");
        assert_eq!(m.fmt(Decimal::from(1234567)), "$1.234.567");
        assert_eq!(m.fmt(Decimal::from(-5000)), "-$5.000");
    }

    #[test]
    fn a_usd_tenant_reads_usd_with_its_cents() {
        // El bug: el número salía bien y la etiqueta mentía. 12,50 dólares se
        // mostraban como «$12» — moneda equivocada Y centavos perdidos.
        let m = money("USD");
        assert_eq!(m.fmt(dec("12.50")), "USD 12,50");
        assert_eq!(m.fmt(dec("1234567.89")), "USD 1.234.567,89");
        assert_eq!(m.fmt(dec("0.05")), "USD 0,05");
        assert_eq!(m.fmt(dec("-5000")), "-USD 5.000,00");
    }

    #[test]
    fn decimals_follow_the_currency() {
        assert_eq!(money("JPY").fmt(dec("1234.6")), "JPY 1.235");
        assert_eq!(money("EUR").fmt(dec("1234.5")), "EUR 1.234,50");
        assert_eq!(money("KWD").fmt(dec("1.2345")), "KWD 1,234");
    }

    #[test]
    fn trailing_zeros_are_written_out() {
        assert_eq!(money("USD").fmt(dec("12.5")), "USD 12,50");
        assert_eq!(money("USD").fmt(dec("12")), "USD 12,00");
    }

    #[test]
    fn default_is_clp_so_a_tenant_that_never_configured_anything_is_unchanged() {
        assert_eq!(Money::default().fmt(Decimal::from(1990)), "$1.990");
    }
}
