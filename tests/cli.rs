//! End-to-end tests: real git repositories, the real binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A throwaway git repository, removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "purecode-cli-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let repo = Self { dir };
        repo.git(&["init", "-q"]);
        // Byte-exact files on every platform (Git for Windows defaults to autocrlf=true).
        repo.git(&["config", "core.autocrlf", "false"]);
        repo
    }

    fn git(&self, args: &[&str]) -> Output {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        out
    }

    fn write(&self, path: &str, content: &[u8]) {
        let p = self.dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }

    fn purecode(&self, args: &[&str]) -> Output {
        self.purecode_in(&self.dir, args)
    }

    fn purecode_in(&self, dir: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_purecode"))
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The `summary` object of a JSON report.
fn summary(out: &Output) -> serde_json::Value {
    let report: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("invalid JSON ({e}): {out:?}"));
    report["summary"].clone()
}

/// A repository with a `base` branch and one commit on top of it.
fn repo_with_change(before: &[(&str, &[u8])], after: &[(&str, &[u8])]) -> Repo {
    let repo = Repo::new();
    for (path, content) in before {
        repo.write(path, content);
    }
    repo.commit("base");
    repo.git(&["branch", "base"]);
    for (path, content) in after {
        repo.write(path, content);
    }
    repo.commit("change");
    repo
}

#[test]
fn line_added_inside_existing_docstring_is_a_docstring() {
    let repo = repo_with_change(
        &[("a.py", b"def f():\n    \"\"\"Summary.\n\n    \"\"\"\n")],
        &[(
            "a.py",
            b"def f():\n    \"\"\"Summary.\n\n    More detail.\n    \"\"\"\n",
        )],
    );
    let out = repo.purecode(&["diff", "--base", "base", "--format", "json"]);
    assert!(out.status.success(), "{out:?}");
    let s = summary(&out);
    assert_eq!(s["docstring_lines_added"], 1);
    assert_eq!(s["pure_added"], 0);
}

#[test]
fn crlf_file_keeps_whole_file_context() {
    let repo = repo_with_change(
        &[("a.rs", b"/*\r\n * a\r\n */\r\nfn x() {}\r\n")],
        &[(
            "a.rs",
            b"/*\r\n * a\r\n plain words\r\n */\r\nfn x() {}\r\n",
        )],
    );
    let s = summary(&repo.purecode(&["diff", "--base", "base", "--format", "json"]));
    assert_eq!(s["comment_lines_added"], 1);
    assert_eq!(s["pure_added"], 0);
}

#[test]
fn rename_with_edit_reports_new_path() {
    let repo = Repo::new();
    repo.write("old.py", b"x = 1\ny = 2\nz = 3\n");
    repo.commit("base");
    repo.git(&["branch", "base"]);
    repo.git(&["mv", "old.py", "new.py"]);
    repo.write("new.py", b"x = 1\ny = 2\nz = 3\nw = 4\n");
    repo.commit("rename");
    let out = repo.purecode(&["diff", "--base", "base", "--per-file", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let files = report["file_stats"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "new.py");
    assert_eq!(files[0]["lang_stats"]["pure_added"], 1);
    assert_eq!(files[0]["lang_stats"]["total_removed"], 0);
}

#[test]
fn staged_changes_on_an_unborn_branch() {
    let repo = Repo::new();
    repo.write("a.py", b"# comment\nx = 1\n");
    repo.git(&["add", "a.py"]);
    let s = summary(&repo.purecode(&["diff", "--staged", "--format", "json"]));
    assert_eq!(s["pure_added"], 1);
    assert_eq!(s["comment_lines_added"], 1);
}

#[test]
fn ref_that_looks_like_an_option_is_rejected() {
    let repo = repo_with_change(&[("a.py", b"x = 1\n")], &[("a.py", b"x = 2\n")]);
    let out = repo.purecode(&["--base=--output=pwned"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(!repo.dir.join("pwned...HEAD").exists());

    repo.write(".purecode.toml", b"base = \"--output=pwned\"\n");
    let out = repo.purecode(&[]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(std::fs::read_dir(&repo.dir).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("pwned")));
}

#[test]
fn user_git_config_cannot_change_the_result() {
    let repo = repo_with_change(&[("a.py", b"x = 1\n")], &[("a.py", b"x = 2\ny = 3\n")]);
    repo.git(&["config", "diff.external", "false"]);
    repo.git(&["config", "diff.noprefix", "true"]);
    let s = summary(&repo.purecode(&["diff", "--base", "base", "--format", "json"]));
    assert_eq!(s["pure_added"], 2);
    assert_eq!(s["pure_removed"], 1);
}

#[test]
fn json_ci_threshold_failure_keeps_stdout_valid_json() {
    let repo = repo_with_change(&[("a.py", b"x = 1\n")], &[("a.py", b"x = 1\n# note\n")]);
    let out = repo.purecode(&[
        "diff",
        "--base",
        "base",
        "--format",
        "json",
        "--ci",
        "--min-pure-lines",
        "5",
    ]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    summary(&out); // parses
    assert!(String::from_utf8_lossy(&out.stderr).contains("less than minimum"));
}

#[test]
fn config_in_repository_root_applies_from_subdirectory() {
    let repo = repo_with_change(
        &[("sub/a.py", b"x = 1\n")],
        &[("sub/a.py", b"x = 1\n# note\n")],
    );
    repo.write(".purecode.toml", b"max_noise_ratio = 0.0\n");
    let out = repo.purecode_in(&repo.dir.join("sub"), &["diff", "--base", "base"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
}

#[test]
fn default_excludes_apply_to_diffs() {
    let repo = repo_with_change(
        &[("a.py", b"x = 1\n")],
        &[
            ("a.py", b"x = 2\n"),
            ("yarn.lock", b"a\nb\n"),
            ("dist/app.js", b"x();\n"),
        ],
    );
    let out = repo.purecode(&["diff", "--base", "base", "--per-file", "--format", "json"]);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let paths: Vec<&str> = report["file_stats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["a.py"]);
}

#[test]
fn invalid_config_is_an_error() {
    let repo = repo_with_change(&[("a.py", b"x = 1\n")], &[("a.py", b"x = 2\n")]);
    repo.write(".purecode.toml", b"max_noise_ratoi = 0.5\n");
    let out = repo.purecode(&["diff", "--base", "base"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("max_noise_ratoi"));
}

#[test]
fn outside_a_repository_reports_gits_reason() {
    let dir = std::env::temp_dir().join(format!("purecode-norepo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_purecode"))
        .current_dir(&dir)
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not a git repository"), "{stderr}");
    assert_eq!(stderr.trim().lines().count(), 1, "{stderr}");
}

#[cfg(unix)]
#[test]
fn control_characters_in_file_names_are_escaped() {
    let repo = repo_with_change(&[("a.py", b"x\n")], &[("evil\x1b[2J.py", b"x = 1\n")]);
    let out = repo.purecode(&["diff", "--base", "base", "--per-file", "--format", "plain"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('\x1b'), "{stdout}");
    assert!(stdout.contains("evil\\u{1b}[2J.py"), "{stdout}");
}
