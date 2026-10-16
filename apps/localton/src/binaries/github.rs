//! GitHub release metadata and authenticated asset requests.
//!
//! Download URLs are constructed from validated repository coordinates and asset
//! IDs. Credentials are attached only to GitHub API requests; reqwest removes
//! authorization when following redirects to another origin.

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder, Url, header};
use serde::Deserialize;

use super::release::{ReleaseSource, validate_sha256};

/// Owns the HTTP session and transient authorization for one installation.
/// Requests to public download URLs never receive the token.
pub(super) struct GitHub {
    client: Client,
    api_base: Url,
    token: Option<header::HeaderValue>,
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
/// Metadata for one release ZIP, identified by GitHub's immutable asset ID.
/// The digest can be absent on older assets, which require an explicit checksum.
pub(super) struct Asset {
    id: u64,
    name: String,
    digest: Option<String>,
}

impl GitHub {
    /// Rejects invalid credentials without including their contents in errors.
    pub(super) fn new(token: Option<&str>) -> Result<Self> {
        let token = token
            .map(|token| {
                anyhow::ensure!(
                    !token.is_empty() && token.bytes().all(|byte| byte.is_ascii_graphic()),
                    "GitHub token must be nonempty and contain no whitespace or control characters"
                );
                let mut value = header::HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid GitHub token")?;
                value.set_sensitive(true);
                Ok::<_, anyhow::Error>(value)
            })
            .transpose()?;
        let client = Client::builder()
            .user_agent(concat!("localton/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(900))
            .build()
            .context("failed to create GitHub client")?;
        Ok(Self {
            client,
            api_base: Url::parse("https://api.github.com/")?,
            token,
        })
    }

    /// Encodes each path segment separately so tags cannot alter the API route.
    fn request(&self, source: &ReleaseSource, suffix: &[&str], accept: &str) -> RequestBuilder {
        let mut url = self.api_base.clone();
        url.path_segments_mut()
            .expect("GitHub API is a base URL")
            .pop_if_empty()
            .push("repos")
            .extend(source.repository.split('/'))
            .extend(suffix);
        let mut request = self
            .client
            .get(url)
            .header(header::ACCEPT, accept)
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.header(header::AUTHORIZATION, token.clone());
        }
        request
    }

    /// Resolves an exact tag and filename, including assets in private releases.
    pub(super) async fn asset(&self, source: &ReleaseSource) -> Result<Asset> {
        let request = self.request(
            source,
            &["releases", "tags", &source.tag],
            "application/vnd.github+json",
        );
        let response = request
            .send()
            .await
            .map_err(reqwest::Error::without_url)
            .context("failed to contact GitHub release API")?
            .error_for_status()
            .map_err(reqwest::Error::without_url)
            .with_context(|| format!(
                "cannot read GitHub release {}@{}; check the tag and token's repository contents access",
                source.repository, source.tag,
            ))?;
        let release: Release = response
            .json()
            .await
            .map_err(reqwest::Error::without_url)
            .context("invalid GitHub release metadata")?;
        release
            .assets
            .into_iter()
            .find(|asset| asset.name == source.asset)
            .with_context(|| {
                format!(
                    "GitHub release {}@{} has no asset {}",
                    source.repository, source.tag, source.asset,
                )
            })
    }

    /// Requests binary content through the API, which can stream or redirect it.
    pub(super) fn download(&self, source: &ReleaseSource, asset: &Asset) -> RequestBuilder {
        self.request(
            source,
            &["releases", "assets", &asset.id.to_string()],
            "application/octet-stream",
        )
    }

    /// Downloads the known public archive without spending a GitHub API request.
    pub(super) fn pinned_download(&self, source: &ReleaseSource) -> RequestBuilder {
        self.client.get(format!(
            "https://github.com/{}/releases/download/{}/{}",
            source.repository, source.tag, source.asset,
        ))
    }
}

impl Asset {
    /// Requires a usable SHA-256 digest before the installer accepts any bytes.
    pub(super) fn sha256(&self) -> Result<String> {
        let digest = self
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .context(
                "GitHub asset has no SHA-256 digest; supply --sha256 with the expected checksum",
            )?;
        validate_sha256(digest).context("invalid GitHub asset SHA-256 digest")?;
        Ok(digest.to_ascii_lowercase())
    }
}

#[cfg(test)]
#[path = "../../tests/binaries/github.rs"]
mod tests;
