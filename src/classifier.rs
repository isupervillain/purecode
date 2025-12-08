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

        if trimmed.starts_with("\"\"\"") {
            let count = line.matches("\"\"\"").count();
            if count >= 2 {
                return LineType::Docstring;
            } else {
                self.in_triple_double = true;
                return LineType::Docstring;
            }
        }

        if trimmed.starts_with("'''") {
            let count = line.matches("'''").count();
            if count >= 2 {
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

        if self.in_block {
            if trimmed.contains("*/") {
                self.in_block = false;
            }
            return LineType::Comment;
        }

        if trimmed.starts_with("//") {
            return LineType::Comment;
        }

        if trimmed.starts_with('*') {
            return LineType::Comment;
        }

        if let Some(start_idx) = trimmed.find("/*") {
            if let Some(end_idx) = trimmed.find("*/") {
                if end_idx > start_idx {
                    return LineType::Comment;
                }
            }
            self.in_block = true;
            return LineType::Comment;
        }

        LineType::Pure
    }
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
        | Language::Css => Box::new(CStyleClassifier::new()),
        Language::Shell | Language::PowerShell => Box::new(ShellClassifier),
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
