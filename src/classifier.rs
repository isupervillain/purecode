use crate::language::Language;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LineType {
    Pure,
    Comment,
    Docstring,
    Blank,
}

pub trait Classifier {
    fn classify(&mut self, line: &str) -> LineType;
}

pub struct DefaultClassifier;

impl Classifier for DefaultClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        if line.trim().is_empty() {
            LineType::Blank
        } else {
            LineType::Pure
        }
    }
}

impl Default for PythonClassifier {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PythonClassifier {
    /// Open triple-quoted string: its delimiter, and whether it is a docstring (true) or data.
    open: Option<(&'static str, bool)>,
    /// Open `(`/`[`/`{` count: inside brackets a string starting a line is an argument, not a
    /// docstring.
    depth: usize,
    /// The previous line ended with a `\` continuation.
    continued: bool,
}

impl PythonClassifier {
    pub fn new() -> Self {
        Self {
            open: None,
            depth: 0,
            continued: false,
        }
    }
}

impl Classifier for PythonClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let t = line.trim();
        if t.is_empty() {
            return LineType::Blank;
        }
        let bytes = t.as_bytes();
        let (mut has_code, mut has_doc) = (false, false);
        let mut i = 0;
        // A docstring is a string at the start of a logical line, outside any brackets.
        let logical_start = self.depth == 0 && !std::mem::take(&mut self.continued);

        if let Some((delim, doc)) = self.open {
            let Some(j) = t.find(delim) else {
                return if doc {
                    LineType::Docstring
                } else {
                    LineType::Pure
                };
            };
            self.open = None;
            i = j + 3;
            if doc {
                has_doc = true;
            } else {
                has_code = true;
            }
        } else if t.starts_with("#!") {
            return LineType::Pure; // shebang
        }

        // Start of text since the last string/docstring that is not attributed yet.
        let mut seg = i;
        let mut quote: Option<u8> = None;
        while i < bytes.len() {
            let c = bytes[i];
            if let Some(q) = quote {
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            if c == b'#' {
                break;
            }
            match c {
                b'(' | b'[' | b'{' => self.depth += 1,
                b')' | b']' | b'}' => self.depth = self.depth.saturating_sub(1),
                _ => {}
            }
            // Byte-wise compare: `i` may sit inside a multi-byte char, where `t[i..]` would panic.
            let delim = if bytes[i..].starts_with(b"\"\"\"") {
                "\"\"\""
            } else if bytes[i..].starts_with(b"'''") {
                "'''"
            } else {
                if c == b'"' || c == b'\'' {
                    quote = Some(c);
                }
                i += 1;
                continue;
            };
            // A statement-level triple-quoted string (only an r/u/b prefix before it) is a
            // docstring; anything else (`x = """`, `f"""`, `help="""`) is data, i.e. code.
            let pre = &t[seg..i];
            let prefix_len = pre
                .bytes()
                .rev()
                .take_while(|b| b"rRuUbBfF".contains(b))
                .count();
            let (head, prefix) = if prefix_len <= 2 {
                pre.split_at(pre.len() - prefix_len)
            } else {
                (pre, "")
            };
            if !head.trim().is_empty() {
                has_code = true;
            }
            let doc = !has_code && logical_start && !prefix.contains(['f', 'F']);
            if doc {
                has_doc = true;
            } else {
                has_code = true;
            }
            match t[i + 3..].find(delim) {
                Some(j) => {
                    i += 3 + j + 3;
                    seg = i;
                }
                None => {
                    self.open = Some((delim, doc));
                    i = bytes.len();
                    seg = i;
                }
            }
        }
        let end = i.min(t.len());
        if !t[seg.min(end)..end].trim().is_empty() {
            has_code = true;
        }
        if self.open.is_none() && quote.is_none() {
            self.continued = t[..end].trim_end().ends_with('\\');
        }

        if has_code {
            LineType::Pure
        } else if has_doc {
            LineType::Docstring
        } else {
            LineType::Comment
        }
    }
}

pub struct CStyleClassifier {
    /// Open block comments; above 1 only for languages that nest them (Rust).
    block_depth: usize,
    /// Closer of a string literal that can span lines (template literal, text block, raw string),
    /// and whether backslash escapes apply inside it.
    open_string: Option<(String, bool)>,
    /// Rust: `'` starts a char literal or a lifetime, never a string.
    char_literals: bool,
    /// Go: backtick strings are raw (no escapes, no `${}` interpolation).
    raw_backticks: bool,
    /// Brace depth inside each open `${...}` template-literal interpolation (innermost last).
    interpolations: Vec<usize>,
    /// No code line seen yet: a diff hunk may begin in the middle of a block comment.
    at_start: bool,
}

impl CStyleClassifier {
    pub fn new() -> Self {
        Self {
            block_depth: 0,
            open_string: None,
            char_literals: false,
            raw_backticks: false,
            interpolations: Vec::new(),
            at_start: true,
        }
    }

    pub fn rust() -> Self {
        Self {
            char_literals: true,
            ..Self::new()
        }
    }

    pub fn go() -> Self {
        Self {
            raw_backticks: true,
            ..Self::new()
        }
    }
}

impl Default for CStyleClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier for CStyleClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LineType::Blank;
        }

        // A diff hunk can begin inside a block comment (`* text`, `*`, `*/`). Only guess that at
        // the start: later, `* x;` is a wrapped expression and `* {` a CSS selector.
        if self.at_start {
            let mid_block =
                (trimmed == "*" || trimmed.starts_with("* ") || trimmed.starts_with("*/"))
                    && !trimmed.ends_with(['{', ';'])
                    && !(trimmed.contains('{') && trimmed.contains(';'));
            if mid_block {
                self.at_start = !trimmed.contains("*/");
                return LineType::Comment;
            }
            self.at_start = false;
        }

        // Scan so comment markers inside literals (e.g. "**/*", '/**', `/* x */`) are ignored.
        let chars: Vec<char> = trimmed.chars().collect();
        let mut has_code = false;
        let mut quote: Option<char> = None;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if let Some((closer, escapes)) = &self.open_string {
                has_code = true;
                if *escapes && c == '\\' {
                    i += 2;
                    continue;
                }
                if closer == "`" && *escapes && c == '$' && next == Some('{') {
                    self.open_string = None;
                    self.interpolations.push(0);
                    i += 2;
                    continue;
                }
                if starts_with(&chars[i..], closer) {
                    i += closer.chars().count();
                    self.open_string = None;
                    continue;
                }
            } else if self.block_depth > 0 {
                if c == '*' && next == Some('/') {
                    self.block_depth -= 1;
                    i += 1;
                } else if self.char_literals && c == '/' && next == Some('*') {
                    self.block_depth += 1; // Rust block comments nest
                    i += 1;
                }
            } else if let Some(q) = quote {
                has_code = true;
                if c == '\\' {
                    i += 1;
                } else if c == q {
                    quote = None;
                }
            } else if c == '/' && next == Some('/') {
                break;
            } else if c == '/' && next == Some('*') {
                self.block_depth = 1;
                i += 1;
            } else {
                if !c.is_whitespace() {
                    has_code = true;
                }
                let prev_is_ident =
                    i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
                if let Some(depth) = self.interpolations.last_mut() {
                    if c == '{' {
                        *depth += 1;
                    } else if c == '}' {
                        if *depth == 0 {
                            self.interpolations.pop();
                            self.open_string = Some(("`".into(), true));
                        } else {
                            *depth -= 1;
                        }
                    }
                }
                if c == '\\' {
                    // Outside literals only line continuations use `\`.
                    i += 1;
                } else if let Some(end) = regex_literal_end(&chars, i) {
                    // JS regex literal (/"""/g, /`[^`]*`/): its contents are not literal delimiters.
                    i = end;
                } else if starts_with(&chars[i..], "\"\"\"") {
                    // Text block / raw string (Java, Kotlin, Swift, Scala, C#). Kotlin and C#
                    // raw strings take no escapes, so `\` must not hide the closer.
                    self.open_string = Some(("\"\"\"".into(), false));
                    i += 2;
                } else if let Some((closer, len)) = (!self.char_literals)
                    .then(|| cpp_raw_string(&chars, i))
                    .flatten()
                {
                    // C++ raw string R"delim( ... )delim".
                    self.open_string = Some((closer, false));
                    i += len - 1;
                } else if c == '`' {
                    // Template literal (JS/TS) or raw string (Go).
                    self.open_string = Some(("`".into(), !self.raw_backticks));
                } else if self.char_literals
                    && c == 'r'
                    && !prev_is_ident
                    && is_raw_string_start(&chars[i + 1..])
                {
                    // Rust raw string r#"..."#.
                    let hashes = chars[i + 1..].iter().take_while(|&&x| x == '#').count();
                    let closer: String = std::iter::once('"')
                        .chain(std::iter::repeat_n('#', hashes))
                        .collect();
                    self.open_string = Some((closer, false));
                    i += hashes + 1;
                } else if c == '"' || (c == '\'' && !self.char_literals) {
                    quote = Some(c);
                } else if c == '\'' {
                    // Rust char literal ('"', '\n'); a lone `'` (lifetime) falls through as code.
                    if next == Some('\\') {
                        i += chars[i + 1..]
                            .iter()
                            .position(|&x| x == '\'')
                            .map_or(0, |p| p + 1);
                    } else if chars.get(i + 2) == Some(&'\'') {
                        i += 2;
                    }
                }
            }
            i += 1;
        }

        if has_code {
            LineType::Pure
        } else {
            LineType::Comment
        }
    }
}

fn starts_with(chars: &[char], s: &str) -> bool {
    let mut it = chars.iter();
    s.chars().all(|c| it.next() == Some(&c))
}

/// Index of the closing `/` if a regex literal starts at `start`; `None` means division.
/// A `/` starts a regex only after an operator, an opening bracket, a keyword such as `return`,
/// or at the start of the line.
fn regex_literal_end(chars: &[char], start: usize) -> Option<usize> {
    if chars[start] != '/' {
        return None;
    }
    let is_word = |c: &char| c.is_alphanumeric() || *c == '_' || *c == '$';
    let before = &chars[..start];
    let end = before.iter().rposition(|c| !c.is_whitespace());
    let after_operator = end.is_none_or(|k| "(,=:[!&|?{};".contains(before[k]));
    let word_start = end.map_or(0, |k| {
        before[..=k]
            .iter()
            .rposition(|c| !is_word(c))
            .map_or(0, |p| p + 1)
    });
    let word: String = end.map_or_else(String::new, |k| {
        before[word_start..=k].iter().take(11).collect()
    });
    let after_keyword = matches!(
        word.as_str(),
        "return"
            | "typeof"
            | "case"
            | "do"
            | "else"
            | "in"
            | "of"
            | "void"
            | "yield"
            | "await"
            | "delete"
            | "instanceof"
            | "new"
            | "throw"
    );
    if !after_operator && !after_keyword {
        return None;
    }
    let mut in_class = false;
    let mut j = start + 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 1,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => return (j > start + 1).then_some(j),
            _ => {}
        }
        j += 1;
    }
    None
}

/// For a C++ raw string `R"delim(` (optionally `u8R`, `uR`, `UR`, `LR`) starting at `i`:
/// its closer `)delim"` and the length of the opening up to and including `(`.
fn cpp_raw_string(chars: &[char], i: usize) -> Option<(String, usize)> {
    if chars[i] != 'R' || chars.get(i + 1) != Some(&'"') {
        return None;
    }
    let prefix_start = chars[..i]
        .iter()
        .rposition(|c| !(c.is_alphanumeric() || *c == '_'))
        .map_or(0, |p| p + 1);
    let prefix: String = chars[prefix_start..i].iter().collect();
    if !["", "u8", "u", "U", "L"].contains(&prefix.as_str()) {
        return None;
    }
    let paren = chars[i + 2..].iter().take(17).position(|&c| c == '(')?;
    let delim: String = chars[i + 2..i + 2 + paren].iter().collect();
    if delim.contains([' ', '\\', ')', '"']) {
        return None;
    }
    Some((format!("){delim}\""), paren + 3))
}

fn is_raw_string_start(rest: &[char]) -> bool {
    let hashes = rest.iter().take_while(|&&x| x == '#').count();
    rest.get(hashes) == Some(&'"')
}

pub struct ShellClassifier;

impl Classifier for ShellClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            LineType::Blank
        } else if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
            LineType::Comment
        } else {
            LineType::Pure
        }
    }
}

/// `#` line comments plus `<# ... #>` block comments.
#[derive(Default)]
pub struct PowerShellClassifier {
    in_block: bool,
}

impl Classifier for PowerShellClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LineType::Blank;
        }
        let (code_before, rest) = if self.in_block {
            ("", trimmed)
        } else if let Some(p) = trimmed.find("<#") {
            self.in_block = true;
            (&trimmed[..p], &trimmed[p + 2..])
        } else if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
            return LineType::Comment;
        } else {
            return LineType::Pure;
        };
        let mut has_code = !code_before.trim().is_empty();
        if let Some(end) = rest.find("#>") {
            self.in_block = false;
            let after = rest[end + 2..].trim();
            has_code |= !after.is_empty() && !after.starts_with('#');
        }
        if has_code {
            LineType::Pure
        } else {
            LineType::Comment
        }
    }
}

pub struct RubyClassifier {
    in_block: bool,
}

impl RubyClassifier {
    pub fn new() -> Self {
        Self { in_block: false }
    }
}

impl Default for RubyClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier for RubyClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LineType::Blank;
        }

        if self.in_block {
            if trimmed.starts_with("=end") {
                self.in_block = false;
            }
            return LineType::Comment;
        }

        if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
            return LineType::Comment;
        }

        if trimmed.starts_with("=begin") {
            self.in_block = true;
            return LineType::Comment;
        }

        LineType::Pure
    }
}

pub struct HtmlClassifier {
    in_comment: bool,
    /// Classifier for the body of an open `<script>`/`<style>` element, and its closing tag.
    embedded: Option<(CStyleClassifier, &'static str)>,
}

impl HtmlClassifier {
    pub fn new() -> Self {
        Self {
            in_comment: false,
            embedded: None,
        }
    }
}

impl Default for HtmlClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier for HtmlClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LineType::Blank;
        }
        let lower = trimmed.to_ascii_lowercase();

        if let Some((inner, close)) = &mut self.embedded {
            let Some(p) = lower.find(*close) else {
                return inner.classify(line);
            };
            let rest = lower[p + close.len()..].to_string();
            self.embedded = None;
            self.open_embedded(&rest); // `</script><script>` re-enters
            return LineType::Pure;
        }

        let kind = self.classify_markup(trimmed);
        if !self.in_comment && kind == LineType::Pure {
            self.open_embedded(&lower);
        }
        kind
    }
}

impl HtmlClassifier {
    /// Enters script/style mode if `lower` opens such an element without closing it. The rest
    /// of the line after the opening tag is fed to the embedded classifier for its state.
    fn open_embedded(&mut self, lower: &str) {
        let markup = without_html_comments(lower);
        // (tag position, opening text, closing tag) of the last script/style opening tag.
        let mut last: Option<(usize, &str, &'static str)> = None;
        for (open, close) in [("<script", "</script>"), ("<style", "</style>")] {
            for (p, _) in markup.match_indices(open) {
                // `<style-guide>` or `<StyleProvider>` are other elements.
                let boundary = markup[p + open.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| c.is_whitespace() || c == '>' || c == '/');
                if boundary && last.is_none_or(|(q, _, _)| p > q) {
                    last = Some((p, open, close));
                }
            }
        }
        let Some((p, open, close)) = last else {
            return;
        };
        let after_tag = &markup[p + open.len()..];
        if after_tag.contains(close) {
            return;
        }
        let mut inner = CStyleClassifier::new();
        if let Some(gt) = after_tag.find('>') {
            if after_tag[..gt].ends_with('/') {
                return; // self-closing
            }
            let body = &after_tag[gt + 1..];
            if !body.trim().is_empty() {
                inner.classify(body);
            }
        }
        self.embedded = Some((inner, close));
    }

    fn classify_markup(&mut self, trimmed: &str) -> LineType {
        if self.in_comment {
            if let Some(idx) = trimmed.find("-->") {
                // Check if there is code after comment end
                // For simplified classification, if a line has code mixed with comment end, we count as pure if it's not just comment.
                // But requirements say: "Classify lines containing both code and comments as LineType::Pure"
                // So if "--> <div>", it's Pure.
                // If "-->", it's Comment.
                let after = &trimmed[idx + 3..];
                if !after.trim().is_empty() {
                    self.in_comment = false;
                    return LineType::Pure;
                }
                self.in_comment = false;
                return LineType::Comment;
            }
            return LineType::Comment;
        }

        // Check for start of comment
        if let Some(start_idx) = trimmed.find("<!--") {
            // Check if it ends on same line
            if let Some(end_idx) = trimmed.find("-->") {
                if end_idx > start_idx {
                    // Full comment on one line.
                    // Check if there is code before or after.
                    let before = &trimmed[..start_idx];
                    let after = &trimmed[end_idx + 3..];
                    if !before.trim().is_empty() || !after.trim().is_empty() {
                        return LineType::Pure;
                    }
                    return LineType::Comment;
                }
            }
            // Starts but doesn't end
            let before = &trimmed[..start_idx];
            if !before.trim().is_empty() {
                self.in_comment = true;
                return LineType::Pure;
            }
            self.in_comment = true;
            return LineType::Comment;
        }

        LineType::Pure
    }
}

/// The line with every complete `<!-- ... -->` removed.
fn without_html_comments(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Splits file content into lines the way every mode reads them: lossy UTF-8 (Latin-1 text is
/// still code), without the line terminator, and without a leading byte-order mark.
pub fn text_lines(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes.split_inclusive(|&b| b == b'\n').map(|raw| {
        let line = String::from_utf8_lossy(raw);
        line.trim_end_matches(['\n', '\r'])
            .trim_start_matches('\u{feff}')
            .to_string()
    })
}

pub fn get_classifier(lang: Language) -> Box<dyn Classifier> {
    match lang {
        Language::Python => Box::new(PythonClassifier::new()),
        Language::TypeScript
        | Language::JavaScript
        | Language::C
        | Language::Cpp
        | Language::Csharp
        | Language::Java
        | Language::Php
        | Language::Swift
        | Language::Kotlin
        | Language::Scala
        | Language::Css => Box::new(CStyleClassifier::new()),
        Language::Rust => Box::new(CStyleClassifier::rust()),
        Language::Go => Box::new(CStyleClassifier::go()),
        Language::PowerShell => Box::new(PowerShellClassifier::default()),
        Language::Shell | Language::Yaml | Language::Toml => Box::new(ShellClassifier),
        Language::Ruby => Box::new(RubyClassifier::new()),
        Language::Html | Language::Vue => Box::new(HtmlClassifier::new()),
        Language::Other => Box::new(DefaultClassifier),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_classifier() {
        let mut c = PythonClassifier::new();
        assert_eq!(c.classify("x = 1"), LineType::Pure);
        assert_eq!(c.classify("# comment"), LineType::Comment);
        assert_eq!(c.classify("   "), LineType::Blank);
    }

    #[test]
    fn test_cstyle_pointer_deref_is_code() {
        let mut c = CStyleClassifier::new();
        assert_eq!(c.classify("*p = 1;"), LineType::Pure);
        // After code, a leading `*` is an expression continuation or a selector, not a comment.
        assert_eq!(c.classify("* tab_size as f64) as f32;"), LineType::Pure);
        assert_eq!(c.classify("* {"), LineType::Pure);
    }

    #[test]
    fn test_cstyle_hunk_starting_mid_block_comment() {
        let mut c = CStyleClassifier::new();
        assert_eq!(c.classify(" * continued doc line"), LineType::Comment);
        assert_eq!(c.classify(" * @param x"), LineType::Comment);
        assert_eq!(c.classify(" */"), LineType::Comment);
        assert_eq!(c.classify("fn f() {}"), LineType::Pure);
        // A hunk that starts with a CSS universal selector is code.
        assert_eq!(CStyleClassifier::new().classify("* {"), LineType::Pure);
        assert_eq!(
            CStyleClassifier::new().classify("* { margin: 0; }"),
            LineType::Pure
        );
    }

    #[test]
    fn test_js_strings_regexes_and_templates() {
        let mut c = CStyleClassifier::new();
        assert_eq!(c.classify("pathname: '/**',"), LineType::Pure);
        assert_eq!(c.classify("port: '',"), LineType::Pure); // not swallowed by a block
        assert_eq!(c.classify(r#"s.replace(/"""/g, '\\"');"#), LineType::Pure);
        assert_eq!(c.classify("// real comment"), LineType::Comment);
        assert_eq!(c.classify("const re = /`([^`]*)`/g;"), LineType::Pure);
        assert_eq!(c.classify("// still a comment"), LineType::Comment);
        assert_eq!(c.classify("const x = a / b / c;"), LineType::Pure);
        // Multi-line template literal: its lines are code even if they look like comments.
        assert_eq!(c.classify("const css = `"), LineType::Pure);
        assert_eq!(c.classify("/* not a comment */"), LineType::Pure);
        assert_eq!(c.classify("`;"), LineType::Pure);
        assert_eq!(c.classify("// comment again"), LineType::Comment);
        // Comments inside a multi-line `${...}` interpolation are comments.
        assert_eq!(c.classify("return `head ${join(["), LineType::Pure);
        assert_eq!(c.classify("  // why this entry"), LineType::Comment);
        assert_eq!(c.classify("  { a: 1 },"), LineType::Pure);
        assert_eq!(c.classify("])} tail`;"), LineType::Pure);
        assert_eq!(c.classify("// after"), LineType::Comment);
    }

    #[test]
    fn test_rust_raw_strings_and_lifetimes() {
        let mut c = CStyleClassifier::rust();
        assert_eq!(c.classify(r##"let s = r#"a "/*" b"#;"##), LineType::Pure);
        assert_eq!(c.classify("let a = 1;"), LineType::Pure);
        assert_eq!(c.classify("fn f<'a>(x: &'a str) {} // c"), LineType::Pure);
        assert_eq!(c.classify("let q = '\"'; // c"), LineType::Pure);
        assert_eq!(c.classify("// comment"), LineType::Comment);
    }

    #[test]
    fn test_python_docstrings_vs_data_strings() {
        let mut c = PythonClassifier::new();
        assert_eq!(c.classify("#!/usr/bin/env python3"), LineType::Pure);
        assert_eq!(
            c.classify(r#""""One-line docstring.""""#),
            LineType::Docstring
        );
        assert_eq!(c.classify(r#"r"""Raw docstring.""""#), LineType::Docstring);
        assert_eq!(c.classify(r#""""Doc.""".strip()"#), LineType::Pure);
        assert_eq!(c.classify(r#"""""#), LineType::Docstring);
        assert_eq!(c.classify("    # inside docstring"), LineType::Docstring);
        assert_eq!(c.classify(r#"""""#), LineType::Docstring);
        // Assigned or f-string triple-quoted strings are data, i.e. code.
        assert_eq!(c.classify(r#"query = f""""#), LineType::Pure);
        assert_eq!(c.classify("SELECT 1 -- # not a comment"), LineType::Pure);
        assert_eq!(c.classify(r#"""""#), LineType::Pure);
        assert_eq!(c.classify(r##"x = "#" # real comment"##), LineType::Pure);
        assert_eq!(c.classify(r#"s = '"""'"#), LineType::Pure);
        assert_eq!(c.classify("y = 2"), LineType::Pure); // not swallowed by a string
    }

    #[test]
    fn test_non_ascii_text_does_not_panic() {
        let mut py = PythonClassifier::new();
        assert_eq!(py.classify("x = 'a — b' # é"), LineType::Pure);
        assert_eq!(py.classify("café = 'midas' — rest"), LineType::Pure);
        assert_eq!(
            py.classify("serialised directly — no wrapper"),
            LineType::Pure
        );
        assert_eq!(
            py.classify(r#""""Adapter — real "x" impl.""""#),
            LineType::Docstring
        );
        assert_eq!(py.classify(r#"s = "\— ü""#), LineType::Pure);
        let mut c = CStyleClassifier::new();
        assert_eq!(c.classify("const s = '— /* ü */'; // ñ"), LineType::Pure);
        let mut h = HtmlClassifier::new();
        assert_eq!(h.classify("<p>Ünïcödé — <!-- ç --></p>"), LineType::Pure);
    }

    #[test]
    fn test_html_embedded_script_and_style() {
        let mut c = HtmlClassifier::new();
        assert_eq!(c.classify("<script>"), LineType::Pure);
        assert_eq!(c.classify("// js comment"), LineType::Comment);
        assert_eq!(c.classify("const a = '<!--';"), LineType::Pure);
        assert_eq!(c.classify("</script>"), LineType::Pure);
        assert_eq!(c.classify("<!-- html comment -->"), LineType::Comment);
        assert_eq!(c.classify(r#"<style lang="scss">"#), LineType::Pure);
        assert_eq!(c.classify("/* css */"), LineType::Comment);
        assert_eq!(c.classify("* { margin: 0; }"), LineType::Pure);
        assert_eq!(c.classify("</style>"), LineType::Pure);
        assert_eq!(
            c.classify(r#"<script src="x.js"></script>"#),
            LineType::Pure
        );
        assert_eq!(c.classify("// text, not script"), LineType::Pure);
    }

    #[test]
    fn test_go_raw_string_ending_in_backslash() {
        let mut c = CStyleClassifier::go();
        assert_eq!(c.classify(r"p := `C:\temp\`"), LineType::Pure);
        assert_eq!(c.classify("// real comment"), LineType::Comment);
        assert_eq!(
            c.classify("s := `${HOME} // not a comment`"),
            LineType::Pure
        );
        assert_eq!(c.classify("// comment"), LineType::Comment);
    }

    #[test]
    fn test_regex_after_keyword() {
        let mut c = CStyleClassifier::new();
        assert_eq!(c.classify("return /`/.test(s)"), LineType::Pure);
        assert_eq!(c.classify("// comment"), LineType::Comment);
        assert_eq!(c.classify("const n = total / count // c"), LineType::Pure);
        assert_eq!(c.classify("// comment"), LineType::Comment);
    }

    #[test]
    fn test_html_comment_mentioning_script() {
        let mut c = HtmlClassifier::new();
        assert_eq!(c.classify("<p><!-- <script> --></p>"), LineType::Pure);
        assert_eq!(c.classify("// text, not script"), LineType::Pure);
        assert_eq!(c.classify("<!-- <style> -->"), LineType::Comment);
        assert_eq!(c.classify("/* text */"), LineType::Pure);
    }

    #[test]
    fn test_python_strings_inside_brackets_are_data() {
        let mut c = PythonClassifier::new();
        for line in [
            "return text(",
            r#"    """"#,
            "    SELECT *",
            r#"    ""","#,
            ")",
        ] {
            assert_eq!(c.classify(line), LineType::Pure, "{line}");
        }
        assert_eq!(c.classify("x = 1 + \\"), LineType::Pure);
        assert_eq!(c.classify(r#"    """continued data""""#), LineType::Pure);
        assert_eq!(c.classify("def f():"), LineType::Pure);
        assert_eq!(
            c.classify(r#"    """Real docstring.""""#),
            LineType::Docstring
        );
    }

    #[test]
    fn test_nested_rust_comments_and_cpp_raw_strings() {
        let mut rs = CStyleClassifier::rust();
        assert_eq!(
            rs.classify("/* outer /* inner */ still comment"),
            LineType::Comment
        );
        assert_eq!(rs.classify("still comment */"), LineType::Comment);
        assert_eq!(rs.classify("let x = 1;"), LineType::Pure);
        let mut cpp = CStyleClassifier::new();
        assert_eq!(
            cpp.classify(r#"auto s = R"(/* not a comment"#),
            LineType::Pure
        );
        assert_eq!(cpp.classify(r#")";"#), LineType::Pure);
        assert_eq!(cpp.classify("int x;"), LineType::Pure);
        assert_eq!(cpp.classify(r#"auto t = u8R"x(a)" b)x";"#), LineType::Pure);
        assert_eq!(cpp.classify("// comment"), LineType::Comment);
        let mut kt = CStyleClassifier::new();
        assert_eq!(kt.classify(r#"val p = """C:\""""#), LineType::Pure);
        assert_eq!(kt.classify("// comment"), LineType::Comment);
    }

    #[test]
    fn test_html_tag_boundaries_and_same_line_scripts() {
        let mut c = HtmlClassifier::new();
        assert_eq!(c.classify("<style-guide>"), LineType::Pure);
        assert_eq!(c.classify(r#"<div data-x="<script">"#), LineType::Pure);
        assert_eq!(c.classify("<!-- still html -->"), LineType::Comment);
        assert_eq!(c.classify("<script>let a = 1; /* open"), LineType::Pure);
        assert_eq!(c.classify("inside js comment"), LineType::Comment);
        assert_eq!(c.classify("*/ </script><script>"), LineType::Pure);
        assert_eq!(c.classify("// js again"), LineType::Comment);
        assert_eq!(c.classify("</script>"), LineType::Pure);
        assert_eq!(c.classify(r#"<script src="a.js" />"#), LineType::Pure);
        assert_eq!(c.classify("<!-- html -->"), LineType::Comment);
    }

    #[test]
    fn test_powershell_block_comments() {
        let mut c = PowerShellClassifier::default();
        assert_eq!(c.classify("<#"), LineType::Comment);
        assert_eq!(c.classify(".SYNOPSIS"), LineType::Comment);
        assert_eq!(c.classify("#>"), LineType::Comment);
        assert_eq!(c.classify("Get-Item x # trailing"), LineType::Pure);
        assert_eq!(c.classify("<# one line #>"), LineType::Comment);
        assert_eq!(c.classify("# comment"), LineType::Comment);
    }

    #[test]
    fn test_long_lines_with_many_slashes_are_linear() {
        let line = format!("x = {}1;", "a/b+".repeat(50_000));
        let start = std::time::Instant::now();
        assert_eq!(CStyleClassifier::new().classify(&line), LineType::Pure);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn test_shebang_is_code() {
        assert_eq!(ShellClassifier.classify("#!/bin/sh"), LineType::Pure);
        assert_eq!(ShellClassifier.classify("# comment"), LineType::Comment);
    }

    #[test]
    fn test_cstyle_markers_in_strings_are_code() {
        let mut c = CStyleClassifier::new();
        assert_eq!(
            c.classify(r#"let g = vec!["**/*".to_string()];"#),
            LineType::Pure
        );
        assert_eq!(c.classify("let a = 1;"), LineType::Pure); // not swallowed as a block
        assert_eq!(c.classify(r#"let s = "// not a comment";"#), LineType::Pure);
        assert_eq!(c.classify(r#"let q = '"'; // trailing"#), LineType::Pure);
        assert_eq!(c.classify("x = 1; /* start"), LineType::Pure);
        assert_eq!(c.classify("still comment"), LineType::Comment);
        assert_eq!(c.classify("end */ y = 2;"), LineType::Pure);
        assert_eq!(c.classify("/* a */ /* b */"), LineType::Comment);
        assert_eq!(c.classify("fn f<'a>(x: &'a str) {}"), LineType::Pure);
    }

    #[test]
    fn test_html_classifier() {
        let mut c = HtmlClassifier::new();
        assert_eq!(c.classify("<div>"), LineType::Pure);
        assert_eq!(c.classify("<!-- comment -->"), LineType::Comment);
        assert_eq!(c.classify("<div> <!-- comment -->"), LineType::Pure);

        assert_eq!(c.classify("<!--"), LineType::Comment);
        assert_eq!(c.classify("inside"), LineType::Comment);
        assert_eq!(c.classify("-->"), LineType::Comment);

        // Mixed
        let mut c2 = HtmlClassifier::new();
        assert_eq!(c2.classify("<!--"), LineType::Comment);
        assert_eq!(c2.classify("--> <div>"), LineType::Pure);
    }
}
