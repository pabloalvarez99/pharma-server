use license::schema::{License, Tier};
use uuid::Uuid;

#[test]
fn free_default_shape() {
    let lic = License::free_default(Uuid::nil());
    assert_eq!(lic.tier, Tier::Free);
    assert!(lic.expires_at.is_none());
    assert!(lic.features.contains(&"reports.sales_daily".to_string()));
    assert!(lic
        .features
        .contains(&"federation.receive_cards".to_string()));
    assert_eq!(lic.bought_addons.len(), 0);
}
