//! Per-rubro declarative pack — the server-side single source of truth for the
//! multi-rubro pivot ("RutBusiness = ERP global para microempresas en Chile").
//!
//! Before this module the rubro rules lived ONLY in the client
//! (`client/src/vertical.ts`): the server stored `business.vertical` but never
//! interpreted it. Now the pack is DATA served by `GET /api/v1/rubro-pack`; the
//! client renders from it (falling back to its local copy when offline).
//!
//! A pack answers four questions per rubro:
//!   1. **features**   — which optional modules the UI turns on
//!   2. **vocab**      — rubro-native nouns ("Producto" vs "Servicio" vs "Plato")
//!   3. **attrs**      — which extra product fields the rubro declares
//!                       (stored in `product.attrs`, migration 0033)
//!   4. **seed/kpi**   — demo pack + dashboard hints
//!
//! Adding a rubro = adding one entry to [`PACKS`]. No endpoint changes.
//!
//! Discipline (docs/strategy/rubro-catalog.md §6.2): packs LIST what's on; the
//! signature features (agenda, comandas, variantes) are built per-rubro only
//! after a real client validates the rubro — `coming_soon` documents direction,
//! never a dead-end (the base ERP works for every rubro today).

use serde::Serialize;

/// Optional modules a rubro turns on. Mirrors the client's `RubroFeatures`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RubroFeatures {
    /// Recetas + libro de controlados (Ley 20.000). Farmacia only.
    pub recetas: bool,
    /// Lotes + vencimiento (perecibles).
    pub lotes: bool,
    /// Tracks physical stock at all (inventario + compras). Service rubros
    /// (belleza, servicios) sell without inventory.
    pub physical_stock: bool,
    /// Clinical product fields (principio activo, laboratorio). Farmacia only.
    pub clinical: bool,
}

/// One extra product attribute the rubro declares (stored under
/// `product.attrs[key]`). The UI renders the product form/detail from this
/// schema instead of hardcoding fields per rubro.
#[derive(Debug, Clone, Serialize)]
pub struct AttrField {
    /// Key inside `product.attrs` (snake_case, stable on the wire).
    pub key: &'static str,
    /// Spanish label for the form.
    pub label: &'static str,
    /// `text` | `number` | `money` | `date` | `bool`.
    pub kind: &'static str,
}

/// Rubro-native vocabulary. Keys are stable; the UI defaults to the generic
/// value when a rubro doesn't override one.
#[derive(Debug, Clone, Serialize)]
pub struct RubroVocab {
    /// What one sellable thing is called ("Producto", "Servicio", "Plato").
    pub item: &'static str,
    /// The catalog section ("Inventario", "Carta", "Servicios").
    pub catalog: &'static str,
}

/// The declarative pack served to the client.
#[derive(Debug, Clone, Serialize)]
pub struct RubroPack {
    /// Canonical rubro key (matches the stored `business.vertical` value).
    pub rubro: &'static str,
    pub label: &'static str,
    pub tagline: &'static str,
    /// Per-rubro accent (6-digit hex) theming cards/preview.
    pub accent: &'static str,
    pub features: RubroFeatures,
    pub vocab: RubroVocab,
    /// Extra product fields this rubro declares (may be empty).
    pub attrs: &'static [AttrField],
    /// Seed pack (en, for the seed-demo endpoint), or `None` when unbuilt.
    pub seed_vertical: Option<&'static str>,
    /// Roadmap bullets shown with grace as "Próximamente" (NOT built).
    pub coming_soon: &'static [&'static str],
}

/// Default rubro when unset/unknown — generic ERP, never farmacia (pivot rule).
pub const DEFAULT_RUBRO: &str = "otro";

const GENERIC_VOCAB: RubroVocab = RubroVocab {
    item: "Producto",
    catalog: "Inventario",
};

/// The catalog. Order = onboarding grid order (beachhead first).
static PACKS: &[RubroPack] = &[
    RubroPack {
        rubro: "farmacia",
        label: "Farmacia",
        tagline: "Tu farmacia, en regla.",
        accent: "#1fd3a3",
        features: RubroFeatures { recetas: true, lotes: true, physical_stock: true, clinical: true },
        vocab: GENERIC_VOCAB,
        attrs: &[
            AttrField { key: "active_ingredient", label: "Principio activo", kind: "text" },
            AttrField { key: "laboratory", label: "Laboratorio", kind: "text" },
            AttrField { key: "therapeutic_action", label: "Acción terapéutica", kind: "text" },
            AttrField { key: "prescription_type", label: "Tipo de receta", kind: "text" },
        ],
        seed_vertical: Some("pharmacy"),
        coming_soon: &[],
    },
    RubroPack {
        rubro: "minimarket",
        label: "Minimarket / Almacén",
        tagline: "Tu almacén, al día.",
        accent: "#f5b53d",
        features: RubroFeatures { recetas: false, lotes: true, physical_stock: true, clinical: false },
        vocab: GENERIC_VOCAB,
        attrs: &[],
        seed_vertical: Some("minimarket"),
        coming_soon: &[],
    },
    RubroPack {
        rubro: "restaurant",
        label: "Restaurant / Comida",
        tagline: "Tu cocina, bajo control.",
        accent: "#ff6b5d",
        features: RubroFeatures { recetas: false, lotes: false, physical_stock: true, clinical: false },
        vocab: RubroVocab { item: "Plato", catalog: "Carta" },
        attrs: &[],
        seed_vertical: Some("restaurant"),
        coming_soon: &["Comandas y gestión de mesas"],
    },
    RubroPack {
        rubro: "cafe",
        label: "Café / Pastelería",
        tagline: "Tu café, listo cada mañana.",
        accent: "#c98a4b",
        features: RubroFeatures { recetas: false, lotes: true, physical_stock: true, clinical: false },
        vocab: RubroVocab { item: "Producto", catalog: "Vitrina" },
        attrs: &[],
        seed_vertical: Some("cafe"),
        coming_soon: &["Producción y recetas de elaboración"],
    },
    RubroPack {
        rubro: "tienda",
        label: "Tienda / Retail",
        tagline: "Tu tienda, ordenada.",
        accent: "#5aa9ff",
        features: RubroFeatures { recetas: false, lotes: false, physical_stock: true, clinical: false },
        vocab: GENERIC_VOCAB,
        attrs: &[
            AttrField { key: "talla", label: "Talla", kind: "text" },
            AttrField { key: "color", label: "Color", kind: "text" },
        ],
        seed_vertical: Some("tienda"),
        coming_soon: &["Variantes y tallas (un producto, varios SKU)"],
    },
    RubroPack {
        rubro: "belleza",
        label: "Belleza / Estética",
        tagline: "Tu salón, agendado.",
        accent: "#e879c7",
        features: RubroFeatures { recetas: false, lotes: false, physical_stock: false, clinical: false },
        vocab: RubroVocab { item: "Servicio", catalog: "Servicios" },
        attrs: &[
            AttrField { key: "duracion_min", label: "Duración (min)", kind: "number" },
        ],
        seed_vertical: None,
        coming_soon: &["Agenda de horas y profesionales"],
    },
    RubroPack {
        rubro: "servicios",
        label: "Servicios / Oficios",
        tagline: "Tu oficio, facturado.",
        accent: "#94a3c4",
        features: RubroFeatures { recetas: false, lotes: false, physical_stock: false, clinical: false },
        vocab: RubroVocab { item: "Servicio", catalog: "Servicios" },
        attrs: &[],
        seed_vertical: Some("servicios"),
        coming_soon: &["Órdenes de trabajo y presupuestos"],
    },
    RubroPack {
        rubro: "otro",
        label: "Otro",
        tagline: "Tu negocio, a tu manera.",
        accent: "#8b97ad",
        features: RubroFeatures { recetas: false, lotes: false, physical_stock: true, clinical: false },
        vocab: GENERIC_VOCAB,
        attrs: &[],
        seed_vertical: None,
        coming_soon: &[],
    },
];

/// The pack for a stored rubro value (unknown/empty → generic `otro`).
pub fn pack_for(rubro: &str) -> &'static RubroPack {
    let key = rubro.trim().to_lowercase();
    PACKS
        .iter()
        .find(|p| p.rubro == key)
        .or_else(|| PACKS.iter().find(|p| p.rubro == DEFAULT_RUBRO))
        .expect("el pack genérico 'otro' siempre existe")
}

/// Every catalog key (onboarding grid).
pub fn all_rubros() -> Vec<&'static str> {
    PACKS.iter().map(|p| p.rubro).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pack_resolves() {
        for p in PACKS {
            assert_eq!(pack_for(p.rubro).rubro, p.rubro);
        }
    }

    #[test]
    fn unknown_falls_back_to_otro() {
        assert_eq!(pack_for("").rubro, DEFAULT_RUBRO);
        assert_eq!(pack_for("  FERRETERIA ").rubro, DEFAULT_RUBRO);
    }

    #[test]
    fn service_rubros_have_no_physical_stock() {
        assert!(!pack_for("belleza").features.physical_stock);
        assert!(!pack_for("servicios").features.physical_stock);
        assert!(pack_for("farmacia").features.physical_stock);
    }

    #[test]
    fn only_farmacia_is_clinical() {
        for p in PACKS {
            assert_eq!(p.features.clinical, p.rubro == "farmacia", "{}", p.rubro);
            assert_eq!(p.features.recetas, p.rubro == "farmacia", "{}", p.rubro);
        }
    }
}
