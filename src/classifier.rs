pub trait Classifier {
    /// Returns true if the line is considered "noise" (comment, docstring, or blank).
    /// This method is stateful for multi-line comments.
    fn is_noise(&mut self, line: &str) -> bool;
}

pub struct DefaultClassifier;

impl Classifier for DefaultClassifier {
    fn is_noise(&mut self, line: &str) -> bool {
        line.trim().is_empty()
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
    fn is_noise(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return true;
        }

        // If we are currently inside a triple-quote block, everything is noise
        // until we find the closing triple-quote.
        if self.in_triple_double {
            if trimmed.contains("\"\"\"") {
                // Potential end of block.
                // Naive check: if it contains the closer, we toggle off.
                // Real python parsing is harder (e.g. string literals),
                // but for diff stats this is usually sufficient.
                // We assume the closer is on this line.
                self.in_triple_double = false;
            }
            return true;
        }
        if self.in_triple_single {
            if trimmed.contains("'''") {
                self.in_triple_single = false;
            }
            return true;
        }

        // Check for comments
        if trimmed.starts_with('#') {
            return true;
        }

        // Check for start of docstrings
        if trimmed.starts_with("\"\"\"") {
            // Check if it opens and closes on the same line
            // e.g. """ doc """
            // Count occurrences.
            let count = line.matches("\"\"\"").count();
            if count >= 2 {
                // Open and close on same line -> it's a one-line docstring -> noise
                return true;
            } else {
                // Open, but not close -> enter state
                self.in_triple_double = true;
                return true;
            }
        }

        if trimmed.starts_with("'''") {
            let count = line.matches("'''").count();
            if count >= 2 {
                return true;
            } else {
                self.in_triple_single = true;
                return true;
            }
        }

        false
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

impl Classifier for CStyleClassifier {
    fn is_noise(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return true;
        }

        if self.in_block {
            if trimmed.contains("*/") {
                self.in_block = false;
            }
            return true;
        }

        if trimmed.starts_with("//") {
            return true;
        }

        // Check for Javadoc style continuations
        if trimmed.starts_with('*') {
            // This is risky for pointer dereferences in C/C++, e.g. *ptr = val;
            // But the spec says: "start with `*` (Javadoc-style continuation inside comment blocks)"
            // Usually Javadoc continuation happens *inside* a block.
            // But if we missed the start of the block (diff context issue), this rule helps catch the middle.
            // However, strictly following the prompt:
            // "Ignore lines that are: ... start with `*` (Javadoc-style continuation inside comment blocks)"
            // It implies we should treat lines starting with * as noise.
            // NOTE: This will produce false positives for pointer dereferences at start of line.
            // But for a "pure code" tool, minimizing noise is often prioritized, or it assumes consistent formatting.
            // We'll follow the spec.
            return true;
        }

        if let Some(start_idx) = trimmed.find("/*") {
            // If `/*` is found.
            // Check if `*/` is after `/*`.
            if let Some(end_idx) = trimmed.find("*/") {
                if end_idx > start_idx {
                    // Ends on same line. Treat as noise.
                    // Note: If there is code before `/*`, strictly speaking the line has code.
                    // The prompt says: "Ignore lines that are: ... inside /* ... */ comment blocks".
                    // And "When seeing /*: If a matching */ exists on the same line... treat the entire line as comment."
                    // This implies strict line-based classification. Even `int x = 1; /* comment */` might be classified as noise
                    // if we aren't careful?
                    // "Ignore lines that are ... inside /* ... */".
                    // "When seeing /*: If a matching */ exists on the same line ... treat the entire line as comment."
                    // This is aggressive. It means `code(); /* comment */` is counted as noise.
                    // I will follow this aggressive spec as requested ("treat the entire line as comment").
                    return true;
                }
            }
            // Starts block, doesn't end.
            self.in_block = true;
            return true;
        }

        false
    }
}

pub struct ShellClassifier;

impl Classifier for ShellClassifier {
    fn is_noise(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.is_empty() || trimmed.starts_with('#')
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

impl Classifier for RubyClassifier {
    fn is_noise(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return true;
        }

        if self.in_block {
            if trimmed.starts_with("=end") {
                self.in_block = false;
            }
            return true;
        }

        if trimmed.starts_with('#') {
            return true;
        }

        if trimmed.starts_with("=begin") {
            self.in_block = true;
            return true;
        }

        false
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
        assert!(!c.is_noise("x = 1"));
        assert!(c.is_noise("# comment"));
        assert!(c.is_noise("   "));

        // Multiline
        assert!(c.is_noise("\"\"\"")); // Start block
        assert!(c.is_noise("docs")); // Inside
        assert!(c.is_noise("\"\"\"")); // End block
        assert!(!c.is_noise("x = 2"));

        // One-liner
        assert!(c.is_noise("\"\"\" one line docs \"\"\""));
        assert!(!c.is_noise("y = 2"));
    }

    #[test]
    fn test_cstyle_classifier() {
        let mut c = CStyleClassifier::new();
        assert!(!c.is_noise("int x = 1;"));
        assert!(c.is_noise("// comment"));
        assert!(c.is_noise("   "));

        // Multiline
        assert!(c.is_noise("/*"));
        assert!(c.is_noise(" * inside"));
        assert!(c.is_noise("*/"));
        assert!(!c.is_noise("x = 2;"));

        // One-liner
        assert!(c.is_noise("/* comment */"));
        // Spec edge case: code with comment on same line is treated as comment
        assert!(c.is_noise("code(); /* comment */"));
    }

    #[test]
    fn test_ruby_classifier() {
        let mut c = RubyClassifier::new();
        assert!(!c.is_noise("x = 1"));
        assert!(c.is_noise("# comment"));

        assert!(c.is_noise("=begin"));
        assert!(c.is_noise("docs"));
        assert!(c.is_noise("=end"));
        assert!(!c.is_noise("y = 2"));
    }
}
