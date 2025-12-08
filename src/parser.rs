use crate::classifier::{get_classifier, LineType};
use crate::language::Language;
use crate::stats::{FileStats, LangStats};
use std::path::Path;

/// Parses a unified diff from the reader and updates statistics.
pub fn parse_diff<R: std::io::BufRead>(
    reader: R,
    stats: &mut Vec<FileStats>,
) -> Result<(), std::io::Error> {
    let mut current_file_stats: Option<FileStats> = None;
    let mut classifier = get_classifier(Language::Other);
    let mut is_binary_diff = false;
    let mut context_warning_printed = false;

    for line_result in reader.lines() {
        let line = line_result?;

        // Detect binary files diff
        if line.starts_with("Binary files") && line.contains("differ") {
            // "Binary files a/foo and b/foo differ"
            // We should skip this file.
            // If we already started tracking it (unlikely if this is the first line about it), clear it.
            current_file_stats = None;
            is_binary_diff = true;
            continue;
        }

        if line.starts_with("--- ") {
            // Save previous
            if let Some(file_stats) = current_file_stats.take() {
                if !is_binary_diff
                    && (file_stats.lang_stats.total_added > 0
                        || file_stats.lang_stats.total_removed > 0)
                {
                    stats.push(file_stats);
                }
            }
            is_binary_diff = false;

            let path_part = line.trim_start_matches("--- ").trim();
            if path_part == "/dev/null" {
                current_file_stats = None;
                continue;
            }

            let clean_path = if let Some(stripped) = path_part.strip_prefix("a/") {
                stripped
            } else {
                path_part
            };

            let language = Language::from_path(Path::new(clean_path));
            classifier = get_classifier(language);
            current_file_stats = Some(FileStats {
                path: clean_path.to_string(),
                language: language.to_string(),
                lang_stats: LangStats::default(),
            });
            continue;
        }

        if line.starts_with("+++ ") {
            let path_part = line.trim_start_matches("+++ ").trim();
            if path_part == "/dev/null" {
                continue;
            }

            let clean_path = if let Some(stripped) = path_part.strip_prefix("b/") {
                stripped
            } else {
                path_part
            };

            if let Some(fs) = &mut current_file_stats {
                if fs.path != clean_path {
                    let language = Language::from_path(Path::new(clean_path));
                    classifier = get_classifier(language);
                    fs.path = clean_path.to_string();
                    fs.language = language.to_string();
                }
            } else {
                let language = Language::from_path(Path::new(clean_path));
                classifier = get_classifier(language);
                current_file_stats = Some(FileStats {
                    path: clean_path.to_string(),
                    language: language.to_string(),
                    lang_stats: LangStats::default(),
                });
            }
            continue;
        }

        // Hunk header
        if line.starts_with("@@") {
            // Reset classifier state for new hunk because hunks are disjoint
            // and carrying state (like in_comment) across hunks is dangerous.
            // We re-initialize the classifier for the current language.
            if let Some(fs) = &current_file_stats {
                let lang = Language::from_path(Path::new(&fs.path));
                classifier = get_classifier(lang);
            }
            continue;
        }

        // Ignore metadata
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
        {
            continue;
        }

        if is_binary_diff {
            continue;
        }

        let file_stats = match &mut current_file_stats {
            Some(fs) => fs,
            None => continue,
        };

        if line.starts_with('+') && !line.starts_with("+++") {
            let content = &line[1..];
            let stat = &mut file_stats.lang_stats;
            stat.total_added += 1;

            match classifier.classify(content) {
                LineType::Pure => {
                    stat.pure_added += 1;
                    stat.code_words_added += count_words(content) as i64;
                }
                LineType::Comment => stat.comment_lines_added += 1,
                LineType::Docstring => stat.docstring_lines_added += 1,
                LineType::Blank => stat.blank_lines_added += 1,
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            let content = &line[1..];
            let stat = &mut file_stats.lang_stats;
            stat.total_removed += 1;

            match classifier.classify(content) {
                LineType::Pure => {
                    stat.pure_removed += 1;
                    stat.code_words_removed += count_words(content) as i64;
                }
                LineType::Comment => stat.comment_lines_removed += 1,
                LineType::Docstring => stat.docstring_lines_removed += 1,
                LineType::Blank => stat.blank_lines_removed += 1,
            }
        } else if line.starts_with(' ') {
            // Context line
            if !context_warning_printed {
                eprintln!("Warning: Context line detected. Please use 'git diff --unified=0' for accurate results.");
                context_warning_printed = true;
            }
        }
    }

    if let Some(file_stats) = current_file_stats.take() {
        if !is_binary_diff
            && (file_stats.lang_stats.total_added > 0 || file_stats.lang_stats.total_removed > 0)
        {
            stats.push(file_stats);
        }
    }

    Ok(())
}

fn count_words(line: &str) -> usize {
    line.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_diff_synthetic() {
        let diff_input = "\
diff --git a/test.py b/test.py
index 123..456 100644
--- a/test.py
+++ b/test.py
@@ -1,3 +1,3 @@
-def foo():
-# comment
+def bar():
+    pass
";
        let mut stats = Vec::new();
        let reader = Cursor::new(diff_input);
        parse_diff(reader, &mut stats).unwrap();

        assert_eq!(stats.len(), 1);
        let file_stats = &stats[0];
        assert_eq!(file_stats.path, "test.py");
        assert_eq!(file_stats.language, "Python");

        let lang_stats = &file_stats.lang_stats;
        assert_eq!(lang_stats.total_removed, 2);
        assert_eq!(lang_stats.total_added, 2);
        assert_eq!(lang_stats.pure_removed, 1);
        assert_eq!(lang_stats.pure_added, 2);
    }
}
