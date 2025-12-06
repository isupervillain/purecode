use crate::classifier::{get_classifier, LineType};
use crate::detect_language;
use crate::stats::LangStats;
use std::collections::HashMap;

/// Parses a unified diff from the reader and updates statistics.
///
/// This function reads the diff line by line, detects file changes and languages,
/// uses classifiers to determine if lines are pure code or noise, and aggregates
/// the results into the provided `stats` map.
pub fn parse_diff<R: std::io::BufRead>(
    reader: R,
    stats: &mut HashMap<String, LangStats>,
) -> Result<(), std::io::Error> {
    let mut current_lang = String::new();
    let mut classifier = get_classifier("Other");

    for line_result in reader.lines() {
        let line = line_result?;

        if line.starts_with("+++ ") {
            let path_part = line.trim_start_matches("+++ ").trim();

            if path_part == "/dev/null" || path_part.ends_with("/dev/null") {
                continue;
            }

            // Strip prefix a/ or b/
            let clean_path = if let Some(stripped) = path_part.strip_prefix("b/") {
                stripped
            } else if let Some(stripped) = path_part.strip_prefix("a/") {
                stripped
            } else {
                path_part
            };

            current_lang = detect_language(clean_path);
            classifier = get_classifier(&current_lang);
            continue;
        }

        if line.starts_with("--- ") {
            let path_part = line.trim_start_matches("--- ").trim();
            if path_part != "/dev/null" && !path_part.ends_with("/dev/null") {
                let clean_path = if let Some(stripped) = path_part.strip_prefix("a/") {
                    stripped
                } else if let Some(stripped) = path_part.strip_prefix("b/") {
                    stripped
                } else {
                    path_part
                };
                current_lang = detect_language(clean_path);
                classifier = get_classifier(&current_lang);
            }
            continue;
        }

        // Ignore metadata
        if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("new file mode")
            || line.starts_with("deleted file mode")
            || line.starts_with("@@")
        {
            continue;
        }

        if current_lang.is_empty() {
            // Probably haven't seen file header yet or parsing error, skip
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            let content = &line[1..];
            let stat = stats.entry(current_lang.clone()).or_default();
            stat.total_added += 1;

            match classifier.classify(content) {
                LineType::Pure => {
                    stat.pure_added += 1;
                    stat.code_words_added += count_words(content) as i64;
                },
                LineType::Comment => stat.comment_lines_added += 1,
                LineType::Docstring => stat.docstring_lines_added += 1,
                LineType::Blank => stat.blank_lines_added += 1,
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            let content = &line[1..];
            let stat = stats.entry(current_lang.clone()).or_default();
            stat.total_removed += 1;

            match classifier.classify(content) {
                LineType::Pure => {
                    stat.pure_removed += 1;
                    stat.code_words_removed += count_words(content) as i64;
                },
                LineType::Comment => stat.comment_lines_removed += 1,
                LineType::Docstring => stat.docstring_lines_removed += 1,
                LineType::Blank => stat.blank_lines_removed += 1,
            }
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
        let mut stats = HashMap::new();
        let reader = Cursor::new(diff_input);
        parse_diff(reader, &mut stats).unwrap();

        let py_stats = stats.get("Python").unwrap();
        // Removed: "def foo():", "# comment"
        // Added: "def bar():", "    pass"
        assert_eq!(py_stats.total_removed, 2);
        assert_eq!(py_stats.total_added, 2);

        // Pure Removed: "def foo():" (1)
        // Pure Added: "def bar():", "    pass" (2)
        assert_eq!(py_stats.pure_removed, 1);
        assert_eq!(py_stats.pure_added, 2);

        // Words
        // "def foo():" -> 2 words
        // "def bar():" -> 2 words
        // "    pass"   -> 1 word
        assert_eq!(py_stats.code_words_removed, 2);
        assert_eq!(py_stats.code_words_added, 3);

        // Comments
        assert_eq!(py_stats.comment_lines_removed, 1);
        assert_eq!(py_stats.comment_lines_added, 0);
    }
}
