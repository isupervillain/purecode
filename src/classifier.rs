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
    in_triple_double: bool,
    in_triple_single: bool,
}

impl PythonClassifier {
    pub fn new() -> Self {
        Self {
            in_triple_double: false,
            in_triple_single: false,
        }
    }
}

impl Classifier for PythonClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LineType::Blank;
        }

        if self.in_triple_double {
            if trimmed.contains("\"\"\"") {
                self.in_triple_double = false;
            }
            return LineType::Docstring;
        }
        if self.in_triple_single {
            if trimmed.contains("'''") {
                self.in_triple_single = false;
            }
            return LineType::Docstring;
        }

        if trimmed.starts_with('#') {
            return LineType::Comment;
        }

        if let Some(idx) = trimmed.find("\"\"\"") {
            if trimmed.matches("\"\"\"").count() >= 2 {
                // Single-line docstring. Check for code before or after.
                let before = &trimmed[..idx];
                let after = &trimmed[idx + 3..];
                if !before.trim().is_empty() || !after.trim().is_empty() {
                    return LineType::Pure;
                }
                return LineType::Docstring;
            } else {
                self.in_triple_double = true;
                return LineType::Docstring;
            }
        }

        if let Some(idx) = trimmed.find("'''") {
            if trimmed.matches("'''").count() >= 2 {
                // Single-line docstring. Check for code before or after.
                let before = &trimmed[..idx];
                let after = &trimmed[idx + 3..];
                if !before.trim().is_empty() || !after.trim().is_empty() {
                    return LineType::Pure;
                }
                return LineType::Docstring;
            } else {
                self.in_triple_single = true;
                return LineType::Docstring;
            }
        }

        LineType::Pure
    }
}

pub struct CStyleClassifier {
    in_block: bool,
}

impl CStyleClassifier {
    pub fn new() -> Self {
        Self { in_block: false }
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

        // Mid-block line of a hunk that started inside a comment (`* text`, `*`, `*/`), not `*ptr`.
        if !self.in_block
            && (trimmed == "*" || trimmed.starts_with("* ") || trimmed.starts_with("*/"))
        {
            return LineType::Comment;
        }

        // Scan so comment markers inside string/char literals (e.g. "**/*") are ignored.
        let chars: Vec<char> = trimmed.chars().collect();
        let mut has_code = false;
        let mut quote: Option<char> = None;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let next = chars.get(i + 1).copied();
            if self.in_block {
                if c == '*' && next == Some('/') {
                    self.in_block = false;
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
                self.in_block = true;
                i += 1;
            } else {
                if !c.is_whitespace() {
                    has_code = true;
                }
                let prev_is_ident =
                    i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
                if c == 'r' && !prev_is_ident && is_raw_string_start(&chars[i + 1..]) {
                    // Rust raw string r#"..."#: skip to its closer on this line (else rest of line).
                    let hashes = chars[i + 1..].iter().take_while(|&&x| x == '#').count();
                    let body = i + hashes + 2;
                    let closer: Vec<char> = std::iter::once('"')
                        .chain(std::iter::repeat_n('#', hashes))
                        .collect();
                    i = (body..chars.len())
                        .find(|&j| chars[j..].starts_with(&closer))
                        .map_or(chars.len(), |j| j + closer.len() - 1);
                } else if c == '"' || c == '`' {
                    quote = Some(c);
                } else if c == '\'' {
                    // Char literal ('"', '\n'); a lone `'` (lifetime) falls through as code.
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
        } else if trimmed.starts_with('#') {
            LineType::Comment
        } else {
            LineType::Pure
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

        if trimmed.starts_with('#') {
            return LineType::Comment;
        }

        if trimmed.starts_with("=begin") {
            self.in_block = true;
            return LineType::Comment;
        }

        LineType::Pure
    }
}

// Updated HTML/Vue Classifier to handle multi-line comments
pub struct HtmlClassifier {
    in_comment: bool,
}

impl HtmlClassifier {
    pub fn new() -> Self {
        Self { in_comment: false }
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

pub fn get_classifier(lang: Language) -> Box<dyn Classifier> {
    match lang {
        Language::Python => Box::new(PythonClassifier::new()),
        Language::TypeScript
        | Language::JavaScript
        | Language::C
        | Language::Cpp
        | Language::Csharp
        | Language::Java
        | Language::Go
        | Language::Php
        | Language::Swift
        | Language::Kotlin
        | Language::Scala
        | Language::Css
        | Language::Rust => Box::new(CStyleClassifier::new()),
        Language::Shell | Language::PowerShell | Language::Yaml | Language::Toml => {
            Box::new(ShellClassifier)
        }
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
        assert_eq!(c.classify("* continued doc line"), LineType::Comment);
        assert_eq!(c.classify("*/"), LineType::Comment);
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
