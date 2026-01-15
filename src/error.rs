use thiserror::Error;

/// Errors that can occur when using the ReleaseNotifier.
#[derive(Error, Debug)]
pub enum ReleaseNotifierError {
    /// Error making HTTP request to GitHub API.
    #[error("Failed to fetch releases from GitHub: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Error parsing JSON response.
    #[error("Failed to parse GitHub API response: {0}")]
    JsonError(#[from] serde_json::Error),

    /// GitHub API returned an error status.
    #[error("GitHub API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    /// Invalid repository format.
    #[error("Invalid repository format: expected 'owner/repo', got '{0}'")]
    InvalidRepo(String),

    /// Invalid base URL.
    #[error("Invalid base URL: {0}")]
    InvalidBaseUrl(String),

    /// Invalid cache file path (parent directory does not exist).
    #[error("Invalid cache file path: parent directory does not exist for '{0}'")]
    InvalidCacheFilePath(String),

    /// IO error (cache file operations).
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type alias for ReleaseNotifier operations.
pub type Result<T> = std::result::Result<T, ReleaseNotifierError>;
