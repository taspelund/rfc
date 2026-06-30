use assert_cmd::Command;
use predicates::str::contains;

/// Register `HOME` to a temp dir so cache operations don't touch the real
/// cache, then run the closure with that env var set.
fn with_isolated_home(f: impl FnOnce()) {
    let tmp = tempfile::TempDir::new().unwrap();
    // Ensure the cache dir exists so CacheManager::new() doesn't fail.
    let cache_dir = tmp.path().join("Library").join("Caches").join("rfc");
    std::fs::create_dir_all(&cache_dir).unwrap();

    let orig_home = std::env::var_os("HOME");
    std::env::set_var("HOME", tmp.path());
    f();
    if let Some(h) = orig_home {
        std::env::set_var("HOME", h);
    } else {
        std::env::remove_var("HOME");
    }
}

#[test]
fn help_succeeds() {
    Command::cargo_bin("rfc")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Search, retrieve, and display"));
}

#[test]
fn no_args_fails() {
    Command::cargo_bin("rfc")
        .unwrap()
        .assert()
        .failure();
}

#[test]
fn search_no_query_fails() {
    Command::cargo_bin("rfc")
        .unwrap()
        .arg("search")
        .assert()
        .failure();
}

#[test]
fn cache_list_empty() {
    with_isolated_home(|| {
        Command::cargo_bin("rfc")
            .unwrap()
            .args(["cache", "list"])
            .assert()
            .success()
            .stdout(contains("Cache is empty"));
    });
}

#[test]
fn cache_info_shows_zero() {
    with_isolated_home(|| {
        Command::cargo_bin("rfc")
            .unwrap()
            .args(["cache", "info"])
            .assert()
            .success()
            .stdout(contains("Cached documents: 0"));
    });
}
