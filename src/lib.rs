//! # gh-release-update-notifier
//!
//! A library for checking GitHub releases and notifying about updates.
//!
//! ## Example
//!
//! ```no_run
//! use relnotify::{ReleaseNotifier, ReleaseNotifierConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ReleaseNotifierConfig::new("owner/repo")
//!         .check_interval(3600000) // 1 hour
//!         .cache_file_path("/tmp/release-cache.json");
//!
//!     let notifier = ReleaseNotifier::new(config)?;
//!
//!     // Get the latest stable release
//!     if let Some(release) = notifier.get_latest_release(false).await? {
//!         println!("Latest release: {}", release.tag_name);
//!     }
//!
//!     // Check if an update is available
//!     let result = notifier.check_version("1.0.0", false).await?;
//!     if result.update_available {
//!         if let Some(latest) = result.latest_release {
//!             println!("Update available: {}", latest.tag_name);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

mod error;
mod notifier;
mod types;

pub use error::{ReleaseNotifierError, Result};
pub use notifier::ReleaseNotifier;
pub use types::{Release, ReleaseNotifierConfig, VersionCheckResult};
