use crate::classifier::get_classifier;
use crate::stats::LangStats;
use std::collections::HashMap;

pub fn detect_language(path: &str) -> String {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    match extension {
        "py" => "Python",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "c" | "h" => "C",
        "cpp" | "cc" | "hpp" | "hh" => "C++",
        "cs" => "C#",
        "java" => "Java",
        "go" => "Go",
        "swift" => "Swift",
        "kt" | "kts" => "Kotlin",
        "php" => "PHP",
        "scala" => "Scala",
        "rb" => "Ruby",
        "sh" | "bash" | "zsh" => "Shell",
        "ps1" => "PowerShell",
        "css" | "scss" => "CSS",
        "html" | "htm" => "HTML",
        "vue" => "Vue",
        _ => "Other",
    }
    .to_string()
}

pub fn parse_diff<R: std::io::BufRead>(
    reader: R,
    stats: &mut HashMap<String, LangStats>,
) -> Result<(), std::io::Error> {
    let mut current_lang = String::new();
    let mut classifier = get_classifier("Other");

    for line_result in reader.lines() {
        let line = line_result?;

        if line.starts_with("+++ ") {
            // Parse file path: "+++ b/path/to/file.ext"
            // Or "+++ /dev/null"
            let path_part = line.trim_start_matches("+++ ").trim();

            if path_part == "/dev/null" || path_part.ends_with("/dev/null") {
                // File deleted. We still process the "-" lines (which appeared before usually? or in this hunk?)
                // Actually in unified diff:
                // --- a/file
                // +++ /dev/null
                // @@ ... @@
                // - content

                // If we see +++ /dev/null, it means we are in the hunk for a deleted file.
                // We should have seen --- a/file before.
                // Wait, usually --- comes first.
                // If we rely on +++ to set the language, and it is /dev/null, we might lose the language info from ---.
                // Strategy:
                // If line starts with "--- ", try to extract lang.
                // If line starts with "+++ ", override lang unless it is /dev/null.
                continue;
            }

            // Strip prefix a/ or b/
            let clean_path = if path_part.starts_with("b/") {
                &path_part[2..]
            } else if path_part.starts_with("a/") {
                &path_part[2..]
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
                let clean_path = if path_part.starts_with("a/") {
                    &path_part[2..]
                } else if path_part.starts_with("b/") {
                    &path_part[2..]
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
            if !classifier.is_noise(content) {
                stat.pure_added += 1;
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            let content = &line[1..];
            let stat = stats.entry(current_lang.clone()).or_default();
            stat.total_removed += 1;
            if !classifier.is_noise(content) {
                stat.pure_removed += 1;
            }
        }
    }
    Ok(())
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
    }
}
