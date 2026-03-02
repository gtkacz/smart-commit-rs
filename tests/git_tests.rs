mod common;

use auto_commit_rs::git;
use serial_test::serial;

use crate::common::{commit_file, git_ok, git_stdout, write_file, DirGuard};

#[test]
fn compute_next_minor_tag_handles_valid_and_invalid_input() {
    assert_eq!(
        git::compute_next_minor_tag(None).expect("default version"),
        "0.1.0"
    );
    assert_eq!(
        git::compute_next_minor_tag(Some("1.4.9")).expect("increment"),
        "1.5.0"
    );
    assert!(git::compute_next_minor_tag(Some("v1.2.3")).is_err());
    assert!(git::compute_next_minor_tag(Some("1.2")).is_err());
}

#[test]
#[serial]
fn staged_diff_and_files_behave_for_empty_and_non_empty_index() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    assert!(git::get_staged_diff().is_err());
    assert!(git::list_staged_files().expect("list staged files").is_empty());

    write_file(&repo.path().join("src.txt"), "hello");
    git_ok(repo.path(), ["add", "src.txt"]);

    let files = git::list_staged_files().expect("list staged files");
    assert_eq!(files, vec!["src.txt".to_string()]);
    let diff = git::get_staged_diff().expect("staged diff");
    assert!(diff.contains("src.txt"));
}

#[test]
#[serial]
fn commit_and_undo_roundtrip() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    write_file(&repo.path().join("a.txt"), "1");
    git_ok(repo.path(), ["add", "a.txt"]);
    git::run_commit("test: add a", &[], true).expect("commit should succeed");
    write_file(&repo.path().join("a.txt"), "2");
    git_ok(repo.path(), ["add", "a.txt"]);
    git::run_commit("test: update a", &[], true).expect("second commit should succeed");
    git::ensure_head_exists().expect("head exists");

    assert_eq!(
        git_stdout(repo.path(), ["rev-list", "--count", "HEAD"]),
        "2".to_string()
    );

    git::undo_last_commit_soft(true).expect("undo should succeed");
    assert!(git::ensure_head_exists().is_ok());
    assert_eq!(
        git_stdout(repo.path(), ["diff", "--cached", "--name-only"]),
        "a.txt".to_string()
    );
}

#[test]
#[serial]
fn latest_tag_and_create_tag_work() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let _first = commit_file(repo.path(), "a.txt", "a", "feat: first");

    assert!(git::get_latest_tag().expect("latest tag").is_none());
    git::create_tag("0.1.0", true).expect("create tag");
    git::create_tag("0.2.0", true).expect("create tag");

    assert_eq!(
        git::get_latest_tag().expect("latest tag"),
        Some("0.2.0".to_string())
    );
}

#[test]
#[serial]
fn commit_and_range_diff_and_head_checks_work() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let first = commit_file(repo.path(), "a.txt", "one", "feat: first");
    let second = commit_file(repo.path(), "a.txt", "two", "feat: second");

    let commit_diff = git::get_commit_diff(&second).expect("commit diff");
    assert!(commit_diff.contains("-one"));
    assert!(commit_diff.contains("+two"));

    let range_diff = git::get_range_diff(&first, &second).expect("range diff");
    assert!(range_diff.contains("a.txt"));

    assert!(git::is_head_commit(&second).expect("head check"));
    assert!(!git::is_head_commit(&first).expect("head check"));
}

#[test]
#[serial]
fn upstream_and_pushed_detection_work() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let head = commit_file(repo.path(), "tracked.txt", "v1", "feat: tracked");
    assert!(!git::has_upstream_branch().expect("upstream check"));
    assert!(!git::is_head_pushed().expect("head pushed check"));
    assert!(!git::commit_is_pushed(&head).expect("commit pushed check"));

    let remote = tempfile::TempDir::new().expect("temp remote");
    git_ok(remote.path(), ["init", "--bare"]);
    let remote_url = remote.path().to_string_lossy().replace('\\', "/");
    git_ok(repo.path(), ["remote", "add", "origin", &remote_url]);
    git_ok(repo.path(), ["push", "-u", "origin", "HEAD"]);

    assert!(git::has_upstream_branch().expect("upstream check"));
    assert!(git::is_head_pushed().expect("head pushed check"));
    assert!(git::commit_is_pushed(&head).expect("commit pushed check"));
}

#[test]
#[serial]
fn rewrite_head_commit_message_amends_commit() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let original = commit_file(repo.path(), "msg.txt", "v1", "feat: old");
    git::rewrite_commit_message("HEAD", "feat: new", true).expect("rewrite head");
    let current = git_stdout(repo.path(), ["log", "-1", "--pretty=%s"]);
    let rewritten = git_stdout(repo.path(), ["rev-parse", "HEAD"]);

    assert_eq!(current, "feat: new");
    assert_ne!(rewritten, original);
}

#[test]
#[serial]
fn commit_is_merge_detects_merge_commits() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    // Create a commit on main
    let c1 = commit_file(repo.path(), "a.txt", "1", "c1");

    // Create a branch
    git_ok(repo.path(), ["checkout", "-b", "feature"]);
    let c2 = commit_file(repo.path(), "b.txt", "2", "c2");

    // Switch back to main (master in init_git_repo usually)
    // We'll just checkout c1 and create a new branch to be sure
    git_ok(repo.path(), ["checkout", &c1]);
    git_ok(repo.path(), ["checkout", "-b", "main_branch"]);
    let _c3 = commit_file(repo.path(), "c.txt", "3", "c3");

    // Merge feature into main_branch
    // We might need to configure user email/name which init_git_repo does.
    // We use --no-ff to force a merge commit
    git_ok(repo.path(), ["merge", "--no-ff", "feature", "-m", "merge commit"]);

    let merge_commit = git_stdout(repo.path(), ["rev-parse", "HEAD"]);

    assert!(git::commit_is_merge(&merge_commit).expect("check merge"));
    assert!(git::head_is_merge_commit().expect("check head merge"));
    assert!(!git::commit_is_merge(&c1).expect("check normal commit"));
    assert!(!git::commit_is_merge(&c2).expect("check normal commit"));
}

#[test]
#[serial]
fn ensure_ancestor_of_head_checks_ancestry() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    let c1 = commit_file(repo.path(), "a.txt", "1", "c1");
    let c2 = commit_file(repo.path(), "a.txt", "2", "c2");

    // c1 is ancestor of c2 (HEAD)
    assert!(git::ensure_ancestor_of_head(&c1).is_ok());
    assert!(git::ensure_ancestor_of_head(&c2).is_ok()); // HEAD is ancestor of HEAD

    // Create a detached commit or parallel branch
    git_ok(repo.path(), ["checkout", "--detach", &c1]);
    let c3 = commit_file(repo.path(), "a.txt", "3", "c3");

    // Switch back to c2
    git_ok(repo.path(), ["checkout", &c2]);

    // c3 is NOT ancestor of c2
    assert!(git::ensure_ancestor_of_head(&c3).is_err());
}

#[test]
#[serial]
fn reword_non_head_commit_works() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    let _c1 = commit_file(repo.path(), "a.txt", "1", "c1");
    let c2 = commit_file(repo.path(), "a.txt", "2", "old message");
    let _c3 = commit_file(repo.path(), "a.txt", "3", "c3");

    // We want to reword c2.
    // git::rewrite_commit_message handles HEAD (amend) and non-HEAD (rebase).
    // It calls reword_non_head_commit for c2.

    git::rewrite_commit_message(&c2, "new message", true).expect("rewrite should succeed");

    // Verify c2 message is changed. 
    // Since history changed, we look at HEAD~1
    let msg = git_stdout(repo.path(), ["log", "-1", "--pretty=%s", "HEAD~1"]);
    assert_eq!(msg, "new message");

    // Verify content is still there (c3 content at HEAD)
    let content = std::fs::read_to_string(repo.path().join("a.txt")).expect("read file");
    assert_eq!(content, "3");
}

#[test]
fn filter_diff_by_globs_excludes_matching_files() {
    let diff = r#"diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("Hello");
 }
diff --git a/package.json b/package.json
index aaaaaaa..bbbbbbb 100644
--- a/package.json
+++ b/package.json
@@ -1,3 +1,4 @@
 {
+  "name": "test"
 }
diff --git a/data.csv b/data.csv
index ccccccc..ddddddd 100644
--- a/data.csv
+++ b/data.csv
@@ -1,2 +1,3 @@
 a,b,c
+1,2,3
"#;

    // Exclude *.json and *.csv
    let patterns = vec!["*.json".to_string(), "*.csv".to_string()];
    let filtered = git::filter_diff_by_globs(diff, &patterns);

    // Should contain main.rs changes
    assert!(filtered.contains("src/main.rs"));
    assert!(filtered.contains("println!"));

    // Should NOT contain package.json or data.csv
    assert!(!filtered.contains("package.json"));
    assert!(!filtered.contains("data.csv"));
}

#[test]
fn filter_diff_by_globs_returns_full_diff_when_no_patterns() {
    let diff = "diff --git a/foo.json b/foo.json\n+test\n";
    let filtered = git::filter_diff_by_globs(diff, &[]);
    assert_eq!(filtered, diff);
}

#[test]
fn filter_diff_by_globs_handles_nested_paths() {
    let diff = r#"diff --git a/deep/nested/config.json b/deep/nested/config.json
--- a/deep/nested/config.json
+++ b/deep/nested/config.json
@@ -1 +1,2 @@
+{"key": "value"}
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1,2 @@
+// code
"#;

    let patterns = vec!["*.json".to_string()];
    let filtered = git::filter_diff_by_globs(diff, &patterns);

    // Should contain lib.rs but not the nested json file
    assert!(filtered.contains("src/lib.rs"));
    assert!(!filtered.contains("config.json"));
}

#[test]
fn filter_diff_by_globs_invalid_pattern_ignored() {
    let diff = "diff --git a/test.rs b/test.rs\n+code\n";
    // Invalid glob pattern should be silently ignored
    let patterns = vec!["[invalid".to_string(), "*.rs".to_string()];
    let filtered = git::filter_diff_by_globs(diff, &patterns);
    // Since *.rs matches, the diff should be filtered out
    assert!(!filtered.contains("test.rs"));
}

#[test]
fn filter_diff_by_globs_multiple_files_partial_match() {
    let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1 +1,2 @@
+// main
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1 +1,2 @@
+lock content
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1,2 @@
+// lib
"#;

    let patterns = vec!["*.lock".to_string()];
    let filtered = git::filter_diff_by_globs(diff, &patterns);

    // Should keep .rs files, exclude .lock
    assert!(filtered.contains("main.rs"));
    assert!(filtered.contains("lib.rs"));
    assert!(!filtered.contains("Cargo.lock"));
}

#[test]
#[serial]
fn find_repo_root_in_subdirectory() {
    let repo = common::init_git_repo();
    let subdir = repo.path().join("src").join("nested");
    std::fs::create_dir_all(&subdir).expect("create subdir");
    let _cwd = DirGuard::enter(&subdir);

    let root = git::find_repo_root().expect("find repo root");
    assert_eq!(
        std::path::Path::new(&root).canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );
}

#[test]
#[serial]
fn ensure_head_exists_on_empty_repo_fails() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    // Empty repo has no HEAD
    assert!(git::ensure_head_exists().is_err());
}

#[test]
#[serial]
fn ensure_commit_exists_invalid_ref() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    commit_file(repo.path(), "a.txt", "content", "initial");

    let result = git::ensure_commit_exists("nonexistent_ref_12345");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
#[serial]
fn get_commit_diff_invalid_commit() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    commit_file(repo.path(), "a.txt", "content", "initial");

    let result = git::get_commit_diff("nonexistent_commit");
    assert!(result.is_err());
}

#[test]
#[serial]
fn get_range_diff_invalid_commits() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let c1 = commit_file(repo.path(), "a.txt", "content", "initial");

    // One valid, one invalid
    let result = git::get_range_diff(&c1, "nonexistent");
    assert!(result.is_err());

    // Both invalid
    let result = git::get_range_diff("bad1", "bad2");
    assert!(result.is_err());
}

#[test]
#[serial]
fn run_commit_with_extra_args() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    write_file(&repo.path().join("test.txt"), "content");
    git_ok(repo.path(), ["add", "test.txt"]);

    // Use --no-gpg-sign or similar benign extra arg
    let extra_args = vec!["--no-gpg-sign".to_string()];
    git::run_commit("test commit", &extra_args, true).expect("commit with extra args");

    let msg = git_stdout(repo.path(), ["log", "-1", "--pretty=%s"]);
    assert_eq!(msg, "test commit");
}

#[test]
#[serial]
fn run_commit_fails_with_nothing_staged() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    // Need at least one commit first
    commit_file(repo.path(), "initial.txt", "content", "first");

    // Now try to commit with nothing staged
    let result = git::run_commit("empty commit", &[], true);
    assert!(result.is_err());
}

#[test]
#[serial]
fn rewrite_merge_commit_fails() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    // Create merge commit
    let c1 = commit_file(repo.path(), "a.txt", "1", "c1");
    git_ok(repo.path(), ["checkout", "-b", "feature"]);
    commit_file(repo.path(), "b.txt", "2", "c2");
    git_ok(repo.path(), ["checkout", &c1]);
    git_ok(repo.path(), ["checkout", "-b", "main_branch"]);
    commit_file(repo.path(), "c.txt", "3", "c3");
    git_ok(
        repo.path(),
        ["merge", "--no-ff", "feature", "-m", "merge commit"],
    );

    let merge = git_stdout(repo.path(), ["rev-parse", "HEAD"]);

    // Create another commit after merge
    commit_file(repo.path(), "d.txt", "4", "d4");

    // Try to rewrite the merge commit (now it's HEAD~1, not HEAD)
    let result = git::rewrite_commit_message(&merge, "new merge message", true);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("merge"));
}

#[test]
#[serial]
fn get_commit_diff_empty_commit() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    // Create initial commit
    commit_file(repo.path(), "a.txt", "content", "initial");

    // Try to create an empty commit (requires --allow-empty)
    git_ok(repo.path(), ["commit", "--allow-empty", "-m", "empty"]);
    let empty_commit = git_stdout(repo.path(), ["rev-parse", "HEAD"]);

    // get_commit_diff should fail for empty commit
    let result = git::get_commit_diff(&empty_commit);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no diff"));
}

#[test]
#[serial]
fn get_range_diff_same_commit() {
    let repo = common::init_git_repo();
    let _cwd = DirGuard::enter(repo.path());

    let c1 = commit_file(repo.path(), "a.txt", "content", "initial");

    // Same commit should produce empty diff
    let result = git::get_range_diff(&c1, &c1);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No diff"));
}
