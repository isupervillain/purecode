use crate::classifier::{get_classifier, LineType};
use crate::language::Language;
use crate::stats::{FileStats, LangStats};
use glob::Pattern;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use walkdir::WalkDir;

pub fn analyze_files(
    include: &[String],
    exclude: &[String],
    reader: Option<Box<dyn BufRead>>, // For stdin support
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

    for entry in WalkDir::new(".").into_iter().flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        // Convert path to relative string for matching
        let path_str = path.to_string_lossy();
        let clean_path = if let Some(stripped) = path_str.strip_prefix("./") {
            stripped
        } else {
            &path_str
        };

        // Check excludes
        if exclude_patterns.iter().any(|p| p.matches(clean_path)) {
            continue;
        }

        // Check includes (at least one must match if we are strict, or default include is "**/*")
        if !include_patterns.iter().any(|p| p.matches(clean_path)) {
            continue;
        }

        if let Ok(fs) = process_file(path) {
            stats.push(fs);
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
