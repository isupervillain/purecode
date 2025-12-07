#[derive(Debug, Default, Clone)]
pub struct LangStats {
    pub total_added: i64,
    pub total_removed: i64,

    pub pure_added: i64,
    pub pure_removed: i64,

    pub comment_lines_added: i64,
    pub comment_lines_removed: i64,

    pub docstring_lines_added: i64,
    pub docstring_lines_removed: i64,

    pub blank_lines_added: i64,
    pub blank_lines_removed: i64,

    pub code_words_added: i64,
    pub code_words_removed: i64,
}

impl LangStats {
    pub fn net_total(&self) -> i64 {
        self.total_added - self.total_removed
    }

    pub fn net_pure(&self) -> i64 {
        self.pure_added - self.pure_removed
    }

    pub fn estimated_tokens_added(&self) -> i64 {
        (self.code_words_added as f64 * 1.3).round() as i64
    }

    pub fn estimated_tokens_removed(&self) -> i64 {
        (self.code_words_removed as f64 * 1.3).round() as i64
    }
}
