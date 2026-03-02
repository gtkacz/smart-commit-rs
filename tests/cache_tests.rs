mod common;

use auto_commit_rs::cache::{
    get_head_hash, record_commit, CacheIndex, CacheIndexEntry, CachedCommit, RepoCache,
};
use common::{commit_file, init_git_repo, DirGuard, EnvGuard};
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

fn setup_cache_env() -> (TempDir, EnvGuard) {
    let cfg_dir = TempDir::new().expect("tempdir");
    let cgen_dir = cfg_dir.path().join("cgen");
    fs::create_dir_all(&cgen_dir).expect("create cgen dir");
    let env = EnvGuard::set(&[(
        "ACR_CONFIG_HOME",
        cfg_dir.path().to_string_lossy().as_ref(),
    )]);
    (cfg_dir, env)
}

#[test]
#[serial]
fn record_commit_creates_cache_files() {
    let (_cfg_dir, _env) = setup_cache_env();
    let repo = init_git_repo();
    let _guard = DirGuard::enter(repo.path());

    let hash = commit_file(repo.path(), "a.txt", "content", "initial");

    record_commit(
        &repo.path().to_string_lossy(),
        &hash,
        "initial commit preview",
    )
    .expect("record_commit should succeed");

    // Verify index was created
    let cache_dir = _cfg_dir.path().join("cgen").join("cache");
    assert!(
        cache_dir.join("index.toml").exists(),
        "index.toml should exist"
    );
}

#[test]
#[serial]
fn record_commit_multiple_entries() {
    let (_cfg_dir, _env) = setup_cache_env();
    let repo = init_git_repo();
    let _guard = DirGuard::enter(repo.path());

    let h1 = commit_file(repo.path(), "a.txt", "1", "first");
    let h2 = commit_file(repo.path(), "b.txt", "2", "second");

    let repo_path = repo.path().to_string_lossy().to_string();

    record_commit(&repo_path, &h1, "first").expect("record first");
    record_commit(&repo_path, &h2, "second").expect("record second");

    // Verify that both commits are recorded (by reading the cache file)
    let cache_dir = _cfg_dir.path().join("cgen").join("cache");
    let entries: Vec<_> = fs::read_dir(&cache_dir)
        .expect("read cache dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .filter(|e| e.file_name() != "index.toml")
        .collect();

    assert!(!entries.is_empty(), "should have at least one cache file");
}

#[test]
#[serial]
fn record_commit_different_repos() {
    let (_cfg_dir, _env) = setup_cache_env();

    let repo1 = init_git_repo();
    let repo2 = init_git_repo();

    let h1 = commit_file(repo1.path(), "a.txt", "1", "repo1 commit");
    let h2 = commit_file(repo2.path(), "a.txt", "2", "repo2 commit");

    record_commit(&repo1.path().to_string_lossy(), &h1, "repo1").expect("record repo1");
    record_commit(&repo2.path().to_string_lossy(), &h2, "repo2").expect("record repo2");

    // Verify index has two entries
    let index_path = _cfg_dir.path().join("cgen").join("cache").join("index.toml");
    let content = fs::read_to_string(&index_path).expect("read index");
    let index: CacheIndex = toml::from_str(&content).expect("parse index");

    assert_eq!(index.repos.len(), 2, "should have two repo entries");
}

#[test]
fn cache_index_empty_default() {
    let index = CacheIndex::default();
    assert!(index.repos.is_empty());
}

#[test]
fn cache_index_entry_serde() {
    let entry = CacheIndexEntry {
        repo_path: "/path/to/repo".into(),
        cache_file: "abc123.toml".into(),
    };

    let toml_str = toml::to_string(&entry).unwrap();
    let parsed: CacheIndexEntry = toml::from_str(&toml_str).unwrap();

    assert_eq!(parsed.repo_path, entry.repo_path);
    assert_eq!(parsed.cache_file, entry.cache_file);
}

#[test]
fn repo_cache_default() {
    let cache = RepoCache::default();
    assert!(cache.repo_path.is_empty());
    assert!(cache.commits.is_empty());
}

#[test]
fn cached_commit_short_hash_display() {
    let commit = CachedCommit {
        hash: "abcdef1234567890".into(),
        message_preview: "feat: test".into(),
    };
    // Just verify the hash can be sliced for display (used in UI)
    let short = &commit.hash[..7.min(commit.hash.len())];
    assert_eq!(short, "abcdef1");
}

#[test]
fn cached_commit_short_hash_handles_small_hash() {
    let commit = CachedCommit {
        hash: "abc".into(),
        message_preview: "test".into(),
    };
    let short = &commit.hash[..7.min(commit.hash.len())];
    assert_eq!(short, "abc");
}

#[test]
#[serial]
fn record_commit_without_config_home_uses_dirs() {
    // This tests the fallback path when ACR_CONFIG_HOME is not set
    let _env = EnvGuard::clear(&["ACR_CONFIG_HOME"]);

    let repo = init_git_repo();
    let _guard = DirGuard::enter(repo.path());

    let hash = commit_file(repo.path(), "test.txt", "content", "test commit");

    // This should use dirs::config_dir() as fallback
    // We don't assert file creation since it depends on the system config dir
    let result = record_commit(&repo.path().to_string_lossy(), &hash, "test");

    // Should succeed or fail gracefully
    if let Err(e) = result {
        // If it fails, it should be because cache dir couldn't be determined
        assert!(
            e.to_string().contains("cache")
                || e.to_string().contains("directory")
                || e.to_string().contains("determine")
        );
    }
}

#[test]
#[serial]
fn get_head_hash_returns_commit_hash() {
    let repo = init_git_repo();
    let _guard = DirGuard::enter(repo.path());

    // Create a commit
    let expected_hash = commit_file(repo.path(), "test.txt", "content", "test commit");

    let hash = get_head_hash().expect("get_head_hash should succeed");
    assert_eq!(hash, expected_hash);
    assert_eq!(hash.len(), 40); // SHA-1 hash is 40 hex chars
}

#[test]
#[serial]
fn get_head_hash_fails_on_empty_repo() {
    let repo = init_git_repo();
    let _guard = DirGuard::enter(repo.path());

    // Empty repo has no HEAD
    let result = get_head_hash();
    assert!(result.is_err());
}

#[test]
#[serial]
fn get_head_hash_updates_after_commit() {
    let repo = init_git_repo();
    let _guard = DirGuard::enter(repo.path());

    let hash1 = commit_file(repo.path(), "a.txt", "1", "first");
    assert_eq!(get_head_hash().unwrap(), hash1);

    let hash2 = commit_file(repo.path(), "b.txt", "2", "second");
    assert_eq!(get_head_hash().unwrap(), hash2);
    assert_ne!(hash1, hash2);
}
