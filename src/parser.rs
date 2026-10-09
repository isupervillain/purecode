use crate::classifier::{get_classifier, Classifier, LineType};
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
    // Lines still owed by the current hunk. While non-zero, `---`/`+++` are content, not headers.
    let (mut old_left, mut new_left) = (0usize, 0usize);

    for line_result in reader.lines() {
        let line = line_result?;

        if line.starts_with("diff --git ") {
            (old_left, new_left) = (0, 0);
        }

        if old_left > 0 || new_left > 0 {
            if let Some(fs) = &mut current_file_stats {
                match line.as_bytes().first() {
                    Some(b'-') => {
                        old_left = old_left.saturating_sub(1);
                        record(&mut fs.lang_stats, classifier.as_mut(), &line[1..], false);
                    }
                    Some(b'+') => {
                        new_left = new_left.saturating_sub(1);
                        record(&mut fs.lang_stats, classifier.as_mut(), &line[1..], true);
                    }
                    Some(b'\\') => {} // "\ No newline at end of file"
                    _ => {
                        old_left = old_left.saturating_sub(1);
                        new_left = new_left.saturating_sub(1);
                        if !context_warning_printed {
                            eprintln!("Warning: Context line detected. Please use 'git diff --unified=0' for accurate results.");
                            context_warning_printed = true;
                        }
                    }
                }
                continue;
            }
        }

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
            (old_left, new_left) = hunk_counts(&line);
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

/// Classifies one added/removed line and updates `stat`.
fn record(stat: &mut LangStats, classifier: &mut dyn Classifier, content: &str, added: bool) {
    let words = count_words(content) as i64;
    let (total, pure, words_stat, comment, docstring, blank) = if added {
        (
            &mut stat.total_added,
            &mut stat.pure_added,
            &mut stat.code_words_added,
            &mut stat.comment_lines_added,
            &mut stat.docstring_lines_added,
            &mut stat.blank_lines_added,
        )
    } else {
        (
            &mut stat.total_removed,
            &mut stat.pure_removed,
            &mut stat.code_words_removed,
            &mut stat.comment_lines_removed,
            &mut stat.docstring_lines_removed,
            &mut stat.blank_lines_removed,
        )
    };
    *total += 1;
    match classifier.classify(content) {
        LineType::Pure => {
            *pure += 1;
            *words_stat += words;
        }
        LineType::Comment => *comment += 1,
        LineType::Docstring => *docstring += 1,
        LineType::Blank => *blank += 1,
    }
}

/// Parses `@@ -a[,b] +c[,d] @@` into the (old, new) line counts; a missing count means 1.
fn hunk_counts(header: &str) -> (usize, usize) {
    let count = |prefix: char| {
        header
            .split_whitespace()
            .find_map(|t| t.strip_prefix(prefix))
            .map(|r| r.split_once(',').map_or(Some(1), |(_, n)| n.parse().ok()))
            .map_or(0, |n| n.unwrap_or(0))
    };
    (count('-'), count('+'))
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
@@ -1,2 +1,2 @@
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

    #[test]
    fn test_content_resembling_headers_is_counted() {
        // `-- x` removed => "--- x"; `++i;` added => "+++i;"; both look like file headers.
        let diff_input = "\
diff --git a/a.c b/a.c
--- a/a.c
+++ b/a.c
@@ -2,2 +2,2 @@
-*p = 1;
--- x
++++i;
+*p = 2;
diff --git a/b.py b/b.py
--- a/b.py
+++ b/b.py
@@ -1 +1 @@
-a = 1
\\ No newline at end of file
+a = 2
";
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats).unwrap();

        assert_eq!(stats.len(), 2);
        let c = &stats[0].lang_stats;
        assert_eq!((c.total_added, c.total_removed), (2, 2));
        // `*p = 1;` is code, `-- x` is code in C; nothing is a comment.
        assert_eq!((c.pure_added, c.pure_removed), (2, 2));
        let py = &stats[1].lang_stats;
        assert_eq!((py.pure_added, py.pure_removed), (1, 1));
    }

    #[test]
    fn test_hunk_counts() {
        assert_eq!(hunk_counts("@@ -1,3 +4 @@ fn x()"), (3, 1));
        assert_eq!(hunk_counts("@@ -0,0 +1,5 @@"), (0, 5));
    }
}
