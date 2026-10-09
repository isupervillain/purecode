use crate::classifier::{get_classifier, LineType};
use crate::language::Language;
use crate::stats::{FileStats, LangStats};
use glob::Pattern;
use ignore::WalkBuilder;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn analyze_files(
    paths: &[String],
    include: &[String],
    exclude: &[String],
    reader: Option<Box<dyn BufRead>>,
) -> Result<Vec<FileStats>, std::io::Error> {
    let mut stats = Vec::new();

    // Process stdin if provided (assuming list of files)
    if let Some(r) = reader {
        for line in r.lines() {
            let path_str = line?;
            let path = Path::new(&path_str);
            if path.exists() {
                if let Ok(fs) = process_file(path) {
                    stats.push(fs);
                }
            } else {
                eprintln!("Warning: File not found: {}", path_str);
            }
        }
        return Ok(stats);
    }

    let exclude_patterns: Vec<Pattern> = exclude
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    let include_patterns: Vec<Pattern> = include
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    let cwd = std::env::current_dir()?.canonicalize()?;
    for root in paths {
        let root = Path::new(root).canonicalize().map_err(|e| {
            std::io::Error::new(e.kind(), format!("cannot read path '{root}': {e}"))
        })?;
        // Patterns are relative to the project (the working directory) or, for a root outside
        // it, to that root; directories above it (e.g. /builds/target/app) never match.
        let base = if root.starts_with(&cwd) { &cwd } else { &root };
        // Respects .gitignore/.ignore (so build output is skipped without being walked);
        // hidden files like .github/ are still analyzed.
        let walker = WalkBuilder::new(&root)
            .hidden(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build();
        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();
            let rel = match path.strip_prefix(base) {
                Ok(r) if !r.as_os_str().is_empty() => r,
                _ => Path::new(path.file_name().unwrap_or_default()), // root is the file itself
            };

            if exclude_patterns.iter().any(|p| p.matches_path(rel)) {
                continue;
            }

            if !include_patterns.iter().any(|p| p.matches_path(rel)) {
                continue;
            }

            let shown = if base == &cwd { rel } else { path };
            if let Ok(mut fs) = process_file(path) {
                fs.path = shown.to_string_lossy().into_owned();
                stats.push(fs);
            }
        }
    }

    Ok(stats)
}

fn process_file(path: &Path) -> Result<FileStats, std::io::Error> {
    let language = Language::from_path(path);

    // Use a separate check
    if is_binary(path)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Binary file",
        ));
    }

    let mut reader = BufReader::new(File::open(path)?);
    let mut classifier = get_classifier(language);
    let mut lang_stats = LangStats::default();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        // Latin-1 and other non-UTF-8 text is still code; decode it lossily instead of skipping.
        let decoded = String::from_utf8_lossy(&buf);
        let line = decoded.trim_end_matches(['\n', '\r']);
        lang_stats.total_added += 1; // Snapshot mode: everything is added
        match classifier.classify(line.trim_start_matches('\u{feff}')) {
            LineType::Pure => {
                lang_stats.pure_added += 1;
                lang_stats.code_words_added += line.split_whitespace().count() as i64;
            }
            LineType::Comment => lang_stats.comment_lines_added += 1,
            LineType::Docstring => lang_stats.docstring_lines_added += 1,
            LineType::Blank => lang_stats.blank_lines_added += 1,
        }
    }

    Ok(FileStats {
        path: path.to_string_lossy().to_string(),
        language: language.to_string(),
        lang_stats,
    })
}

fn is_binary(path: &Path) -> Result<bool, std::io::Error> {
    let mut file = File::open(path)?;
    let mut buffer = [0; 1024];
    use std::io::Read;
    let n = file.read(&mut buffer)?;
    if n == 0 {
        return Ok(false);
    } // Empty file is not binary
    if buffer[..n].contains(&0) {
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;

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
        let stats = analyze_files(
            &[root.to_string_lossy().into_owned()],
            &config.include,
            &config.exclude,
            None,
        )
        .unwrap();
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
        let err = analyze_files(&["/nonexistent/purecode".into()], &[], &[], None).unwrap_err();
        assert!(err.to_string().contains("/nonexistent/purecode"));
    }
}
