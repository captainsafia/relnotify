use std::fs;
use std::path::Path;
use std::sync::Mutex;

use chrono::Utc;
use reqwest::Client;
use url::Url;

use crate::error::{ReleaseNotifierError, Result};
use crate::types::{
    CacheData, GitHubReleaseResponse, Release, ReleaseNotifierConfig, VersionCheckResult,
};

/// A notifier for checking GitHub release updates.
pub struct ReleaseNotifier {
    config: ReleaseNotifierConfig,
    client: Client,
    cache: Mutex<Cache>,
}

struct Cache {
    releases: Vec<Release>,
    last_fetch_time: Option<i64>,
}

impl ReleaseNotifier {
    /// Creates a new ReleaseNotifier with the given configuration.
    ///
    /// If a cache file path is configured and the file exists, the cache will be loaded from disk.
    pub fn new(config: ReleaseNotifierConfig) -> Result<Self> {
        // Validate repo format
        if !is_valid_repo_format(&config.repo) {
            return Err(ReleaseNotifierError::InvalidRepo(config.repo.clone()));
        }

        // Validate base URL
        if Url::parse(&config.base_url).is_err() {
            return Err(ReleaseNotifierError::InvalidBaseUrl(config.base_url.clone()));
        }

        // Validate cache file path (parent directory must exist)
        if let Some(ref path) = config.cache_file_path {
            let path = Path::new(path);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    return Err(ReleaseNotifierError::InvalidCacheFilePath(
                        config.cache_file_path.clone().unwrap(),
                    ));
                }
            }
        }

        let client = Client::new();

        let cache = if let Some(ref path) = config.cache_file_path {
            Self::load_cache_from_disk(path).unwrap_or(Cache {
                releases: Vec::new(),
                last_fetch_time: None,
            })
        } else {
            Cache {
                releases: Vec::new(),
                last_fetch_time: None,
            }
        };

        Ok(Self {
            config,
            client,
            cache: Mutex::new(cache),
        })
    }

    /// Gets the latest stable release from the repository.
    ///
    /// # Arguments
    /// * `include_prerelease` - If true, prereleases will be included in the search.
    ///
    /// # Returns
    /// The latest release, or None if no releases are found.
    pub async fn get_latest_release(&self, include_prerelease: bool) -> Result<Option<Release>> {
        let releases = self.fetch_all_releases().await?;

        let release = releases
            .into_iter()
            .filter(|r| !r.draft)
            .filter(|r| include_prerelease || !r.prerelease)
            .max_by_key(|r| r.published_at);

        Ok(release)
    }

    /// Gets the latest prerelease from the repository.
    ///
    /// # Returns
    /// The latest prerelease, or None if no prereleases are found.
    pub async fn get_latest_prerelease(&self) -> Result<Option<Release>> {
        let releases = self.fetch_all_releases().await?;

        let release = releases
            .into_iter()
            .filter(|r| !r.draft && r.prerelease)
            .max_by_key(|r| r.published_at);

        Ok(release)
    }

    /// Checks if a newer version is available.
    ///
    /// # Arguments
    /// * `current_version` - The current version string (with or without 'v' prefix).
    /// * `is_prerelease` - If true, checks against prereleases; otherwise checks stable releases.
    ///
    /// # Returns
    /// A VersionCheckResult indicating if an update is available and the latest release.
    pub async fn check_version(
        &self,
        current_version: &str,
        is_prerelease: bool,
    ) -> Result<VersionCheckResult> {
        let latest_release = if is_prerelease {
            self.get_latest_prerelease().await?
        } else {
            self.get_latest_release(false).await?
        };

        let Some(latest) = latest_release else {
            return Ok(VersionCheckResult {
                update_available: false,
                latest_release: None,
            });
        };

        // Check if current version is older than the latest
        let releases = self.fetch_all_releases().await?;
        let update_available = self.is_version_older(current_version, &latest, &releases);

        Ok(VersionCheckResult {
            update_available,
            latest_release: Some(latest),
        })
    }

    /// Clears both in-memory and disk cache.
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.releases.clear();
        cache.last_fetch_time = None;

        // Clear disk cache if configured
        if let Some(ref path) = self.config.cache_file_path {
            let _ = fs::remove_file(path);
        }
    }

    /// Fetches all releases, using cache if available and valid.
    async fn fetch_all_releases(&self) -> Result<Vec<Release>> {
        // Check if we have a valid cache
        if self.config.check_interval > 0 {
            let cache = self.cache.lock().unwrap();
            if let Some(last_fetch) = cache.last_fetch_time {
                let now = Utc::now().timestamp_millis();
                if now - last_fetch < self.config.check_interval as i64
                    && !cache.releases.is_empty()
                {
                    return Ok(cache.releases.clone());
                }
            }
        }

        let releases = self.fetch_from_github().await?;

        {
            let mut cache = self.cache.lock().unwrap();
            cache.releases = releases.clone();
            cache.last_fetch_time = Some(Utc::now().timestamp_millis());
        }

        if let Some(ref path) = self.config.cache_file_path {
            let _ = self.save_cache_to_disk(path);
        }

        Ok(releases)
    }

    /// Fetches releases directly from the GitHub API.
    async fn fetch_from_github(&self) -> Result<Vec<Release>> {
        let url = format!(
            "{}/repos/{}/releases",
            self.config.base_url, self.config.repo
        );

        let mut request = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "gh-release-update-notifier-rs");

        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ReleaseNotifierError::ApiError { status, message });
        }

        let github_releases: Vec<GitHubReleaseResponse> = response.json().await?;

        let releases: Vec<Release> = github_releases.into_iter().map(Release::from).collect();

        Ok(releases)
    }

    /// Loads cache from disk.
    fn load_cache_from_disk(path: &str) -> Option<Cache> {
        let content = fs::read_to_string(path).ok()?;
        let data: CacheData = serde_json::from_str(&content).ok()?;
        Some(Cache {
            releases: data.releases,
            last_fetch_time: Some(data.last_fetch_time),
        })
    }

    /// Saves cache to disk.
    fn save_cache_to_disk(&self, path: &str) -> Result<()> {
        let cache = self.cache.lock().unwrap();
        let data = CacheData {
            releases: cache.releases.clone(),
            last_fetch_time: cache.last_fetch_time.unwrap_or_else(|| Utc::now().timestamp_millis()),
        };
        let content = serde_json::to_string(&data)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Finds a release by its version tag.
    ///
    /// Handles version strings with or without 'v' prefix.
    fn find_release_by_version<'a>(
        &self,
        version: &str,
        releases: &'a [Release],
    ) -> Option<&'a Release> {
        let normalized_version = version.strip_prefix('v').unwrap_or(version);

        releases.iter().find(|r| {
            let normalized_tag = r.tag_name.strip_prefix('v').unwrap_or(&r.tag_name);
            normalized_tag == normalized_version
        })
    }

    /// Determines if the current version is older than the latest release.
    /// Uses publish date for comparison, not semantic versioning to handle
    /// varying versioning schemes.
    fn is_version_older(
        &self,
        current_version: &str,
        latest: &Release,
        releases: &[Release],
    ) -> bool {
        // Find the current version's release to get its publish date
        let Some(current) = self.find_release_by_version(current_version, releases) else {
            // If we can't find the current version, assume it's not older
            // (user might be on an unreleased/dev version)
            return false;
        };

        // Compare by publish date to get a true representation of "newer"
        match (current.published_at, latest.published_at) {
            (Some(current_date), Some(latest_date)) => current_date < latest_date,
            _ => false,
        }
    }
}

/// Maximum length for a GitHub username/organization name.
/// This limit is enforced by GitHub.
const MAX_GITHUB_OWNER_LENGTH: usize = 39;

/// Maximum length for a GitHub repository name.
/// This limit is enforced by GitHub.
const MAX_GITHUB_REPO_LENGTH: usize = 100;

/// Validates that a repo string is in valid "owner/repo" format.
///
/// GitHub requirements:
/// - Owner: alphanumeric or hyphens, cannot start/end with hyphen, max 39 chars
/// - Repo: alphanumeric, hyphens, underscores, or dots, max 100 chars
fn is_valid_repo_format(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else {
        return false;
    };

    // Check no additional slashes
    if name.contains('/') {
        return false;
    }

    is_valid_owner(owner) && is_valid_repo_name(name)
}

fn is_valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= MAX_GITHUB_OWNER_LENGTH
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn is_valid_repo_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_GITHUB_REPO_LENGTH
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_repo_format() {
        let invalid_repos = [
            "invalid-repo",       // no slash
            "too/many/slashes",   // multiple slashes
            "/repo",              // empty owner
            "owner/",             // empty repo
            "-owner/repo",        // owner starts with hyphen
            "owner-/repo",        // owner ends with hyphen
            "owner/repo name",    // space in repo
            "own er/repo",        // space in owner
            "owner//repo",        // double slash (empty repo)
        ];

        for repo in invalid_repos {
            let config = ReleaseNotifierConfig::new(repo);
            let result = ReleaseNotifier::new(config);
            assert!(result.is_err(), "Expected '{}' to be invalid", repo);
        }
    }

    #[test]
    fn test_invalid_repo_format_owner_too_long() {
        let long_owner = "a".repeat(40);
        let repo = format!("{}/repo", long_owner);
        let config = ReleaseNotifierConfig::new(repo);
        let result = ReleaseNotifier::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_repo_format() {
        let valid_repos = [
            "owner/repo",
            "my-org/my-repo",
            "owner/repo.js",
            "owner/repo_name",
            "Owner123/Repo456",
        ];

        for repo in valid_repos {
            let config = ReleaseNotifierConfig::new(repo);
            let result = ReleaseNotifier::new(config);
            assert!(result.is_ok(), "Expected '{}' to be valid", repo);
        }
    }

    #[test]
    fn test_config_builder() {
        let config = ReleaseNotifierConfig::new("owner/repo")
            .check_interval(60000)
            .cache_file_path("/tmp/cache.json")
            .token("test-token");

        assert_eq!(config.repo, "owner/repo");
        assert_eq!(config.check_interval, 60000);
        assert_eq!(config.cache_file_path, Some("/tmp/cache.json".to_string()));
        assert_eq!(config.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_invalid_base_url() {
        let config = ReleaseNotifierConfig::new("owner/repo")
            .base_url("not-a-valid-url");
        let result = ReleaseNotifier::new(config);
        assert!(result.is_err());

        let Err(ReleaseNotifierError::InvalidBaseUrl(url)) = result else {
            panic!("Expected InvalidBaseUrl error");
        };
        assert_eq!(url, "not-a-valid-url");
    }

    #[test]
    fn test_invalid_cache_file_path() {
        let config = ReleaseNotifierConfig::new("owner/repo")
            .cache_file_path("/nonexistent/directory/cache.json");
        let result = ReleaseNotifier::new(config);
        assert!(result.is_err());

        let Err(ReleaseNotifierError::InvalidCacheFilePath(path)) = result else {
            panic!("Expected InvalidCacheFilePath error");
        };
        assert_eq!(path, "/nonexistent/directory/cache.json");
    }

    #[test]
    fn test_valid_base_url() {
        let config = ReleaseNotifierConfig::new("owner/repo")
            .base_url("https://github.example.com/api");
        let result = ReleaseNotifier::new(config);
        assert!(result.is_ok());
    }
}
