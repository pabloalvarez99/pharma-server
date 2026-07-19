mod common;

use license::gate::{entitled, require};
use license::schema::License;
use uuid::Uuid;

#[test]
fn free_default_blocks_paid_features() {
    let lic = License::free_default(Uuid::nil());
    assert!(entitled(&lic, "reports.sales_daily"));
    assert!(!entitled(&lic, "reports.margins_daily"));
    let err = require(&lic, "reports.margins_daily").unwrap_err();
    assert_eq!(err.feature, "reports.margins_daily");
    assert_eq!(err.tier_required, "pro");
}
