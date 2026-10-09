use crate::classifier::{get_classifier, text_lines, LineType};
use crate::config::PathFilter;
use crate::language::Language;
use crate::report::printable;
use crate::stats::{FileStats, LangStats};
use ignore::WalkBuilder;
use std::io::{self, BufRead};
use std::path::Path;

/// Analyzes every selected text file under `paths`, or each path listed on `reader`.
/// `filter` patterns are relative to `project_root` (or to a scanned path outside it).
pub fn analyze_files(
    paths: &[String],
    filter: &PathFilter,
    project_root: &Path,
    reader: Option<Box<dyn BufRead>>,
) -> io::Result<Vec<FileStats>> {
    let mut stats = Vec::new();

    // Explicit file list on stdin: analyzed as given, without include/exclude.
    if let Some(r) = reader {
        for line in r.lines() {
            let line = line?;
            let path = Path::new(line.trim());
            if line.trim().is_empty() {
                continue;
            }
            if !path.is_file() {
                eprintln!(
                    "Warning: not a file, skipped: {}",
                    printable(&path.to_string_lossy())
                );
                continue;
            }
            if let Some(fs) = analyze_or_warn(path, path) {
                stats.push(fs);
            }
        }
        return Ok(stats);
    }

    let project_root = project_root.canonicalize()?;
    for root in paths {
        let root = Path::new(root)
            .canonicalize()
            .map_err(|e| io::Error::new(e.kind(), format!("cannot read path '{root}': {e}")))?;
        // Directories above the scanned project (e.g. /builds/target/app) never match patterns.
        let base = if root.starts_with(&project_root) {
            &project_root
        } else {
            &root
        };
        // Respects .gitignore/.ignore (so build output is skipped without being walked);
        // hidden files like .github/ are still analyzed.
        let walker = WalkBuilder::new(&root)
            .hidden(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build();
        for entry in walker {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    eprintln!("Warning: skipped: {}", printable(&e.to_string()));
                    continue;
                }
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let rel = match path.strip_prefix(base) {
                Ok(r) if !r.as_os_str().is_empty() => r,
                _ => Path::new(path.file_name().unwrap_or_default()), // root is the file itself
            };
            if !filter.selects(rel) {
                continue;
            }
            let shown = if base == &project_root { rel } else { path };
            if let Some(fs) = analyze_or_warn(path, shown) {
                stats.push(fs);
            }
        }
    }

    Ok(stats)
}

/// Stats for a text file, reported under `shown`; binary files are skipped, unreadable ones
/// are skipped with a warning.
fn analyze_or_warn(path: &Path, shown: &Path) -> Option<FileStats> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!(
                "Warning: cannot read {}: {e}",
                printable(&path.to_string_lossy())
            );
            return None;
        }
    };
    if bytes[..bytes.len().min(1024)].contains(&0) {
        return None; // binary
    }

    let language = Language::from_path(path);
    let mut classifier = get_classifier(language);
    let mut lang_stats = LangStats::default();
    for line in text_lines(&bytes) {
        lang_stats.total_added += 1; // Snapshot mode: everything is added
        match classifier.classify(&line) {
            LineType::Pure => {
                lang_stats.pure_added += 1;
                lang_stats.code_words_added += line.split_whitespace().count() as i64;
            }
            LineType::Comment => lang_stats.comment_lines_added += 1,
            LineType::Docstring => lang_stats.docstring_lines_added += 1,
            LineType::Blank => lang_stats.blank_lines_added += 1,
        }
    }

    Some(FileStats {
        path: shown.to_string_lossy().into_owned(),
        language: language.to_string(),
        lang_stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn walk_respects_gitignore_and_default_excludes() {
        // The root sits under a directory named `target`: ancestors must not trigger excludes.
        let root = std::env::temp_dir()
            .join(format!("purecode-walk-{}", std::process::id()))
            .join("target")
            .join("proj");
        let _ = fs::remove_dir_all(root.parent().unwrap().parent().unwrap());
        let files: [(&str, &[u8]); 8] = [
            (".gitignore", b"build/\n"),
            ("build/gen.js", b"x();\n"),
            ("src/a.js", b"a();\n// c\n"),
            (".github/ci.yml", b"on: push\n"),
            (".git/config", b"[core]\n"),
            ("web/node_modules/m.js", b"m();\n"),
            ("package-lock.json", b"{}\n"),
            ("latin1.py", b"x = 'caf\xe9'\r\n# c\r\n"), // not valid UTF-8
        ];
        for (path, body) in files {
            let p = root.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }

        let root_canonical = root.canonicalize().unwrap();
        let config = Config::default();
        let filter = PathFilter::new(&config.include, &config.exclude).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let stats =
            analyze_files(&[root.to_string_lossy().into_owned()], &filter, &cwd, None).unwrap();
        fs::remove_dir_all(root.parent().unwrap().parent().unwrap()).unwrap();
        let root = root_canonical;

        let mut found: Vec<String> = stats
            .iter()
            .map(|f| {
                Path::new(&f.path)
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        found.sort();
        assert_eq!(
            found,
            [".github/ci.yml", ".gitignore", "latin1.py", "src/a.js"]
        );
        let latin1 = stats
            .iter()
            .find(|f| f.path.ends_with("latin1.py"))
            .unwrap();
        assert_eq!(
            (
                latin1.lang_stats.pure_added,
                latin1.lang_stats.comment_lines_added
            ),
            (1, 1)
        );
    }

    #[test]
    fn missing_root_is_an_error() {
        let filter = PathFilter::new(&[], &[]).unwrap();
        let err = analyze_files(
            &["/nonexistent/purecode".into()],
            &filter,
            &PathBuf::from("."),
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("/nonexistent/purecode"));
    }
}
