//! Persistence helpers for `data/license.json`. Verify on load; pretty-print
//! on save (so humans can inspect / support can paste in bug reports).

use std::path::{Path, PathBuf};

use crate::schema::License;
use crate::verify::{parse_and_verify, LicenseError};

pub fn default_license_path(data_dir: &Path) -> PathBuf {
    data_dir.join("license.json")
}

/// Read + verify the on-disk license. Caller decides what to do on error
/// (typically: log + fall back to [`License::free_default`]).
pub fn load_from_disk(path: &Path) -> Result<License, LicenseError> {
    let bytes = std::fs::read(path)?;
    parse_and_verify(&bytes)
}

/// Write a license to disk pretty-printed. Does NOT verify — caller must
/// pass an already-validated license (typically the output of
/// `parse_and_verify`).
pub fn save_to_disk(license: &License, path: &Path) -> Result<(), LicenseError> {
    let json = serde_json::to_vec_pretty(license)?;
    std::fs::write(path, json)?;
    Ok(())
}
