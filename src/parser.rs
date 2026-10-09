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
    // Removed and added lines are separate texts; each side needs its own comment/string state.
    let (mut old_classifier, mut new_classifier) = classifiers(Language::Other);
    let mut context_warning_printed = false;
    // Lines still owed by the current hunk. While non-zero, `---`/`+++` are content, not headers.
    let (mut old_left, mut new_left) = (0usize, 0usize);
    let mut reader = reader;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        // Diffs of Latin-1 (or otherwise non-UTF-8) files must not abort the whole run.
        let decoded = String::from_utf8_lossy(&buf);
        let line = decoded.trim_end_matches(['\n', '\r']);

        if line.starts_with("diff --git ") {
            // A new file starts; files without `---` headers (binary, mode-only) must not
            // swallow the previous file's stats.
            flush(&mut current_file_stats, stats);
            (old_left, new_left) = (0, 0);
        }

        if old_left > 0 || new_left > 0 {
            if let Some(fs) = &mut current_file_stats {
                match line.as_bytes().first() {
                    Some(b'-') => {
                        old_left = old_left.saturating_sub(1);
                        record(
                            &mut fs.lang_stats,
                            old_classifier.as_mut(),
                            &line[1..],
                            false,
                        );
                    }
                    Some(b'+') => {
                        new_left = new_left.saturating_sub(1);
                        record(
                            &mut fs.lang_stats,
                            new_classifier.as_mut(),
                            &line[1..],
                            true,
                        );
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

        // "Binary files a/foo and b/foo differ": nothing to count for this file.
        if line.starts_with("Binary files") && line.contains("differ") {
            flush(&mut current_file_stats, stats);
            continue;
        }

        if line.starts_with("--- ") {
            flush(&mut current_file_stats, stats);

            let path_part = unquote(line.trim_start_matches("--- ").trim());
            let path_part = path_part.as_str();
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
            (old_classifier, new_classifier) = classifiers(language);
            current_file_stats = Some(FileStats {
                path: clean_path.to_string(),
                language: language.to_string(),
                lang_stats: LangStats::default(),
            });
            continue;
        }

        if line.starts_with("+++ ") {
            let path_part = unquote(line.trim_start_matches("+++ ").trim());
            let path_part = path_part.as_str();
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
                    (old_classifier, new_classifier) = classifiers(language);
                    fs.path = clean_path.to_string();
                    fs.language = language.to_string();
                }
            } else {
                let language = Language::from_path(Path::new(clean_path));
                (old_classifier, new_classifier) = classifiers(language);
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
            (old_left, new_left) = hunk_counts(line);
            // Reset classifier state for new hunk because hunks are disjoint
            // and carrying state (like in_comment) across hunks is dangerous.
            // We re-initialize the classifier for the current language.
            if let Some(fs) = &current_file_stats {
                (old_classifier, new_classifier) =
                    classifiers(Language::from_path(Path::new(&fs.path)));
            }
            continue;
        }
        // Anything else outside a hunk is metadata (index, mode, rename, similarity lines).
    }

    flush(&mut current_file_stats, stats);

    Ok(())
}

type BoxedClassifier = Box<dyn Classifier>;

fn classifiers(language: Language) -> (BoxedClassifier, BoxedClassifier) {
    (get_classifier(language), get_classifier(language))
}

/// Decodes a path git wrote as a C-style quoted string (`"b/\303\251.py"`) for
/// non-ASCII or special characters; other paths are returned unchanged.
fn unquote(path: &str) -> String {
    let Some(inner) = path.strip_prefix('"').and_then(|p| p.strip_suffix('"')) else {
        return path.to_string();
    };
    let src = inner.as_bytes();
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] != b'\\' || i + 1 == src.len() {
            out.push(src[i]);
            i += 1;
            continue;
        }
        let esc = src[i + 1];
        i += 2;
        out.push(match esc {
            b'n' => b'\n',
            b't' => b'\t',
            b'r' => b'\r',
            b'a' => 7,
            b'b' => 8,
            b'f' => 12,
            b'v' => 11,
            b'0'..=b'7' => {
                // Up to three octal digits encode one raw byte of the UTF-8 path.
                let mut v = u32::from(esc - b'0');
                for _ in 0..2 {
                    match src.get(i) {
                        Some(d @ b'0'..=b'7') => {
                            v = v * 8 + u32::from(d - b'0');
                            i += 1;
                        }
                        _ => break,
                    }
                }
                v as u8
            }
            other => other, // \" and \\
        });
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Moves the file being parsed into `stats` if any line of it was counted.
fn flush(current: &mut Option<FileStats>, stats: &mut Vec<FileStats>) {
    if let Some(fs) = current.take() {
        if fs.lang_stats.total_added > 0 || fs.lang_stats.total_removed > 0 {
            stats.push(fs);
        }
    }
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
    match classifier.classify(content.trim_start_matches('\u{feff}')) {
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
    fn test_text_file_followed_by_binary_file_is_kept() {
        let diff_input = "\
diff --git a/a.ts b/a.ts
--- a/a.ts
+++ b/a.ts
@@ -1 +1,2 @@
-  'x'
+  'x',
+  'y'
diff --git a/i.png b/i.png
new file mode 100644
Binary files /dev/null and b/i.png differ
diff --git a/b.ts b/b.ts
old mode 100644
new mode 100755
";
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].path, "a.ts");
        assert_eq!(
            (
                stats[0].lang_stats.total_added,
                stats[0].lang_stats.total_removed
            ),
            (2, 1)
        );
    }

    #[test]
    fn test_removed_and_added_sides_do_not_share_state() {
        // Editing a docstring's first line: the removed `"""` must not close the added one.
        let diff_input = "\
diff --git a/a.py b/a.py
--- a/a.py
+++ b/a.py
@@ -3 +3,2 @@
-    \"\"\"Old summary.
+    \"\"\"New summary.
+    Second line.\"\"\"
diff --git a/b.c b/b.c
--- a/b.c
+++ b/b.c
@@ -1 +1 @@
-/* old
+int y;
";
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats).unwrap();
        let py = &stats[0].lang_stats;
        assert_eq!(
            (py.docstring_lines_removed, py.docstring_lines_added),
            (1, 2)
        );
        assert_eq!(py.pure_added, 0);
        let c = &stats[1].lang_stats;
        assert_eq!((c.comment_lines_removed, c.pure_added), (1, 1));
    }

    #[test]
    fn test_quoted_paths_and_non_utf8_content() {
        let mut diff_input =
            b"diff --git \"a/\\303\\251 \\303\\274.py\" \"b/\\303\\251 \\303\\274.py\"\n\
--- \"a/\\303\\251 \\303\\274.py\"\n\
+++ \"b/\\303\\251 \\303\\274.py\"\n\
@@ -1 +1 @@\n"
                .to_vec();
        diff_input.extend_from_slice(b"-x = \"caf\xe9\"\r\n+x = 1\r\n");
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats).unwrap();
        assert_eq!(stats[0].path, "é ü.py");
        assert_eq!(stats[0].language, "Python");
        let s = &stats[0].lang_stats;
        assert_eq!((s.pure_removed, s.pure_added), (1, 1));
    }

    #[test]
    fn test_hunk_counts() {
        assert_eq!(hunk_counts("@@ -1,3 +4 @@ fn x()"), (3, 1));
        assert_eq!(hunk_counts("@@ -0,0 +1,5 @@"), (0, 5));
    }
}
