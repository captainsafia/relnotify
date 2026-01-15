use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for the ReleaseNotifier.
#[derive(Debug, Clone)]
pub struct ReleaseNotifierConfig {
    /// The repository in "owner/repo" format.
    pub repo: String,
    /// The interval in milliseconds between checks. Default is 3600000 (1 hour).
    /// Set to 0 to disable caching.
    pub check_interval: u64,
    /// Optional path to a file for persisting cache to disk.
    pub cache_file_path: Option<String>,
    /// Optional GitHub API token for authentication.
    pub token: Option<String>,
    /// Base URL for GitHub API (for testing). Defaults to "https://api.github.com".
    pub(crate) base_url: String,
}

impl ReleaseNotifierConfig {
    /// Creates a new config with the given repository.
    pub fn new(repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            check_interval: 3600000, // 1 hour default
            cache_file_path: None,
            token: None,
            base_url: "https://api.github.com".to_string(),
        }
    }

    /// Sets a custom base URL (for testing).
    #[doc(hidden)]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Sets the check interval in milliseconds.
    pub fn check_interval(mut self, interval: u64) -> Self {
        self.check_interval = interval;
        self
    }

    /// Sets the cache file path.
    pub fn cache_file_path(mut self, path: impl Into<String>) -> Self {
        self.cache_file_path = Some(path.into());
        self
    }

    /// Sets the GitHub API token.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }
}

/// Represents a GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    /// The release tag name (e.g., "v1.0.0").
    pub tag_name: String,
    /// The release name/title.
    pub name: Option<String>,
    /// The release body/description.
    pub body: Option<String>,
    /// Whether this is a prerelease.
    pub prerelease: bool,
    /// Whether this is a draft release.
    pub draft: bool,
    /// The URL to the release page.
    pub html_url: String,
    /// When the release was published.
    pub published_at: Option<DateTime<Utc>>,
}

/// The result of a version check.
#[derive(Debug, Clone)]
pub struct VersionCheckResult {
    /// Whether an update is available.
    pub update_available: bool,
    /// The latest release, if any.
    pub latest_release: Option<Release>,
}

/// Internal structure for GitHub API response.
#[derive(Debug, Deserialize)]
pub(crate) struct GitHubReleaseResponse {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub prerelease: bool,
    pub draft: bool,
    pub html_url: String,
    pub published_at: Option<DateTime<Utc>>,
}

impl From<GitHubReleaseResponse> for Release {
    fn from(response: GitHubReleaseResponse) -> Self {
        Self {
            tag_name: response.tag_name,
            name: response.name,
            body: response.body,
            prerelease: response.prerelease,
            draft: response.draft,
            html_url: response.html_url,
            published_at: response.published_at,
        }
    }
}

/// Internal cache data structure for disk persistence.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CacheData {
    pub releases: Vec<Release>,
    pub last_fetch_time: i64,
}
