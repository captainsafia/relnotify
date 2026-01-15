# relnotify

A lightweight Rust library for checking GitHub Releases to notify CLI users of available updates, with built-in caching and disk persistence.

## Installation

```sh
cargo add relnotify
cargo add tokio --features full
```

## Usage

### Basic Usage

```rust
use relnotify::{ReleaseNotifier, ReleaseNotifierConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ReleaseNotifierConfig::new("owner/repo");
    let notifier = ReleaseNotifier::new(config)?;

    // Check if an update is available
    let result = notifier.check_version("1.0.0", false).await?;

    if result.update_available {
        if let Some(release) = &result.latest_release {
            println!("Update available: {}", release.tag_name);
            println!("Download: {}", release.html_url);
        }
    }

    Ok(())
}
```

### Get Latest Release

```rust
let config = ReleaseNotifierConfig::new("owner/repo");
let notifier = ReleaseNotifier::new(config)?;

// Get the latest stable release
if let Some(stable) = notifier.get_latest_release(false).await? {
    println!("Latest stable: {}", stable.tag_name);
}

// Include prereleases in the search
if let Some(latest) = notifier.get_latest_release(true).await? {
    println!("Latest (including prereleases): {}", latest.tag_name);
}
```

### Get Latest Prerelease

```rust
let config = ReleaseNotifierConfig::new("owner/repo");
let notifier = ReleaseNotifier::new(config)?;

if let Some(prerelease) = notifier.get_latest_prerelease().await? {
    println!("Latest prerelease: {}", prerelease.tag_name);
}
```

### Check Version with Prereleases

```rust
let config = ReleaseNotifierConfig::new("owner/repo");
let notifier = ReleaseNotifier::new(config)?;

// Check against the latest prerelease
let result = notifier.check_version("2.0.0-beta.1", true).await?;

if result.update_available {
    if let Some(release) = &result.latest_release {
        println!("New prerelease available: {}", release.tag_name);
    }
}
```

### Caching Configuration

```rust
let config = ReleaseNotifierConfig::new("owner/repo")
    // Check interval in milliseconds (default: 1 hour)
    .check_interval(3600000)
    // Optional: persist cache to disk
    .cache_file_path("/path/to/cache.json");

let notifier = ReleaseNotifier::new(config)?;

// Clear the cache manually
notifier.clear_cache();
```

## CLI Integration Example

```rust
use relnotify::{ReleaseNotifier, ReleaseNotifierConfig};
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() {
    // Run update check in the background
    tokio::spawn(check_for_updates());

    // ... rest of your CLI logic
}

async fn check_for_updates() {
    let home = env::var("HOME").unwrap_or_default();

    let config = ReleaseNotifierConfig::new("your-org/your-cli")
        .check_interval(86400000) // Check once per day
        .cache_file_path(format!("{}/.your-cli/update-cache.json", home))
        .token(env::var("GITHUB_TOKEN").ok().unwrap_or_default());

    let Ok(notifier) = ReleaseNotifier::new(config) else {
        return; // Silently fail
    };

    if let Ok(result) = notifier.check_version(VERSION, false).await {
        if result.update_available {
            if let Some(release) = result.latest_release {
                eprintln!("\n Update available: {} -> {}", VERSION, release.tag_name);
                eprintln!("   Run: cargo install your-cli");
                eprintln!("   Or visit: {}\n", release.html_url);
            }
        }
    }
    // Silently fail - don't block the CLI for update checks
}
```
