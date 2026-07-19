mod common;

use chrono::{Duration, Utc};
use common::build_license;
use license::gate::{entitled, require};
use license::schema::Tier;

#[test]
fn pro_license_with_feature_allows_it() {
    let lic = build_license(
        Tier::Pro,
        vec!["reports.margins_daily"],
        Some(Utc::now() + Duration::days(30)),
    );
    assert!(entitled(&lic, "reports.margins_daily"));
    require(&lic, "reports.margins_daily").expect("allowed");
}
