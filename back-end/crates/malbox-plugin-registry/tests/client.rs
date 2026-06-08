use malbox_plugin_registry::client::Cache;
use tempfile::TempDir;

#[test]
fn cache_write_and_read() {
    let tmp = TempDir::new().unwrap();
    let cache = Cache::new(tmp.path().to_path_buf());

    cache
        .write("index.json", b"cached content", Some("etag-abc"))
        .unwrap();

    let (content, etag) = cache.read("index.json").unwrap().unwrap();
    assert_eq!(content, b"cached content");
    assert_eq!(etag.as_deref(), Some("etag-abc"));
}

#[test]
fn cache_read_missing_returns_none() {
    let tmp = TempDir::new().unwrap();
    let cache = Cache::new(tmp.path().to_path_buf());
    assert!(cache.read("nonexistent.json").unwrap().is_none());
}

#[test]
fn cache_write_without_etag() {
    let tmp = TempDir::new().unwrap();
    let cache = Cache::new(tmp.path().to_path_buf());

    cache.write("data.json", b"content", None).unwrap();

    let (content, etag) = cache.read("data.json").unwrap().unwrap();
    assert_eq!(content, b"content");
    assert!(etag.is_none());
}

#[test]
fn cache_nested_key() {
    let tmp = TempDir::new().unwrap();
    let cache = Cache::new(tmp.path().to_path_buf());

    cache
        .write(
            "plugins/yara-scanner.json",
            b"plugin data",
            Some("etag-xyz"),
        )
        .unwrap();

    let (content, etag) = cache.read("plugins/yara-scanner.json").unwrap().unwrap();
    assert_eq!(content, b"plugin data");
    assert_eq!(etag.as_deref(), Some("etag-xyz"));
}
