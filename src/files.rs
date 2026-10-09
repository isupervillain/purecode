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

    for root in paths {
        // Respects .gitignore/.ignore (so build output is skipped without being walked);
        // hidden files like .github/ are still analyzed.
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git")
            .build();
        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let path = entry.path();

            let path_str = path.to_string_lossy();
            let clean_path = if let Some(stripped) = path_str.strip_prefix("./") {
                stripped
            } else {
                &path_str
            };

            if exclude_patterns.iter().any(|p| p.matches(clean_path)) {
                continue;
            }

            if !include_patterns.iter().any(|p| p.matches(clean_path)) {
                continue;
            }

            if let Ok(fs) = process_file(path) {
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

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut classifier = get_classifier(language);
    let mut lang_stats = LangStats::default();

    for line_result in reader.lines() {
        match line_result {
            Ok(line) => {
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
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Read error",
                ))
            }
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
        let root = std::env::temp_dir().join(format!("purecode-walk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (path, body) in [
            (".gitignore", "build/\n"),
            ("build/gen.js", "x();\n"),
            ("src/a.js", "a();\n// c\n"),
            (".github/ci.yml", "on: push\n"),
            (".git/config", "[core]\n"),
            ("web/node_modules/m.js", "m();\n"),
            ("package-lock.json", "{}\n"),
        ] {
            let p = root.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }

        let config = Config::default();
        let stats = analyze_files(
            &[root.to_string_lossy().into_owned()],
            &config.include,
            &config.exclude,
            None,
        )
        .unwrap();
        fs::remove_dir_all(&root).unwrap();

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
        assert_eq!(found, [".github/ci.yml", ".gitignore", "src/a.js"]);
    }
}
