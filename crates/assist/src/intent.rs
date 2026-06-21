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
    /// Yesterday's sales.
    VentasAyer,
    /// Month-to-date sales.
    VentasMes,
    /// Last calendar month's sales (full month).
    VentasMesPasado,
    /// Today vs yesterday comparison.
    ComparativaDia,
    /// Month-to-date vs last full month comparison.
    ComparativaMes,
    /// Sales broken down by payment method (cash vs card), month-to-date.
    VentasPorMetodo,
    /// Batches expiring soon / already expired (next 30 days).
    PorVencer,
    /// Batches expiring within the next 7 days.
    PorVencerSemana,
    /// Stock of a specific product (the captured search term).
    StockProducto(String),
    /// Cash currently expected in the open drawer.
    CajaActual,
    /// Best-selling products (month-to-date ranking).
    TopProductos,
    /// Top customers by loyalty points.
    ClientesTop,
    /// Look up a specific customer by name/RUT/phone (the captured search term).
    BuscarCliente(String),
    /// How many customers the business has (active count + loyalty points).
    ResumenClientes,
    /// Sale price of a specific product (the captured search term).
    PrecioProducto(String),
    /// Gross margin month-to-date.
    MargenMes,
    /// Unit margin of a specific product (the captured search term).
    MargenProducto(String),
    /// Month-to-date expenses (total + breakdown by category).
    GastosMes,
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
            Intent::VentasAyer => "ventas_ayer",
            Intent::VentasMes => "ventas_mes",
            Intent::VentasMesPasado => "ventas_mes_pasado",
            Intent::ComparativaDia => "ventas_vs_ayer",
            Intent::ComparativaMes => "ventas_vs_mes_pasado",
            Intent::VentasPorMetodo => "ventas_metodo_pago",
            Intent::PorVencer => "por_vencer",
            Intent::PorVencerSemana => "por_vencer_semana",
            Intent::StockProducto(_) => "stock_producto",
            Intent::CajaActual => "caja_actual",
            Intent::TopProductos => "top_productos",
            Intent::ClientesTop => "clientes_top",
            Intent::BuscarCliente(_) => "buscar_cliente",
            Intent::ResumenClientes => "clientes_resumen",
            Intent::PrecioProducto(_) => "precio_producto",
            Intent::MargenMes => "margen_mes",
            Intent::MargenProducto(_) => "margen_producto",
            Intent::GastosMes => "gastos_mes",
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
pub(crate) fn normalize(s: &str) -> String {
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

    // Payment-method breakdown. MUST precede both the comparative branch
    // ("efectivo vs tarjeta" carries the "vs" cue) and the cash-drawer branch
    // ("efectivo"/"tarjeta" alone are cash cues). A method question is one that
    // names the *concept* of payment method, or contrasts cash AND card.
    if contains_any(
        &q,
        &[
            "metodo de pago",
            "metodos de pago",
            "medio de pago",
            "medios de pago",
            "forma de pago",
            "formas de pago",
            "como pagan",
            "como me pagan",
            "como paga la gente",
            "por metodo",
        ],
    ) || (contains_any(&q, &["efectivo", "cash"])
        && contains_any(&q, &["tarjeta", "debito", "credito"]))
    {
        return Intent::VentasPorMetodo;
    }

    // Comparatives ("ventas vs ayer", "compara con el mes pasado"). Before the
    // cash and plain-sales branches so the comparison wins over either side.
    if contains_any(&q, &[" vs ", " vs.", "versus", "compar", "contra "]) {
        if mentions_month(&q) {
            return Intent::ComparativaMes;
        }
        return Intent::ComparativaDia;
    }

    // Expenses — "gastos del mes", "cuánto gasté". Distinct vocabulary from
    // sales; safe to test early.
    if contains_any(
        &q,
        &[
            "gasto",
            "gaste",
            "gastamos",
            "egreso",
            "cuanto sali",
            "en que se fue",
        ],
    ) {
        return Intent::GastosMes;
    }

    // Cash drawer — keep before generic "sales" since both mention plata.
    // "plata"/"efectivo" are strong cash cues for a CL business owner.
    if contains_any(&q, &["caja", "drawer", "efectivo", "plata"]) {
        return Intent::CajaActual;
    }

    // Top customers — before TopProductos because "top clientes" carries "top".
    if contains_any(
        &q,
        &[
            "mejores clientes",
            "mejor cliente",
            "clientes top",
            "top clientes",
            "clientes frecuentes",
            "clientes fieles",
            "quien compra",
            "quien me compra",
            "quienes compran",
        ],
    ) {
        return Intent::ClientesTop;
    }

    // How many customers — before the per-customer lookup so "cuántos clientes
    // tengo" isn't read as a name search.
    if contains_any(
        &q,
        &[
            "cuantos clientes",
            "cuantos cliente",
            "cuantas personas",
            "cantidad de clientes",
            "total de clientes",
            "numero de clientes",
        ],
    ) {
        return Intent::ResumenClientes;
    }

    // Look up a specific customer by name/RUT/phone.
    if let Some(term) = capture_customer(&q) {
        return Intent::BuscarCliente(term);
    }

    // Margin — before "ventas mes" because both can mention "mes". A margin
    // question is per-product when a product name follows the cue; otherwise it
    // is the month-to-date margin.
    if contains_any(&q, &["margen", "ganancia", "utilidad", "rentabilidad"]) {
        if let Some(term) = capture_margin_product(&q) {
            return Intent::MargenProducto(term);
        }
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

    // Expiry — week window before the generic 30-day one.
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
        if mentions_week(&q) {
            return Intent::PorVencerSemana;
        }
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

    // Price of a specific product — captured term after a price cue. After the
    // inventory summary ("valor inventario") so that doesn't get read as a
    // product-price lookup.
    if let Some(term) = capture_price_product(&q) {
        return Intent::PrecioProducto(term);
    }

    // Sales — disambiguate by relative-date window.
    if contains_any(
        &q,
        &[
            "venta",
            "vendi",
            "vendido",
            "vendimos",
            "facturacion",
            "facture",
        ],
    ) {
        if mentions_last_month(&q) {
            return Intent::VentasMesPasado;
        }
        if mentions_month(&q) {
            return Intent::VentasMes;
        }
        if mentions_yesterday(&q) {
            return Intent::VentasAyer;
        }
        // Default sales window = today (the most common owner question).
        return Intent::VentasHoy;
    }

    Intent::Unknown
}

/// True when the question refers to the current month.
fn mentions_month(q: &str) -> bool {
    contains_any(q, &["mes", "mensual"])
}

/// True when the question refers to the *previous* month.
fn mentions_last_month(q: &str) -> bool {
    contains_any(q, &["mes pasado", "mes anterior", "el mes pasado"])
}

/// True when the question refers to yesterday.
fn mentions_yesterday(q: &str) -> bool {
    contains_any(q, &["ayer", "el dia de ayer"])
}

/// True when the question scopes to "this week" / a 7-day window.
fn mentions_week(q: &str) -> bool {
    contains_any(q, &["semana", "7 dias", "siete dias"])
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

/// Cues that introduce a product name in a margin question ("margen de X").
/// Returns the trailing product term, or `None` when no product follows (e.g.
/// "margen del mes" — the tail is a time word, not a product → month margin).
fn capture_margin_product(q: &str) -> Option<String> {
    const CUES: &[&str] = &[
        "margen de ",
        "margen del ",
        "margen en ",
        "ganancia de ",
        "ganancia del ",
        "rentabilidad de ",
        "utilidad de ",
    ];
    // Time words that mean "the margin over a period", NOT a product name.
    const PERIODS: &[&str] = &[
        "mes",
        "mes pasado",
        "mes anterior",
        "dia",
        "hoy",
        "ayer",
        "semana",
        "ano",
        "anio",
    ];
    for cue in CUES {
        if let Some(pos) = q.find(cue) {
            let tail = q[pos + cue.len()..].trim();
            let term = strip_trailing_punct(tail);
            if !term.is_empty() && !PERIODS.contains(&term) {
                return Some(term.to_string());
            }
        }
    }
    None
}

/// Cues that introduce a customer name/RUT for a lookup ("busca el cliente X").
fn capture_customer(q: &str) -> Option<String> {
    const CUES: &[&str] = &[
        "busca el cliente ",
        "busca al cliente ",
        "busca cliente ",
        "buscar cliente ",
        "buscame al cliente ",
        "buscame el cliente ",
        "encuentra al cliente ",
        "encuentra el cliente ",
        "datos del cliente ",
        "datos de cliente ",
        "ficha del cliente ",
        "info del cliente ",
        "informacion del cliente ",
        "cliente llamado ",
        "el cliente ",
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

/// Cues that introduce a product name in a price question ("precio de X",
/// "cuánto cuesta X"). Returns the trailing product term.
fn capture_price_product(q: &str) -> Option<String> {
    const CUES: &[&str] = &[
        "precio de ",
        "precio del ",
        "cuanto cuesta ",
        "cuanto vale ",
        "cuanto sale ",
        "a cuanto esta ",
        "a como esta ",
        "que precio tiene ",
    ];
    for cue in CUES {
        if let Some(pos) = q.find(cue) {
            let mut tail = q[pos + cue.len()..].trim();
            // Drop a leading article so "precio del ibuprofeno" → "ibuprofeno".
            for art in ["el ", "la ", "los ", "las ", "un ", "una ", "producto "] {
                if let Some(s) = tail.strip_prefix(art) {
                    tail = s.trim();
                }
            }
            let term = strip_trailing_punct(tail);
            if !term.is_empty() {
                return Some(term.to_string());
            }
        }
    }
    None
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
    fn ventas_ayer_synonyms() {
        for q in ["ventas de ayer", "cuánto vendí ayer", "facturación de ayer"] {
            assert_eq!(parse(q), Intent::VentasAyer, "q={q}");
        }
    }

    #[test]
    fn ventas_mes_pasado_synonyms() {
        for q in [
            "ventas del mes pasado",
            "cuánto vendí el mes anterior",
            "ventas mes pasado",
        ] {
            assert_eq!(parse(q), Intent::VentasMesPasado, "q={q}");
        }
    }

    #[test]
    fn comparativa_dia_vs_ayer() {
        for q in [
            "ventas de hoy vs ayer",
            "compara las ventas con ayer",
            "ventas hoy versus ayer",
            "hoy contra ayer",
        ] {
            assert_eq!(parse(q), Intent::ComparativaDia, "q={q}");
        }
    }

    #[test]
    fn comparativa_mes_vs_mes_pasado() {
        for q in [
            "ventas de este mes vs el mes pasado",
            "compara este mes con el mes pasado",
            "este mes versus el mes anterior",
        ] {
            assert_eq!(parse(q), Intent::ComparativaMes, "q={q}");
        }
    }

    #[test]
    fn ventas_por_metodo_synonyms() {
        for q in [
            "ventas por método de pago",
            "cómo pagan mis clientes",
            "efectivo vs tarjeta",
            "medios de pago",
        ] {
            assert_eq!(parse(q), Intent::VentasPorMetodo, "q={q}");
        }
        // "efectivo en caja" is a cash question, not a method breakdown.
        assert_eq!(parse("efectivo en caja"), Intent::CajaActual);
    }

    #[test]
    fn gastos_mes_synonyms() {
        for q in ["gastos del mes", "cuánto gasté", "egresos de este mes"] {
            assert_eq!(parse(q), Intent::GastosMes, "q={q}");
        }
    }

    #[test]
    fn clientes_top_synonyms() {
        for q in [
            "mejores clientes",
            "clientes top",
            "quién me compra más",
            "clientes frecuentes",
        ] {
            assert_eq!(parse(q), Intent::ClientesTop, "q={q}");
        }
        // Must not be stolen by the products ranking.
        assert_eq!(parse("top productos"), Intent::TopProductos);
    }

    #[test]
    fn margen_producto_vs_mes() {
        assert_eq!(
            parse("margen de paracetamol"),
            Intent::MargenProducto("paracetamol".into())
        );
        assert_eq!(
            parse("cuál es la ganancia de coca cola"),
            Intent::MargenProducto("coca cola".into())
        );
        // Time word after the cue → month margin, not a product.
        assert_eq!(parse("margen del mes"), Intent::MargenMes);
        assert_eq!(parse("rentabilidad"), Intent::MargenMes);
    }

    #[test]
    fn por_vencer_semana_vs_generico() {
        assert_eq!(parse("qué se vence esta semana"), Intent::PorVencerSemana);
        assert_eq!(parse("qué vence en 7 días"), Intent::PorVencerSemana);
        assert_eq!(parse("qué se vence"), Intent::PorVencer);
    }

    #[test]
    fn buscar_cliente_captures_term() {
        assert_eq!(
            parse("busca el cliente Juan Pérez"),
            Intent::BuscarCliente("juan perez".into())
        );
        assert_eq!(
            parse("datos del cliente maria"),
            Intent::BuscarCliente("maria".into())
        );
    }

    #[test]
    fn resumen_clientes_synonyms() {
        for q in [
            "cuántos clientes tengo",
            "cantidad de clientes",
            "total de clientes",
        ] {
            assert_eq!(parse(q), Intent::ResumenClientes, "q={q}");
        }
        // Must not be stolen by the per-customer lookup or the top ranking.
        assert_eq!(parse("mejores clientes"), Intent::ClientesTop);
    }

    #[test]
    fn precio_producto_captures_term() {
        assert_eq!(
            parse("precio de paracetamol"),
            Intent::PrecioProducto("paracetamol".into())
        );
        assert_eq!(
            parse("cuánto cuesta el ibuprofeno"),
            Intent::PrecioProducto("ibuprofeno".into())
        );
        assert_eq!(
            parse("a cuánto está la coca cola"),
            Intent::PrecioProducto("coca cola".into())
        );
        // "valor inventario" stays the inventory summary, not a price lookup.
        assert_eq!(parse("valor inventario"), Intent::ResumenInventario);
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(Intent::VentasHoy.label(), "ventas_hoy");
        assert_eq!(Intent::VentasAyer.label(), "ventas_ayer");
        assert_eq!(Intent::VentasMesPasado.label(), "ventas_mes_pasado");
        assert_eq!(Intent::ComparativaDia.label(), "ventas_vs_ayer");
        assert_eq!(Intent::ComparativaMes.label(), "ventas_vs_mes_pasado");
        assert_eq!(Intent::VentasPorMetodo.label(), "ventas_metodo_pago");
        assert_eq!(Intent::PorVencerSemana.label(), "por_vencer_semana");
        assert_eq!(Intent::ClientesTop.label(), "clientes_top");
        assert_eq!(Intent::BuscarCliente("x".into()).label(), "buscar_cliente");
        assert_eq!(Intent::ResumenClientes.label(), "clientes_resumen");
        assert_eq!(
            Intent::PrecioProducto("x".into()).label(),
            "precio_producto"
        );
        assert_eq!(Intent::GastosMes.label(), "gastos_mes");
        assert_eq!(
            Intent::MargenProducto("x".into()).label(),
            "margen_producto"
        );
        assert_eq!(Intent::StockProducto("x".into()).label(), "stock_producto");
        assert_eq!(Intent::Unknown.label(), "desconocido");
    }
}
