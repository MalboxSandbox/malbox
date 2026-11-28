//! GitHub release lookup and verified asset downloads.

use crate::error::InstallError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

const GITHUB_API_BASE: &str = "https://api.github.com";

/// Release channel an installation tracks.
///
/// Mirrors the channel choice offered by the bootstrap script: a release is
/// "stable" when its tag carries no semver pre-release suffix (`v0.1.0`, not
/// `v0.1.0-alpha.5`). The GitHub `prerelease` flag is deliberately not used
/// for filtering - malbox currently publishes every release with it set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by the malbox CLI install wizard"
)]
pub enum Channel {
    /// Releases without a semver pre-release suffix.
    Stable,
    /// The newest release of any kind.
    #[default]
    Nightly,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Stable => write!(f, "stable"),
            Channel::Nightly => write!(f, "nightly"),
        }
    }
}

/// A published GitHub release, as returned by the releases API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    /// Git tag of the release, e.g. `v0.1.0`.
    pub tag_name: String,
    /// Display title of the release.
    pub name: Option<String>,
    /// Release notes in Markdown.
    pub body: Option<String>,
    /// Files attached to the release.
    pub assets: Vec<ReleaseAsset>,
}

/// A downloadable file attached to a release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    /// File name of the asset.
    pub name: String,
    /// Public download URL.
    pub browser_download_url: String,
    /// Size in bytes.
    pub size: u64,
}

impl Release {
    /// Strips the conventional `v` prefix from a tag to obtain the version.
    #[must_use]
    pub fn version_from_tag(tag: &str) -> &str {
        tag.strip_prefix('v').unwrap_or(tag)
    }

    /// Version of this release, without the `v` tag prefix.
    #[must_use]
    pub fn version(&self) -> &str {
        Self::version_from_tag(&self.tag_name)
    }

    /// A release belongs to the stable channel when its version is plain
    /// semver without a pre-release segment.
    pub fn is_stable(&self) -> bool {
        semver::Version::parse(self.version()).is_ok_and(|v| v.pre.is_empty())
    }

    /// Prebuilt `malboxd` archive for `arch`, if the release ships one.
    #[must_use]
    pub fn find_daemon_asset(&self, arch: &str) -> Option<&ReleaseAsset> {
        let expected = format!("malboxd-{}-{}.tar.gz", self.tag_name, arch);
        self.assets.iter().find(|a| a.name == expected)
    }

    /// Prebuilt `malbox` CLI archive for `arch`, if the release ships one.
    #[must_use]
    pub fn find_malbox_asset(&self, arch: &str) -> Option<&ReleaseAsset> {
        let expected = format!("malbox-{}-{}.tar.gz", self.tag_name, arch);
        self.assets.iter().find(|a| a.name == expected)
    }

    /// The prebuilt front-end SPA bundle. Architecture-independent (static
    /// HTML/JS), so it carries no arch suffix.
    pub fn find_web_asset(&self) -> Option<&ReleaseAsset> {
        let expected = format!("malbox-web-{}.tar.gz", self.tag_name);
        self.assets.iter().find(|a| a.name == expected)
    }

    /// GitHub source tarball URL for this release's tag in `owner/repo`.
    #[must_use]
    pub fn source_archive_url(&self, owner: &str, repo: &str) -> String {
        format!(
            "https://github.com/{owner}/{repo}/archive/refs/tags/{}.tar.gz",
            self.tag_name
        )
    }
}

/// Client for the GitHub releases API of one repository.
#[derive(Debug)]
pub struct GitHubClient {
    client: reqwest::Client,
    owner: String,
    repo: String,
}

impl GitHubClient {
    /// Creates a client for `owner/repo` with connect and idle-read timeouts.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Network`] when the HTTP client cannot be built.
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> crate::Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent("malbox-installer")
            .connect_timeout(Duration::from_secs(10))
            // Idle-read timeout rather than a whole-request timeout: large
            // asset downloads on slow links must not be killed mid-transfer.
            .read_timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            owner: owner.into(),
            repo: repo.into(),
        })
    }

    /// GitHub account or organization owning the repository.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Repository name.
    #[must_use]
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// GET a GitHub API endpoint as JSON. Returns `None` on 404 so callers
    /// can distinguish "does not exist" from real failures.
    async fn get_api<T: serde::de::DeserializeOwned>(&self, url: &str) -> crate::Result<Option<T>> {
        let mut request = self.client.get(url);
        // A token raises the unauthenticated rate limit (60 requests/hour).
        // Asset downloads stay tokenless: GitHub redirects them to S3, which
        // rejects requests that carry an Authorization header.
        if let Ok(token) = std::env::var("GITHUB_TOKEN")
            && !token.is_empty()
        {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if status == reqwest::StatusCode::FORBIDDEN
            && response
                .headers()
                .get("x-ratelimit-remaining")
                .is_some_and(|v| v == "0")
        {
            return Err(InstallError::GitHub(
                "GitHub API rate limit exceeded - set GITHUB_TOKEN to raise the limit".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(InstallError::GitHub(format!(
                "GitHub API request failed: HTTP {status}"
            )));
        }

        response
            .json()
            .await
            .map(Some)
            .map_err(|e| InstallError::GitHub(e.to_string()))
    }

    /// Newest published release on `channel`.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Network`] on request failure and [`InstallError::GitHub`]
    /// on API errors, rate limiting, or when no release matches the channel.
    pub async fn latest_release(&self, channel: Channel) -> crate::Result<Release> {
        // NOTE: deliberately not using `/releases/latest`. That endpoint only
        // ever returns the most recent *non-prerelease* release, and malbox
        // currently publishes every release as a prerelease. Instead we list
        // published releases (GitHub returns them newest-first) and take the
        // first one matching the channel. Unauthenticated requests never
        // include drafts.
        let url = format!(
            "{}/repos/{}/{}/releases?per_page=20",
            GITHUB_API_BASE, self.owner, self.repo
        );

        let releases: Vec<Release> = self
            .get_api(&url)
            .await?
            .ok_or_else(|| InstallError::GitHub("releases endpoint returned 404".to_string()))?;

        let release = match channel {
            Channel::Nightly => releases.into_iter().next(),
            Channel::Stable => releases.into_iter().find(Release::is_stable),
        };

        release.ok_or_else(|| match channel {
            Channel::Stable => InstallError::GitHub(
                "no stable release available yet (only pre-releases exist - try the nightly channel)"
                    .to_string(),
            ),
            Channel::Nightly => InstallError::GitHub("no releases found".to_string()),
        })
    }

    /// Release for an exact tag, or `None` when the tag has no release.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Network`] on request failure and [`InstallError::GitHub`]
    /// on non-404 API errors or rate limiting.
    pub async fn release_by_tag(&self, tag: &str) -> crate::Result<Option<Release>> {
        let url = format!(
            "{}/repos/{}/{}/releases/tags/{tag}",
            GITHUB_API_BASE, self.owner, self.repo
        );
        self.get_api(&url).await
    }

    /// Release matching an installed version, trying the canonical `v` tag
    /// prefix first. Resume and rebuild use this to pin to the installed
    /// version instead of floating to the latest release.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::GitHub`] when neither tag has a release or the API fails,
    /// and [`InstallError::Network`] on request failure.
    pub async fn release_for_version(&self, version: &str) -> crate::Result<Release> {
        if let Some(release) = self.release_by_tag(&format!("v{version}")).await? {
            return Ok(release);
        }
        if let Some(release) = self.release_by_tag(version).await? {
            return Ok(release);
        }
        Err(InstallError::GitHub(format!(
            "no release found for installed version {version}"
        )))
    }

    /// Download `url`, reporting `(bytes_downloaded, total_bytes)` after each
    /// chunk so callers can surface progress.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::GitHub`] on a non-success HTTP status and
    /// [`InstallError::Network`] when the request or body transfer fails.
    pub async fn download_asset(
        &self,
        url: &str,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> crate::Result<Vec<u8>> {
        let mut response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(InstallError::GitHub(format!(
                "failed to download asset: HTTP {}",
                response.status()
            )));
        }

        let total = response.content_length();
        let capacity = total.and_then(|len| usize::try_from(len).ok()).unwrap_or(0);
        let mut bytes = Vec::with_capacity(capacity);
        while let Some(chunk) = response.chunk().await? {
            bytes.extend_from_slice(&chunk);
            on_progress(u64::try_from(bytes.len()).unwrap_or(u64::MAX), total);
        }

        Ok(bytes)
    }

    /// Download a release asset and, when the release ships a sibling
    /// `<name>.sha256` asset (the release workflow generates one per
    /// tarball), verify the payload against it. URLs that do not correspond
    /// to a release asset (e.g. source tarballs) have no checksum and are
    /// returned as-is.
    ///
    /// # Errors
    ///
    /// Returns the download errors of [`Self::download_asset`],
    /// [`InstallError::GitHub`] for a malformed checksum file, and
    /// [`InstallError::ChecksumMismatch`] when the payload does not match.
    pub async fn download_verified(
        &self,
        release: &Release,
        url: &str,
        on_progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
    ) -> crate::Result<Vec<u8>> {
        let bytes = self.download_asset(url, on_progress).await?;

        let Some(asset) = release
            .assets
            .iter()
            .find(|a| a.browser_download_url == url)
        else {
            return Ok(bytes);
        };

        let checksum_name = format!("{}.sha256", asset.name);
        let Some(checksum_asset) = release.assets.iter().find(|a| a.name == checksum_name) else {
            return Ok(bytes);
        };

        let sidecar = self
            .download_asset(&checksum_asset.browser_download_url, &mut |_, _| {})
            .await?;
        let sidecar = String::from_utf8_lossy(&sidecar);
        let expected = sidecar.split_whitespace().next().ok_or_else(|| {
            InstallError::GitHub(format!("malformed checksum file {checksum_name}"))
        })?;

        let digest = Sha256::digest(&bytes);
        let actual = hex::encode(digest);

        if !actual.eq_ignore_ascii_case(expected) {
            return Err(InstallError::ChecksumMismatch {
                asset: asset.name.clone(),
            });
        }

        Ok(bytes)
    }
}
