use crate::classifier::{get_classifier, text_lines, Classifier, LineType};
use crate::language::Language;
use crate::stats::{FileStats, LangStats};
use std::io::{self, BufRead};
use std::path::Path;

/// Full file contents by git blob id, so changed lines can be classified in the context of the
/// whole file (an added line inside an existing docstring is a docstring line).
pub trait BlobSource {
    fn blob(&mut self, id: &str) -> Option<Vec<u8>>;
}

/// No file contents available: every hunk is classified on its own.
pub struct NoBlobs;

impl BlobSource for NoBlobs {
    fn blob(&mut self, _id: &str) -> Option<Vec<u8>> {
        None
    }
}

/// One side (old or new) of the file being parsed.
struct Side {
    language: Language,
    blob_id: Option<String>,
    /// Lines of the whole file with their classification; loaded on first use.
    lines: Option<Option<Vec<(String, LineType)>>>,
    /// Classifier for the current hunk alone, used when the whole file is unavailable.
    hunk: Box<dyn Classifier>,
    /// 1-based line number of the next line of this side in the current hunk.
    next_line: usize,
}

impl Side {
    fn new(language: Language) -> Self {
        Self {
            language,
            blob_id: None,
            lines: None,
            hunk: get_classifier(language),
            next_line: 0,
        }
    }

    fn start_hunk(&mut self, first_line: usize) {
        self.hunk = get_classifier(self.language);
        self.next_line = first_line;
    }

    /// Classifies the next line of this side, preferring the whole-file classification when the
    /// file is available and its line matches the diff text exactly.
    fn classify(&mut self, content: &str, blobs: &mut dyn BlobSource) -> LineType {
        let local = self.hunk.classify(content);
        let n = self.next_line;
        self.next_line += 1;
        let language = self.language;
        let id = self.blob_id.as_deref();
        let lines = self.lines.get_or_insert_with(|| {
            let bytes = blobs.blob(id?)?;
            let mut classifier = get_classifier(language);
            Some(
                text_lines(&bytes)
                    .map(|l| {
                        let t = classifier.classify(&l);
                        (l, t)
                    })
                    .collect(),
            )
        });
        match lines.as_ref().and_then(|ls| ls.get(n.checked_sub(1)?)) {
            Some((text, t)) if text == content => *t,
            _ => local,
        }
    }
}

/// The file being parsed.
struct FileState {
    stats: FileStats,
    old: Side,
    new: Side,
}

/// Parses a unified diff from `reader` and appends per-file statistics to `stats`.
pub fn parse_diff<R: BufRead>(
    mut reader: R,
    stats: &mut Vec<FileStats>,
    blobs: &mut dyn BlobSource,
) -> io::Result<()> {
    let mut file: Option<FileState> = None;
    // Metadata of the next file, from its `diff --git` / `index` lines.
    let mut blob_ids: (Option<String>, Option<String>) = (None, None);
    let mut submodule = false;
    // Lines still owed by the current hunk. While non-zero, `---`/`+++` are content, not headers.
    let (mut old_left, mut new_left) = (0usize, 0usize);
    let (mut saw_diff, mut saw_text) = (false, false);
    let mut buf = Vec::new();

    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        // Diffs of Latin-1 (or otherwise non-UTF-8) files must not abort the whole run.
        let decoded = String::from_utf8_lossy(&buf);
        let line = decoded.trim_end_matches(['\n', '\r']);
        saw_text |= !line.trim().is_empty();

        if old_left > 0 || new_left > 0 {
            if let Some(f) = &mut file {
                let content = line.get(1..).unwrap_or("").trim_start_matches('\u{feff}');
                match line.as_bytes().first() {
                    Some(b'-') => {
                        old_left -= old_left.min(1);
                        let t = f.old.classify(content, blobs);
                        record(&mut f.stats.lang_stats, t, content, false);
                    }
                    Some(b'+') => {
                        new_left -= new_left.min(1);
                        let t = f.new.classify(content, blobs);
                        record(&mut f.stats.lang_stats, t, content, true);
                    }
                    Some(b'\\') => {} // "\ No newline at end of file"
                    _ => {
                        // Context line: not counted, but it carries comment/string state.
                        old_left -= old_left.min(1);
                        new_left -= new_left.min(1);
                        f.old.classify(content, blobs);
                        f.new.classify(content, blobs);
                    }
                }
                continue;
            }
        }

        if line.starts_with("diff --cc ") || line.starts_with("diff --combined ") {
            return Err(io::Error::other(
                "combined (merge) diffs are not supported; diff against one parent instead, \
                 e.g. `git diff <merge>^1 <merge>`",
            ));
        }

        if line.starts_with("diff --git ") {
            // A new file starts; files without `---` headers (binary, mode-only) must not
            // swallow the previous file's stats.
            flush(&mut file, stats);
            saw_diff = true;
            blob_ids = (None, None);
            submodule = false;
            (old_left, new_left) = (0, 0);
            continue;
        }

        // `index <old>..<new> [mode]`: blob ids of both sides; mode 160000 is a submodule.
        if let Some(rest) = line.strip_prefix("index ") {
            let mut parts = rest.split_whitespace();
            if let Some((old, new)) = parts.next().and_then(|ids| ids.split_once("..")) {
                blob_ids = (blob_id(old), blob_id(new));
            }
            submodule = parts.next() == Some("160000");
            continue;
        }

        // "Binary files a/foo and b/foo differ": nothing to count for this file.
        if line.starts_with("Binary files") && line.contains("differ") {
            flush(&mut file, stats);
            continue;
        }

        if let Some(path) = line.strip_prefix("--- ") {
            flush(&mut file, stats);
            saw_diff = true;
            let path = unquote(path.trim());
            if path == "/dev/null" || submodule {
                continue;
            }
            let path = path.strip_prefix("a/").unwrap_or(&path);
            file = Some(new_file(path, &blob_ids));
            continue;
        }

        if let Some(path) = line.strip_prefix("+++ ") {
            let path = unquote(path.trim());
            if path == "/dev/null" || submodule {
                continue;
            }
            let path = path.strip_prefix("b/").unwrap_or(&path);
            match &mut file {
                // Renamed: report the new path; the old side keeps the old file's language.
                Some(f) if f.stats.path != path => {
                    let language = Language::from_path(Path::new(path));
                    f.stats.path = path.to_string();
                    f.stats.language = language.to_string();
                    f.new = Side::new(language);
                    f.new.blob_id = blob_ids.1.clone();
                }
                Some(_) => {}
                None => file = Some(new_file(path, &blob_ids)), // added file (`--- /dev/null`)
            }
            continue;
        }

        if line.starts_with("@@@") {
            return Err(io::Error::other(
                "combined (merge) diffs are not supported; diff against one parent instead",
            ));
        }
        if line.starts_with("@@") {
            let ((old_start, old_count), (new_start, new_count)) = hunk_ranges(line);
            (old_left, new_left) = (old_count, new_count);
            if let Some(f) = &mut file {
                f.old.start_hunk(old_start);
                f.new.start_hunk(new_start);
            }
        }
        // Anything else outside a hunk is metadata (mode, rename, similarity lines).
    }

    flush(&mut file, stats);
    if saw_text && !saw_diff {
        eprintln!("Warning: input does not look like a unified diff; nothing was counted.");
    }
    Ok(())
}

fn new_file(path: &str, blob_ids: &(Option<String>, Option<String>)) -> FileState {
    let language = Language::from_path(Path::new(path));
    let mut old = Side::new(language);
    let mut new = Side::new(language);
    old.blob_id = blob_ids.0.clone();
    new.blob_id = blob_ids.1.clone();
    FileState {
        stats: FileStats {
            path: path.to_string(),
            language: language.to_string(),
            lang_stats: LangStats::default(),
        },
        old,
        new,
    }
}

/// A usable blob id: hex only (stdin is untrusted), and not the all-zero id of a missing side.
fn blob_id(id: &str) -> Option<String> {
    let valid = id.len() >= 4 && id.bytes().all(|b| b.is_ascii_hexdigit());
    (valid && id.bytes().any(|b| b != b'0')).then(|| id.to_string())
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
fn flush(file: &mut Option<FileState>, stats: &mut Vec<FileStats>) {
    if let Some(f) = file.take() {
        if f.stats.lang_stats.total_added > 0 || f.stats.lang_stats.total_removed > 0 {
            stats.push(f.stats);
        }
    }
}

/// Counts one added/removed line of type `t` into `stat`.
fn record(stat: &mut LangStats, t: LineType, content: &str, added: bool) {
    let words = content.split_whitespace().count() as i64;
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
    match t {
        LineType::Pure => {
            *pure += 1;
            *words_stat += words;
        }
        LineType::Comment => *comment += 1,
        LineType::Docstring => *docstring += 1,
        LineType::Blank => *blank += 1,
    }
}

/// Parses `@@ -a[,b] +c[,d] @@` into ((old start, old count), (new start, new count));
/// a missing count means 1.
fn hunk_ranges(header: &str) -> ((usize, usize), (usize, usize)) {
    let range = |prefix: char| {
        header
            .split_whitespace()
            .find_map(|t| t.strip_prefix(prefix))
            .map_or((0, 0), |r| match r.split_once(',') {
                Some((start, n)) => (start.parse().unwrap_or(0), n.parse().unwrap_or(0)),
                None => (r.parse().unwrap_or(0), 1),
            })
    };
    (range('-'), range('+'))
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
        parse_diff(reader, &mut stats, &mut NoBlobs).unwrap();

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
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();

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
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();
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
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();
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
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();
        assert_eq!(stats[0].path, "é ü.py");
        assert_eq!(stats[0].language, "Python");
        let s = &stats[0].lang_stats;
        assert_eq!((s.pure_removed, s.pure_added), (1, 1));
    }

    #[test]
    fn test_hunk_ranges() {
        assert_eq!(hunk_ranges("@@ -1,3 +4 @@ fn x()"), ((1, 3), (4, 1)));
        assert_eq!(hunk_ranges("@@ -0,0 +1,5 @@"), ((0, 0), (1, 5)));
    }

    struct MapBlobs(std::collections::HashMap<&'static str, &'static str>);

    impl BlobSource for MapBlobs {
        fn blob(&mut self, id: &str) -> Option<Vec<u8>> {
            self.0.get(id).map(|s| s.as_bytes().to_vec())
        }
    }

    #[test]
    fn test_lines_added_inside_existing_docstring_and_comment() {
        let old_py = "def f():\n    \"\"\"Summary.\n\n    \"\"\"\n    return 1\n";
        let new_py = "def f():\n    \"\"\"Summary.\n\n    More detail.\n    And more.\n    \"\"\"\n    return 1\n";
        let old_rs = "/*\n * a\n */\nfn x() {}\n";
        let new_rs = "/*\n * a\n new text\n */\nfn x() {}\n";
        let diff_input = "\
diff --git a/a.py b/a.py
index aaaa111..bbbb222 100644
--- a/a.py
+++ b/a.py
@@ -3,0 +4,2 @@
+    More detail.
+    And more.
diff --git a/a.rs b/a.rs
index cccc333..dddd444 100644
--- a/a.rs
+++ b/a.rs
@@ -2,0 +3 @@
+ new text
";
        let mut blobs = MapBlobs(
            [
                ("aaaa111", old_py),
                ("bbbb222", new_py),
                ("cccc333", old_rs),
                ("dddd444", new_rs),
            ]
            .into(),
        );
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats, &mut blobs).unwrap();
        let py = &stats[0].lang_stats;
        assert_eq!((py.docstring_lines_added, py.pure_added), (2, 0));
        let rs = &stats[1].lang_stats;
        assert_eq!((rs.comment_lines_added, rs.pure_added), (1, 0));

        // Without file contents the same hunks can only be judged on their own.
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();
        assert_eq!(stats[0].lang_stats.pure_added, 2);
    }

    #[test]
    fn test_blob_not_matching_diff_text_is_ignored() {
        // A blob from elsewhere (wrong repo, textconv) must not override the diff's own text.
        let diff_input = "\
diff --git a/a.py b/a.py
index aaaa111..bbbb222 100644
--- a/a.py
+++ b/a.py
@@ -0,0 +1 @@
+x = 1
";
        let mut blobs = MapBlobs([("bbbb222", "# x = 1 is different\n")].into());
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats, &mut blobs).unwrap();
        assert_eq!(stats[0].lang_stats.pure_added, 1);
    }

    #[test]
    fn test_context_lines_carry_state_without_blobs() {
        let diff_input = "\
diff --git a/a.py b/a.py
--- a/a.py
+++ b/a.py
@@ -1,3 +1,4 @@
 def f():
     \"\"\"Summary.
+    More detail.
     \"\"\"
";
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();
        let s = &stats[0].lang_stats;
        assert_eq!((s.total_added, s.docstring_lines_added), (1, 1));
    }

    #[test]
    fn test_rename_across_languages_keeps_old_language_for_removed_lines() {
        let diff_input = "\
diff --git a/old.py b/new.js
--- a/old.py
+++ b/new.js
@@ -1,2 +1 @@
-# python comment
-# another
+x();
";
        let mut stats = Vec::new();
        parse_diff(Cursor::new(diff_input), &mut stats, &mut NoBlobs).unwrap();
        let s = &stats[0].lang_stats;
        assert_eq!((s.comment_lines_removed, s.pure_added), (2, 1));
        assert_eq!(stats[0].path, "new.js");
    }

    #[test]
    fn test_submodule_and_combined_diffs() {
        let submodule = "\
diff --git a/lib b/lib
index 1111111..2222222 160000
--- a/lib
+++ b/lib
@@ -1 +1 @@
-Subproject commit 1111111
+Subproject commit 2222222
";
        let mut stats = Vec::new();
        parse_diff(Cursor::new(submodule), &mut stats, &mut NoBlobs).unwrap();
        assert!(stats.is_empty());

        let combined =
            "diff --cc a.py\nindex 1,2..3\n--- a/a.py\n+++ b/a.py\n@@@ -1,1 -1,1 +1,2 @@@\n";
        let err = parse_diff(Cursor::new(combined), &mut Vec::new(), &mut NoBlobs).unwrap_err();
        assert!(err.to_string().contains("combined"));
    }
}
