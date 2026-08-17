use license::schema::{License, Tier};
use uuid::Uuid;

#[test]
fn free_default_shape() {
    let lic = License::free_default(Uuid::nil());
    assert_eq!(lic.tier, Tier::Free);
    assert!(lic.expires_at.is_none());
    assert!(lic
        .features
        .contains(&"federation.receive_cards".to_string()));
    assert_eq!(lic.bought_addons.len(), 0);

    // Todos los reportes sobre datos del propio negocio se nombran en el
    // resumen: es lo que contesta `GET /api/v1/license`, y un negocio tiene
    // que poder leer ahí qué tiene incluido.
    for feature in [
        "reports.sales_daily",
        "reports.margins_daily",
        "reports.top_products",
        "reports.stock_rotation",
        "reports.near_expiry",
    ] {
        assert!(
            lic.features.contains(&feature.to_string()),
            "{feature} está incluido en Free y tiene que figurar en el resumen"
        );
    }
}
