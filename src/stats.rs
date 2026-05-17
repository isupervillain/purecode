use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Default)]
pub struct AnalysisResult {
    pub summary: LangStats,
    pub language_stats: HashMap<String, LangStats>, // Keyed by Language::to_string()
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_stats: Option<Vec<FileStats>>,
    pub complexity_score: f64,
    pub token_estimate: u64,
    pub mode: String, // "diff" or "snapshot"
}

#[derive(Debug, Clone, Serialize)]
pub struct FileStats {
    pub path: String,
    pub language: String, // String for serialization, but internal logic uses Language
    pub lang_stats: LangStats,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
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
    #[must_use]
    pub fn net_pure(&self) -> i64 {
        self.pure_added - self.pure_removed
    }

    #[must_use]
    pub fn noise_added(&self) -> i64 {
        self.comment_lines_added + self.docstring_lines_added + self.blank_lines_added
    }

    #[must_use]
    pub fn noise_removed(&self) -> i64 {
        self.comment_lines_removed + self.docstring_lines_removed + self.blank_lines_removed
    }
}

#[must_use]
pub fn aggregate_stats(stats: &[FileStats]) -> LangStats {
    stats.iter().fold(LangStats::default(), |mut acc, file| {
        acc.total_added += file.lang_stats.total_added;
        acc.total_removed += file.lang_stats.total_removed;
        acc.pure_added += file.lang_stats.pure_added;
        acc.pure_removed += file.lang_stats.pure_removed;
        acc.comment_lines_added += file.lang_stats.comment_lines_added;
        acc.comment_lines_removed += file.lang_stats.comment_lines_removed;
        acc.docstring_lines_added += file.lang_stats.docstring_lines_added;
        acc.docstring_lines_removed += file.lang_stats.docstring_lines_removed;
        acc.blank_lines_added += file.lang_stats.blank_lines_added;
        acc.blank_lines_removed += file.lang_stats.blank_lines_removed;
        acc.code_words_added += file.lang_stats.code_words_added;
        acc.code_words_removed += file.lang_stats.code_words_removed;
        acc
    })
}

#[must_use]
pub fn calculate_complexity(stats: &LangStats) -> f64 {
    // complexity = pure_added * 1.0 + pure_removed * 0.5 + noise_added * 0.1 + noise_removed * 0.05
    (stats.pure_added as f64 * 1.0)
        + (stats.pure_removed as f64 * 0.5)
        + (stats.noise_added() as f64 * 0.1)
        + (stats.noise_removed() as f64 * 0.05)
}

#[must_use]
pub fn estimate_tokens(word_count: i64) -> u64 {
    (word_count as f64 * 1.3).round() as u64
}

#[derive(Debug)]
pub enum ThresholdError {
    NoiseRatioExceeded { actual: f64, max: f64 },
    MinPureLines { actual: i64, min: i64 },
    PureLinesDecreased { actual: i64 },
}

impl std::fmt::Display for ThresholdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThresholdError::NoiseRatioExceeded { actual, max } => {
                write!(f, "Noise ratio {:.2} exceeds limit {:.2}", actual, max)
            }
            ThresholdError::MinPureLines { actual, min } => {
                write!(f, "Net pure lines {} is less than minimum {}", actual, min)
            }
            ThresholdError::PureLinesDecreased { actual } => {
                write!(f, "Net pure code decreased ({})", actual)
            }
        }
    }
}
