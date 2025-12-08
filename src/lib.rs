pub mod classifier;
pub mod config;
pub mod diff;
pub mod files;
pub mod language;
pub mod parser;
pub mod report;
pub mod stats;

#[cfg(test)]
mod tests;

pub fn detect_language(path: &str) -> String {
    language::Language::from_path(std::path::Path::new(path)).to_string()
}
