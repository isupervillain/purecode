#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LineType {
    Pure,
    Comment,
    Docstring,
    Blank,
}

pub trait Classifier {
    /// Classifies the line as Pure, Comment, Docstring, or Blank.
    /// This method is stateful for multi-line comments.
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

impl Default for PythonClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Classifier for PythonClassifier {
    fn classify(&mut self, line: &str) -> LineType {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return LineType::Blank;
        }

        // If we are currently inside a triple-quote block
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

        // Check for comments
        if trimmed.starts_with('#') {
            return LineType::Comment;
        }

        // Check for start of docstrings
        if trimmed.starts_with("\"\"\"") {
            let count = line.matches("\"\"\"").count();
            if count >= 2 {
                // Open and close on same line -> one-line docstring
                return LineType::Docstring;
            } else {
                // Open, but not close -> enter state
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

        // Check for Javadoc style continuations
        if trimmed.starts_with('*') {
            return LineType::Comment;
        }

        if let Some(start_idx) = trimmed.find("/*") {
            if let Some(end_idx) = trimmed.find("*/") {
                if end_idx > start_idx {
                    // Ends on same line.
                    return LineType::Comment;
                }
            }
            // Starts block, doesn't end.
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

pub fn get_classifier(lang: &str) -> Box<dyn Classifier> {
    match lang {
        "Python" => Box::new(PythonClassifier::new()),
        "TypeScript" | "JavaScript" | "C" | "C++" | "C#" | "Java" | "Go" | "PHP" | "Swift"
        | "Kotlin" | "Scala" => Box::new(CStyleClassifier::new()),
        "Shell" | "PowerShell" => Box::new(ShellClassifier), // PowerShell uses # for comments too
        "Ruby" => Box::new(RubyClassifier::new()),
        _ => Box::new(DefaultClassifier),
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

        // Multiline docstring
        assert_eq!(c.classify("\"\"\""), LineType::Docstring); // Start block
        assert_eq!(c.classify("docs"), LineType::Docstring); // Inside
        assert_eq!(c.classify("\"\"\""), LineType::Docstring); // End block
        assert_eq!(c.classify("x = 2"), LineType::Pure);

        // One-liner
        assert_eq!(c.classify("\"\"\" one line docs \"\"\""), LineType::Docstring);
        assert_eq!(c.classify("y = 2"), LineType::Pure);
    }

    #[test]
    fn test_cstyle_classifier() {
        let mut c = CStyleClassifier::new();
        assert_eq!(c.classify("int x = 1;"), LineType::Pure);
        assert_eq!(c.classify("// comment"), LineType::Comment);
        assert_eq!(c.classify("   "), LineType::Blank);

        // Multiline
        assert_eq!(c.classify("/*"), LineType::Comment);
        assert_eq!(c.classify(" * inside"), LineType::Comment);
        assert_eq!(c.classify("*/"), LineType::Comment);
        assert_eq!(c.classify("x = 2;"), LineType::Pure);

        // One-liner
        assert_eq!(c.classify("/* comment */"), LineType::Comment);
        assert_eq!(c.classify("code(); /* comment */"), LineType::Comment);
    }

    #[test]
    fn test_ruby_classifier() {
        let mut c = RubyClassifier::new();
        assert_eq!(c.classify("x = 1"), LineType::Pure);
        assert_eq!(c.classify("# comment"), LineType::Comment);
        assert_eq!(c.classify(""), LineType::Blank);

        assert_eq!(c.classify("=begin"), LineType::Comment);
        assert_eq!(c.classify("docs"), LineType::Comment);
        assert_eq!(c.classify("=end"), LineType::Comment);
        assert_eq!(c.classify("y = 2"), LineType::Pure);
    }
}
