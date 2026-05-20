mod common;

use chrono::{Duration, Utc};
use common::build_license;
use license::gate::{is_expired, is_in_grace};
use license::schema::{License, Tier};
use uuid::Uuid;

#[test]
fn free_perpetual_never_expires() {
    let lic = License::free_default(Uuid::nil());
    assert!(!is_expired(&lic, Utc::now(), Duration::days(30)));
    assert!(!is_in_grace(&lic, Utc::now(), Duration::days(30)));
}

#[test]
fn expired_outside_grace_is_expired() {
    let past = Utc::now() - Duration::days(60);
    let lic = build_license(Tier::Pro, vec!["reports.margins_daily"], Some(past));
    assert!(is_expired(&lic, Utc::now(), Duration::days(30)));
    assert!(!is_in_grace(&lic, Utc::now(), Duration::days(30)));
}

#[test]
fn expired_inside_grace_is_in_grace() {
    let past = Utc::now() - Duration::days(5);
    let lic = build_license(Tier::Pro, vec!["reports.margins_daily"], Some(past));
    assert!(!is_expired(&lic, Utc::now(), Duration::days(30)));
    assert!(is_in_grace(&lic, Utc::now(), Duration::days(30)));
}
