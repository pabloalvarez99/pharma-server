mod common;

use license::gate::{entitled, require};
use license::schema::License;
use uuid::Uuid;

#[test]
fn free_default_blocks_paid_features() {
    let lic = License::free_default(Uuid::nil());
    assert!(entitled(&lic, "reports.sales_daily"));
    assert!(!entitled(&lic, "integrations.sii_dte_auto"));
    let err = require(&lic, "integrations.sii_dte_auto").unwrap_err();
    assert_eq!(err.feature, "integrations.sii_dte_auto");
    assert_eq!(err.tier_required, "pro");
}

/// Free es el piso del producto, no una entrada de una lista.
///
/// Un negocio que se dio de alta antes de que un feature bajara a Free tiene en
/// disco un archivo firmado con la lista vieja. Si `entitled` sólo mirara esa
/// lista, ese negocio seguiría sin poder ver algo que el producto ya anuncia
/// como incluido, y no habría forma de que nadie se enterara: no falla, no
/// avisa, simplemente no está.
#[test]
fn free_features_are_granted_even_if_the_file_does_not_list_them() {
    let mut lic = License::free_default(Uuid::nil());
    lic.features.clear();

    for feature in [
        "reports.sales_daily",
        "reports.margins_daily",
        "reports.top_products",
        "reports.stock_rotation",
        "reports.near_expiry",
    ] {
        assert!(
            entitled(&lic, feature),
            "{feature} es de tier Free: tiene que estar incluido con la lista vacía"
        );
        require(&lic, feature).expect("un feature de tier Free nunca puede requerir upgrade");
    }
}

/// Lo que sí se cobra sigue cobrándose: el piso Free no abre lo de arriba.
#[test]
fn an_empty_file_does_not_grant_paid_features() {
    let mut lic = License::free_default(Uuid::nil());
    lic.features.clear();

    for feature in [
        "integrations.sii_dte_auto",
        "federation.online_sync",
        "backup.s3_compat",
        "branding.white_label",
    ] {
        assert!(!entitled(&lic, feature), "{feature} no es gratis");
    }
}
