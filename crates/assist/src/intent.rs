//! Deterministic es-CL intent parser.
//!
//! Maps a natural-language question typed by the business owner into one of a
//! closed set of [`Intent`]s using keyword/pattern matching. NO machine
//! learning, NO network — 100% local, offline-first (ADR-0005, ADR-0016). The
//! match is intentionally conservative: anything it can't confidently classify
//! becomes [`Intent::Unknown`] so the agent answers with a friendly nudge
//! instead of guessing.

/// A recognised question the owner can ask their business agent. The variants
/// map 1:1 to a read-only executor in the deterministic provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Today's sales (orders + revenue).
    VentasHoy,
    /// Month-to-date sales.
    VentasMes,
    /// Batches expiring soon / already expired.
    PorVencer,
    /// Stock of a specific product (the captured search term).
    StockProducto(String),
    /// Cash currently expected in the open drawer.
    CajaActual,
    /// Best-selling products (month-to-date ranking).
    TopProductos,
    /// Gross margin month-to-date.
    MargenMes,
    /// Products at/below their low-stock threshold (reorder hints).
    StockBajo,
    /// Inventory headline figures (SKUs, value, low/out of stock).
    ResumenInventario,
    /// The owner asked what they can ask.
    Ayuda,
    /// Could not be classified.
    Unknown,
}

impl Intent {
    /// Stable machine label echoed back in the API response `intent` field.
    pub fn label(&self) -> &'static str {
        match self {
            Intent::VentasHoy => "ventas_hoy",
            Intent::VentasMes => "ventas_mes",
            Intent::PorVencer => "por_vencer",
            Intent::StockProducto(_) => "stock_producto",
            Intent::CajaActual => "caja_actual",
            Intent::TopProductos => "top_productos",
            Intent::MargenMes => "margen_mes",
            Intent::StockBajo => "stock_bajo",
            Intent::ResumenInventario => "resumen_inventario",
            Intent::Ayuda => "ayuda",
            Intent::Unknown => "desconocido",
        }
    }
}

/// Lowercase + strip Spanish accents/diacritics so "qué", "que" and "QUE" all
/// match the same keyword table. Keeps `ñ` folded to `n` (Chilean users rarely
/// type it for keywords like "mañana").
fn normalize(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// True when every needle in `all` appears in `hay`.
fn contains_all(hay: &str, all: &[&str]) -> bool {
    all.iter().all(|n| hay.contains(n))
}

/// True when any needle appears in `hay`.
fn contains_any(hay: &str, any: &[&str]) -> bool {
    any.iter().any(|n| hay.contains(n))
}

/// Parse a raw question into an [`Intent`]. Order matters: more specific
/// patterns are tested before broad ones so e.g. "stock de paracetamol" is a
/// product lookup, not the generic inventory summary.
pub fn parse(question: &str) -> Intent {
    let q = normalize(question);
    if q.is_empty() {
        return Intent::Unknown;
    }

    // Help / capabilities discovery.
    if contains_any(
        &q,
        &[
            "ayuda",
            "que puedo preguntar",
            "que puedes hacer",
            "que puedo hacer",
            "opciones",
            "help",
        ],
    ) {
        return Intent::Ayuda;
    }

    // Cash drawer — keep before generic "sales" since both mention plata.
    // "plata"/"efectivo" are strong cash cues for a CL business owner.
    if contains_any(&q, &["caja", "drawer", "efectivo", "plata"]) {
        return Intent::CajaActual;
    }

    // Margin — before "ventas mes" because both can mention "mes".
    if contains_any(&q, &["margen", "ganancia", "utilidad", "rentabilidad"]) {
        return Intent::MargenMes;
    }

    // Stock of a specific product — capture the search term after the cue.
    if let Some(term) = capture_product(&q) {
        return Intent::StockProducto(term);
    }

    // Reorder / low stock — before the generic inventory summary.
    if contains_any(
        &q,
        &[
            "stock bajo",
            "bajo stock",
            "reponer",
            "reposicion",
            "reorden",
            "falta",
            "faltante",
            "agotado",
            "agotando",
            "quiebre",
        ],
    ) {
        return Intent::StockBajo;
    }

    // Expiry.
    if contains_any(
        &q,
        &[
            "vence",
            "vencer",
            "vencido",
            "vencimiento",
            "caduca",
            "caducidad",
            "por vencer",
        ],
    ) {
        return Intent::PorVencer;
    }

    // Top products / best sellers.
    if contains_any(&q, &["top", "mas vendido", "mas vendidos", "best seller"])
        || contains_all(&q, &["productos", "vendidos"])
    {
        return Intent::TopProductos;
    }

    // Generic inventory headline. MUST precede the sales branch: "inventario"
    // contains the substring "venta", which would otherwise be misread as a
    // sales question.
    if contains_any(
        &q,
        &[
            "inventario",
            "cuantos productos",
            "cuantos sku",
            "resumen inventario",
            "valor inventario",
        ],
    ) {
        return Intent::ResumenInventario;
    }

    // Sales — disambiguate today vs month.
    if contains_any(
        &q,
        &["venta", "vendi", "vendido", "vendimos", "facturacion"],
    ) {
        if contains_any(&q, &["mes", "mensual", "este mes"]) {
            return Intent::VentasMes;
        }
        // Default sales window = today (the most common owner question).
        return Intent::VentasHoy;
    }

    Intent::Unknown
}

/// Cues that introduce a product name for a stock lookup. Returns the trailing
/// term (trimmed of filler) if a cue is present and a non-empty name follows.
fn capture_product(q: &str) -> Option<String> {
    const CUES: &[&str] = &[
        "stock de ",
        "stock del ",
        "cuanto stock de ",
        "cuanto stock hay de ",
        "cuanto queda de ",
        "cuanto hay de ",
        "queda de ",
        "hay de ",
        "tengo de ",
        "inventario de ",
    ];
    for cue in CUES {
        if let Some(pos) = q.find(cue) {
            let tail = q[pos + cue.len()..].trim();
            let term = strip_trailing_punct(tail);
            if !term.is_empty() {
                return Some(term.to_string());
            }
        }
    }
    None
}

fn strip_trailing_punct(s: &str) -> &str {
    s.trim_matches(|c: char| c == '?' || c == '!' || c == '.' || c == ',' || c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ventas_hoy_synonyms() {
        for q in [
            "ventas hoy",
            "cuánto vendí hoy?",
            "Cuanto se vendió hoy",
            "ventas del día",
            "facturación de hoy",
        ] {
            assert_eq!(parse(q), Intent::VentasHoy, "q={q}");
        }
    }

    #[test]
    fn ventas_mes_beats_hoy_when_month_present() {
        for q in [
            "ventas del mes",
            "cuánto vendí este mes",
            "ventas mensuales",
        ] {
            assert_eq!(parse(q), Intent::VentasMes, "q={q}");
        }
    }

    #[test]
    fn por_vencer_synonyms() {
        for q in [
            "qué se vence",
            "productos por vencer",
            "vencimientos próximos",
            "qué caduca pronto",
        ] {
            assert_eq!(parse(q), Intent::PorVencer, "q={q}");
        }
    }

    #[test]
    fn stock_producto_captures_term() {
        assert_eq!(
            parse("stock de paracetamol"),
            Intent::StockProducto("paracetamol".into())
        );
        assert_eq!(
            parse("cuánto queda de Ibuprofeno 400?"),
            Intent::StockProducto("ibuprofeno 400".into())
        );
        assert_eq!(
            parse("cuánto hay de coca cola"),
            Intent::StockProducto("coca cola".into())
        );
    }

    #[test]
    fn caja_synonyms() {
        for q in [
            "cuánto hay en caja",
            "efectivo en caja",
            "cuánta plata tengo",
        ] {
            assert_eq!(parse(q), Intent::CajaActual, "q={q}");
        }
    }

    #[test]
    fn top_productos_synonyms() {
        for q in [
            "top productos",
            "los más vendidos",
            "productos más vendidos",
        ] {
            assert_eq!(parse(q), Intent::TopProductos, "q={q}");
        }
    }

    #[test]
    fn margen_synonyms() {
        for q in ["margen del mes", "cuál es mi ganancia", "rentabilidad"] {
            assert_eq!(parse(q), Intent::MargenMes, "q={q}");
        }
    }

    #[test]
    fn stock_bajo_synonyms() {
        for q in [
            "qué tengo que reponer",
            "productos con stock bajo",
            "faltantes",
        ] {
            assert_eq!(parse(q), Intent::StockBajo, "q={q}");
        }
    }

    #[test]
    fn resumen_inventario() {
        for q in ["resumen del inventario", "cuántos productos tengo"] {
            assert_eq!(parse(q), Intent::ResumenInventario, "q={q}");
        }
    }

    #[test]
    fn ayuda() {
        for q in ["ayuda", "qué puedo preguntar?", "help"] {
            assert_eq!(parse(q), Intent::Ayuda, "q={q}");
        }
    }

    #[test]
    fn unknown_is_graceful() {
        for q in ["", "   ", "cuéntame un chiste", "asdf qwer"] {
            assert_eq!(parse(q), Intent::Unknown, "q={q}");
        }
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(Intent::VentasHoy.label(), "ventas_hoy");
        assert_eq!(Intent::StockProducto("x".into()).label(), "stock_producto");
        assert_eq!(Intent::Unknown.label(), "desconocido");
    }
}
