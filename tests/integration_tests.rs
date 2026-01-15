use relnotify::{ReleaseNotifier, ReleaseNotifierConfig};
use tempfile::NamedTempFile;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mock_releases_json() -> serde_json::Value {
    serde_json::json!([
        {
            "tag_name": "v2.0.0",
            "name": "Version 2.0.0",
            "body": "Latest stable release",
            "prerelease": false,
            "draft": false,
            "html_url": "https://github.com/test/repo/releases/tag/v2.0.0",
            "published_at": "2024-03-15T10:00:00Z"
        },
        {
            "tag_name": "v2.1.0-beta.1",
            "name": "Version 2.1.0 Beta 1",
            "body": "Latest prerelease",
            "prerelease": true,
            "draft": false,
            "html_url": "https://github.com/test/repo/releases/tag/v2.1.0-beta.1",
            "published_at": "2024-03-20T10:00:00Z"
        },
        {
            "tag_name": "v1.0.0",
            "name": "Version 1.0.0",
            "body": "First stable release",
            "prerelease": false,
            "draft": false,
            "html_url": "https://github.com/test/repo/releases/tag/v1.0.0",
            "published_at": "2024-01-01T10:00:00Z"
        },
        {
            "tag_name": "v3.0.0-draft",
            "name": "Draft Release",
            "body": "Draft release - should be filtered",
            "prerelease": false,
            "draft": true,
            "html_url": "https://github.com/test/repo/releases/tag/v3.0.0-draft",
            "published_at": "2024-04-01T10:00:00Z"
        }
    ])
}

#[tokio::test]
async fn test_get_latest_release_stable() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .and(header("Accept", "application/vnd.github+json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let release = notifier.get_latest_release(false).await.unwrap();

    assert!(release.is_some());
    let release = release.unwrap();
    assert_eq!(release.tag_name, "v2.0.0");
    assert!(!release.prerelease);
    assert!(!release.draft);
}

#[tokio::test]
async fn test_get_latest_release_including_prerelease() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let release = notifier.get_latest_release(true).await.unwrap();

    assert!(release.is_some());
    let release = release.unwrap();
    // v2.1.0-beta.1 is newer than v2.0.0
    assert_eq!(release.tag_name, "v2.1.0-beta.1");
}

#[tokio::test]
async fn test_get_latest_prerelease() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let release = notifier.get_latest_prerelease().await.unwrap();

    assert!(release.is_some());
    let release = release.unwrap();
    assert_eq!(release.tag_name, "v2.1.0-beta.1");
    assert!(release.prerelease);
}

#[tokio::test]
async fn test_draft_releases_are_filtered() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    // Even with include_prerelease=true, the draft release (v3.0.0-draft)
    // should not be returned as the latest, despite having the newest date
    let release = notifier.get_latest_release(true).await.unwrap();
    assert!(release.is_some());
    assert_ne!(release.unwrap().tag_name, "v3.0.0-draft");
}

#[tokio::test]
async fn test_check_version_update_available() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    // v1.0.0 is older than v2.0.0
    let result = notifier.check_version("v1.0.0", false).await.unwrap();
    assert!(result.update_available);
    assert!(result.latest_release.is_some());
    assert_eq!(result.latest_release.unwrap().tag_name, "v2.0.0");
}

#[tokio::test]
async fn test_check_version_no_update_when_on_latest() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    // v2.0.0 is the latest stable
    let result = notifier.check_version("v2.0.0", false).await.unwrap();
    assert!(!result.update_available);
}

#[tokio::test]
async fn test_check_version_handles_missing_v_prefix() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    // Without 'v' prefix should still work
    let result = notifier.check_version("1.0.0", false).await.unwrap();
    assert!(result.update_available);
}

#[tokio::test]
async fn test_check_version_unknown_version_returns_no_update() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    // Unknown version (dev build) - should assume user is on latest/dev
    let result = notifier.check_version("999.0.0", false).await.unwrap();
    assert!(!result.update_available);
}

#[tokio::test]
async fn test_caching_returns_cached_data() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .expect(1) // Should only be called once due to caching
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(3600000) // 1 hour cache
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    // First call - fetches from API
    let release1 = notifier.get_latest_release(false).await.unwrap();
    // Second call - should use cache
    let release2 = notifier.get_latest_release(false).await.unwrap();

    assert_eq!(release1.unwrap().tag_name, release2.unwrap().tag_name);
}

#[tokio::test]
async fn test_cache_disabled_when_interval_zero() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .expect(2) // Should be called twice (no caching)
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0) // Disable cache
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    notifier.get_latest_release(false).await.unwrap();
    notifier.get_latest_release(false).await.unwrap();
}

#[tokio::test]
async fn test_clear_cache() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .expect(2) // Called twice - before and after cache clear
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(3600000)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();

    notifier.get_latest_release(false).await.unwrap();
    notifier.clear_cache();
    notifier.get_latest_release(false).await.unwrap();
}

#[tokio::test]
async fn test_disk_cache_persistence() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .expect(1) // Only called once - second notifier loads from disk
        .mount(&mock_server)
        .await;

    let cache_file = NamedTempFile::new().unwrap();
    let cache_path = cache_file.path().to_str().unwrap().to_string();

    // First notifier - fetches and saves to disk
    {
        let config = ReleaseNotifierConfig::new("test/repo")
            .check_interval(3600000)
            .cache_file_path(&cache_path)
            .base_url(mock_server.uri());

        let notifier = ReleaseNotifier::new(config).unwrap();
        notifier.get_latest_release(false).await.unwrap();
    }

    // Second notifier - should load from disk cache
    {
        let config = ReleaseNotifierConfig::new("test/repo")
            .check_interval(3600000)
            .cache_file_path(&cache_path)
            .base_url(mock_server.uri());

        let notifier = ReleaseNotifier::new(config).unwrap();
        let release = notifier.get_latest_release(false).await.unwrap();
        assert!(release.is_some());
        assert_eq!(release.unwrap().tag_name, "v2.0.0");
    }
}

#[tokio::test]
async fn test_api_error_handling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let result = notifier.get_latest_release(false).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_releases() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let release = notifier.get_latest_release(false).await.unwrap();

    assert!(release.is_none());
}

#[tokio::test]
async fn test_no_prereleases_available() {
    let mock_server = MockServer::start().await;

    // Only stable releases
    let releases = serde_json::json!([
        {
            "tag_name": "v1.0.0",
            "name": "Version 1.0.0",
            "body": "Stable release",
            "prerelease": false,
            "draft": false,
            "html_url": "https://github.com/test/repo/releases/tag/v1.0.0",
            "published_at": "2024-01-01T10:00:00Z"
        }
    ]);

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(releases))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let release = notifier.get_latest_prerelease().await.unwrap();

    assert!(release.is_none());
}

#[tokio::test]
async fn test_token_is_sent_in_header() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/test/repo/releases"))
        .and(header("Authorization", "Bearer test-token-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_releases_json()))
        .mount(&mock_server)
        .await;

    let config = ReleaseNotifierConfig::new("test/repo")
        .check_interval(0)
        .token("test-token-123")
        .base_url(mock_server.uri());

    let notifier = ReleaseNotifier::new(config).unwrap();
    let release = notifier.get_latest_release(false).await.unwrap();

    assert!(release.is_some());
}
