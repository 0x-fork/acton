//! Release coordinates and platform mapping for TON binary archives.
//!
//! Each supported OS and CPU architecture maps to one archive filename and its
//! expected SHA-256 digest. Automatic installation is available for macOS and
//! Linux on arm64 and x86_64; other platforms must provide `--ton-bin-dir`.

use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

use crate::storage::TON_RELEASE;

const OFFICIAL_REPOSITORY: &str = "ton-blockchain/ton";

/// Identifies a compatible TON ZIP without retaining download credentials.
/// Custom sources use either an explicit SHA-256 or the GitHub asset digest.
/// Cache identities include the checksum so changed releases cannot replace
/// binaries already referenced by a node's persisted state.
pub(crate) struct ReleaseSource {
    pub(super) repository: String,
    pub(super) tag: String,
    pub(super) asset: String,
    pub(super) sha256: Option<String>,
    pub(super) pinned: bool,
}

impl ReleaseSource {
    /// Uses the official release and platform checksum shipped with Localton.
    pub(crate) fn pinned() -> Result<Self> {
        Self::new(OFFICIAL_REPOSITORY.into(), TON_RELEASE.into(), None, None)
    }

    /// Validates source coordinates before they can reach URLs or cache paths.
    /// Tags are exact GitHub tags; asset names default to this platform's ZIP.
    pub(crate) fn new(
        repository: String,
        tag: String,
        asset: Option<String>,
        sha256: Option<String>,
    ) -> Result<Self> {
        let parts: Vec<_> = repository.split('/').collect();
        ensure!(
            parts.len() == 2
                && parts.iter().all(|part| {
                    !part.is_empty()
                        && !matches!(*part, "." | "..")
                        && part.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                        })
                }),
            "repository must use OWNER/REPO format"
        );
        ensure!(
            !tag.is_empty()
                && !matches!(tag.as_str(), "." | "..")
                && !tag.chars().any(char::is_control),
            "release must be a nonempty GitHub tag"
        );

        let platform_asset = current_asset()?;
        let repository = repository.to_ascii_lowercase();
        let asset = asset.unwrap_or_else(|| platform_asset.file_name.to_owned());
        ensure!(
            asset.ends_with(".zip")
                && !asset.contains(['/', '\\'])
                && !asset.chars().any(char::is_control),
            "asset must be a ZIP filename"
        );
        let sha256 = sha256.map(|digest| digest.to_ascii_lowercase());
        if let Some(digest) = &sha256 {
            validate_sha256(digest)?;
        }

        let pinned = repository == OFFICIAL_REPOSITORY
            && tag == TON_RELEASE
            && asset == platform_asset.file_name
            && sha256
                .as_deref()
                .is_none_or(|digest| digest == platform_asset.sha256);
        let sha256 = sha256.or_else(|| pinned.then(|| platform_asset.sha256.to_owned()));

        Ok(Self {
            repository,
            tag,
            asset,
            sha256,
            pinned,
        })
    }

    /// Separates custom builds by source and digest without using tags as paths.
    pub(super) fn cache_dir(&self, root: &Path, sha256: &str) -> PathBuf {
        if self.pinned {
            return root.join("ton").join(TON_RELEASE);
        }

        let identity = serde_json::to_vec(&[&self.repository, &self.tag, &self.asset, sha256])
            .expect("release coordinates are serializable");
        root.join("ton")
            .join("github")
            .join(hex::encode(Sha256::digest(identity)))
    }
}

/// Applies the same checksum format to user input and GitHub metadata.
pub(super) fn validate_sha256(digest: &str) -> Result<()> {
    ensure!(
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "SHA-256 must contain exactly 64 hexadecimal digits"
    );
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReleaseAsset {
    pub file_name: &'static str,
    pub sha256: &'static str,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-mac-arm64.zip",
        sha256: "46cb42491213adf2adc9cb7e94c922249a586ba55b7dfe4e66626695a68b9d4b",
    })
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-mac-x86-64.zip",
        sha256: "479cfb6ac9a750683816c1db7501bdc6d2d1d5fdb9a11861b784efc4f734be2a",
    })
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-linux-arm64.zip",
        sha256: "acb73cd85118754e149744d35c3ef4775f065c8545b49600f70ffe8dea68060e",
    })
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    Ok(ReleaseAsset {
        file_name: "ton-linux-x86_64.zip",
        sha256: "548f0f29b5ebf1a42d6aa23438b25c7a0c05b2330a7f278d2ca83fc2015685e9",
    })
}

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64")
)))]
pub(super) fn current_asset() -> Result<ReleaseAsset> {
    anyhow::bail!("automatic TON binary installation supports macOS/Linux on arm64/x86_64")
}

pub(super) fn platform_id() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_asset_has_sha256() {
        let asset = current_asset().unwrap();
        assert_eq!(asset.sha256.len(), 64);
        assert!(asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}
