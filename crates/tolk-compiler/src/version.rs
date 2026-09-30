use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::ffi::{CStr, c_char, c_void};

/// Version of Tolk bundled with this crate.
///
/// Use this constant for defaults such as verification requests.
pub const TOLK_VERSION: &str = "1.5.0";

/// TON source revision of the bundled native compiler. Persistent build caches
/// include it to distinguish compiler builds with the same Tolk version.
pub const TOLK_COMMIT_HASH: &str = "9334027d63528da1a78b2efbe898bc586d4e6cec";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeTolkVersion {
    #[serde(rename = "tolkVersion")]
    pub version: String,
    #[serde(rename = "tolkFiftLibCommitHash")]
    pub ton_commit_hash: String,
    #[serde(rename = "tolkFiftLibCommitDate")]
    pub ton_commit_date: String,
}

pub fn native_tolk_version() -> Result<NativeTolkVersion> {
    // SAFETY: `tolk_version` is a pure native accessor that returns either
    // a valid null-terminated string allocated with `malloc` or a null pointer.
    #[allow(unsafe_code)]
    let raw = unsafe { tolk_version() };
    if raw.is_null() {
        bail!("native tolk returned a null version pointer");
    }

    // SAFETY: `raw` was checked for null above and points to a valid
    // null-terminated string; the upstream implementation returns it via
    // `strdup`, so freeing it with libc `free` is the correct teardown path.
    #[allow(unsafe_code)]
    let json = unsafe {
        let json = CStr::from_ptr(raw).to_string_lossy().into_owned();
        free(raw.cast_mut().cast::<c_void>());
        json
    };

    parse_native_tolk_version(&json)
}

fn parse_native_tolk_version(json: &str) -> Result<NativeTolkVersion> {
    serde_json::from_str(json).context("failed to parse native tolk version JSON")
}

#[allow(unsafe_code)]
unsafe extern "C" {
    fn tolk_version() -> *const c_char;
    fn free(ptr: *mut c_void);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_version_matches_linked_compiler() {
        let native = native_tolk_version().unwrap();
        assert_eq!(native.version, TOLK_VERSION);
        assert_eq!(native.ton_commit_hash, TOLK_COMMIT_HASH);
    }

    #[test]
    fn tolk_version_json_deserializes() {
        let version = parse_native_tolk_version(
            r#"{
                "tolkVersion":"0.99.0",
                "tolkFiftLibCommitHash":"0fbeb9f",
                "tolkFiftLibCommitDate":"2026-03-24T00:00:00Z"
            }"#,
        )
        .expect("failed to parse tolk version JSON");

        assert_eq!(version.version, "0.99.0");
        assert_eq!(version.ton_commit_hash, "0fbeb9f");
        assert_eq!(version.ton_commit_date, "2026-03-24T00:00:00Z");
    }
}
