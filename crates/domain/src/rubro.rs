//! Per-rubro declarative pack — the server-side single source of truth for the
//! multi-rubro pivot ("RutBusiness = ERP global para microempresas en Chile").
//!
//! Before this module the rubro rules lived ONLY in the client
//! (`client/src/vertical.ts`): the server stored `business.vertical` but never
//! interpreted it. Now the pack is DATA served by `GET /api/v1/rubro-pack`; the
//! client renders from it (falling back to its local copy when offline).
//!
//! A pack answers four questions per rubro:
//! 1. **features** — which optional modules the UI turns on
//! 2. **vocab** — rubro-native nouns ("Producto" vs "Servicio" vs "Plato")
//! 3. **attrs** — extra product fields (stored in `product.attrs`, migration 0033)
//! 4. **seed/kpi** — demo pack + dashboard hints
//!
//! Adding a rubro = adding one entry to [`PACKS`]. No endpoint changes.
//!
//! Discipline (docs/strategy/rubro-catalog.md §6.2): packs LIST what's on; the
//! signature features (agenda, comandas, variantes) are built per-rubro only
//! after a real client validates the rubro — `coming_soon` documents direction,
//! never a dead-end (the base ERP works for every rubro today).

use serde::Serialize;

/// Optional modules a rubro turns on. Mirrors the client's `RubroFeatures`.
///
/// Surface flags (`agent_home`, `barcode`, `printer`, `dte`, `informal_ok`) were
/// added 2026-08-08 for the feria pivot (ADR-0022): same ERP core, different
/// first-day UX. Older clients that only read the original four flags stay
/// compatible if they ignore unknown JSON keys; typed clients must map all.
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
    /// Agent chat is the primary home (feria / voice-first operators).
    pub agent_home: bool,
    /// Camera / barcode scanner in POS. Off for feria (no EAN at the puesto).
    pub barcode: bool,
    /// Thermal ESC/POS printer flows. Off for feria (no printer at the puesto).
    pub printer: bool,
    /// Electronic invoice / DTE / SII operator surfaces. Off for informal feria.
    pub dte: bool,
    /// Onboarding allows informal operation (no RUT empresa / no SII day-1).
    pub informal_ok: bool,
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
/// 2026-08-08: `feria` is first — street / feria vendors are the primary focus
/// (ADR-0022). Farmacia remains a full vertical pack, not the product north star.
static PACKS: &[RubroPack] = &[
    RubroPack {
        rubro: "feria",
        label: "Feria / Calle",
        tagline: "Tu puesto, sin cuaderno.",
        accent: "#f59e0b",
        features: RubroFeatures {
            recetas: false,
            lotes: false,
            // Soft stock: names and prices, not FEFO. On so "qué vendí" still
            // maps to sellable names; off would hide inventory entirely.
            physical_stock: true,
            clinical: false,
            agent_home: true,
            barcode: false,
            printer: false,
            dte: false,
            informal_ok: true,
        },
        vocab: RubroVocab {
            item: "Cosa",
            catalog: "Lo que vendo",
        },
        attrs: &[
            AttrField {
                key: "unidad",
                label: "Se vende por",
                kind: "text", // kg, unidad, atado, bolsa…
            },
            AttrField {
                key: "precio_ref",
                label: "Precio de referencia",
                kind: "money",
            },
        ],
        seed_vertical: None,
        coming_soon: &[
            "Venta por peso en el celular",
            "Recordatorios de quién te debe",
        ],
    },
    RubroPack {
        rubro: "farmacia",
        label: "Farmacia",
        tagline: "Tu farmacia, en regla.",
        accent: "#1fd3a3",
        features: RubroFeatures {
            recetas: true,
            lotes: true,
            physical_stock: true,
            clinical: true,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
        vocab: GENERIC_VOCAB,
        attrs: &[
            AttrField {
                key: "active_ingredient",
                label: "Principio activo",
                kind: "text",
            },
            AttrField {
                key: "laboratory",
                label: "Laboratorio",
                kind: "text",
            },
            AttrField {
                key: "therapeutic_action",
                label: "Acción terapéutica",
                kind: "text",
            },
            AttrField {
                key: "prescription_type",
                label: "Tipo de receta",
                kind: "text",
            },
        ],
        seed_vertical: Some("pharmacy"),
        coming_soon: &[],
    },
    RubroPack {
        rubro: "minimarket",
        label: "Minimarket / Almacén",
        tagline: "Tu almacén, al día.",
        accent: "#f5b53d",
        features: RubroFeatures {
            recetas: false,
            lotes: true,
            physical_stock: true,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
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
        features: RubroFeatures {
            recetas: false,
            lotes: false,
            physical_stock: true,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
        vocab: RubroVocab {
            item: "Plato",
            catalog: "Carta",
        },
        attrs: &[],
        seed_vertical: Some("restaurant"),
        coming_soon: &["Comandas y gestión de mesas"],
    },
    RubroPack {
        rubro: "cafe",
        label: "Café / Pastelería",
        tagline: "Tu café, listo cada mañana.",
        accent: "#c98a4b",
        features: RubroFeatures {
            recetas: false,
            lotes: true,
            physical_stock: true,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
        vocab: RubroVocab {
            item: "Producto",
            catalog: "Vitrina",
        },
        attrs: &[],
        seed_vertical: Some("cafe"),
        coming_soon: &["Producción y recetas de elaboración"],
    },
    RubroPack {
        rubro: "tienda",
        label: "Tienda / Retail",
        tagline: "Tu tienda, ordenada.",
        accent: "#5aa9ff",
        // Retail Chile: stock físico sí; lotes/vencimiento no (bienes no
        // perecibles). Variantes multi-SKU: hijos `product` con `parent_id`
        // (migración 0034) + attrs talla/color/sku.
        features: RubroFeatures {
            recetas: false,
            lotes: false,
            physical_stock: true,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
        vocab: RubroVocab {
            item: "Producto",
            catalog: "Inventario",
        },
        attrs: &[
            AttrField {
                key: "talla",
                label: "Talla",
                kind: "text",
            },
            AttrField {
                key: "color",
                label: "Color",
                kind: "text",
            },
            // SKU interno del local (distinto del EAN de barra del POS).
            AttrField {
                key: "sku",
                label: "SKU",
                kind: "text",
            },
        ],
        seed_vertical: Some("tienda"),
        coming_soon: &["Etiquetas de precio y código de barras"],
    },
    RubroPack {
        rubro: "belleza",
        label: "Belleza / Estética",
        tagline: "Tu salón, agendado.",
        accent: "#e879c7",
        features: RubroFeatures {
            recetas: false,
            lotes: false,
            physical_stock: false,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
        vocab: RubroVocab {
            item: "Servicio",
            catalog: "Servicios",
        },
        attrs: &[AttrField {
            key: "duracion_min",
            label: "Duración (min)",
            kind: "number",
        }],
        seed_vertical: None,
        coming_soon: &["Agenda de horas y profesionales"],
    },
    RubroPack {
        rubro: "servicios",
        label: "Servicios / Oficios",
        tagline: "Tu oficio, facturado.",
        accent: "#94a3c4",
        features: RubroFeatures {
            recetas: false,
            lotes: false,
            physical_stock: false,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
        vocab: RubroVocab {
            item: "Servicio",
            catalog: "Servicios",
        },
        attrs: &[],
        seed_vertical: Some("servicios"),
        coming_soon: &["Órdenes de trabajo y presupuestos"],
    },
    RubroPack {
        rubro: "otro",
        label: "Otro",
        tagline: "Tu negocio, a tu manera.",
        accent: "#8b97ad",
        features: RubroFeatures {
            recetas: false,
            lotes: false,
            physical_stock: true,
            clinical: false,
            agent_home: false,
            barcode: true,
            printer: true,
            dte: true,
            informal_ok: false,
        },
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

    /// Tienda/retail (P1 beachhead Chile): stock físico, sin clínica ni lotes UI.
    #[test]
    fn tienda_is_retail_not_pharmacy() {
        let p = pack_for("tienda");
        assert_eq!(p.rubro, "tienda");
        assert!(p.features.physical_stock, "retail vende bienes físicos");
        assert!(!p.features.lotes, "no perecible → sin UI de vencimiento");
        assert!(!p.features.recetas);
        assert!(!p.features.clinical, "tienda ≠ farmacia");
        assert_eq!(p.vocab.item, "Producto");
        assert_eq!(p.vocab.catalog, "Inventario");
        assert_eq!(p.seed_vertical, Some("tienda"));

        let keys: Vec<&str> = p.attrs.iter().map(|a| a.key).collect();
        assert!(keys.contains(&"talla"), "retail declara talla");
        assert!(keys.contains(&"color"), "retail declara color");
        assert!(keys.contains(&"sku"), "retail declara SKU interno");
        assert!(
            !p.coming_soon.is_empty(),
            "variantes/etiquetas van en coming_soon, no como feature mentida"
        );
    }

    /// Contraste explícito: lo que farmacia enciende, tienda apaga (y viceversa
    /// en stock físico genérico).
    #[test]
    fn tienda_diverges_from_farmacia_on_clinical_surface() {
        let farm = pack_for("farmacia");
        let shop = pack_for("tienda");
        assert!(farm.features.clinical && !shop.features.clinical);
        assert!(farm.features.recetas && !shop.features.recetas);
        assert!(farm.features.lotes && !shop.features.lotes);
        // Ambos mueven inventario físico.
        assert!(farm.features.physical_stock && shop.features.physical_stock);
        // Farmacia no declara attrs de talla/color; tienda no declara clínicos.
        let farm_keys: Vec<&str> = farm.attrs.iter().map(|a| a.key).collect();
        let shop_keys: Vec<&str> = shop.attrs.iter().map(|a| a.key).collect();
        assert!(farm_keys.contains(&"active_ingredient"));
        assert!(!shop_keys.contains(&"active_ingredient"));
        assert!(shop_keys.contains(&"talla"));
        assert!(!farm_keys.contains(&"talla"));
    }

    /// Cada pack con `seed_vertical` apunta a un vertical real del seed service
    /// (coherencia rubro-pack ↔ seed-demo). Parse sin depender del módulo seed
    /// para no acoplar unit tests de rubro a Surreal.
    #[test]
    fn declared_seed_verticals_are_known_keys() {
        // Claves que `SeedVertical::parse` acepta hoy (label canónico + sinónimos
        // del default pharmacy). Si se agrega un pack con seed, actualizar aquí.
        const KNOWN: &[&str] = &[
            "pharmacy",
            "minimarket",
            "cafe",
            "tienda",
            "servicios",
            "restaurant",
        ];
        for p in PACKS {
            if let Some(sv) = p.seed_vertical {
                assert!(
                    KNOWN.contains(&sv),
                    "rubro «{}» seed_vertical «{}» no está en el catálogo seed",
                    p.rubro,
                    sv
                );
            }
        }
        // Tienda siempre tiene pack demo (P1 beachhead histórico).
        assert_eq!(pack_for("tienda").seed_vertical, Some("tienda"));
    }

    /// Feria = beachhead 2026-08-08 (ADR-0022): agent-first, no barcode/printer/DTE.
    #[test]
    fn feria_is_agent_first_and_informal() {
        let p = pack_for("feria");
        assert_eq!(p.rubro, "feria");
        assert!(p.features.agent_home);
        assert!(p.features.informal_ok);
        assert!(!p.features.barcode);
        assert!(!p.features.printer);
        assert!(!p.features.dte);
        assert!(!p.features.recetas);
        assert!(!p.features.clinical);
        assert!(!p.features.lotes);
        assert!(p.features.physical_stock, "nombres vendibles, no FEFO");
        assert_eq!(p.vocab.item, "Cosa");
        // First in onboarding grid.
        assert_eq!(PACKS[0].rubro, "feria");
    }
}
