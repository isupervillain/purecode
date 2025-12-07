pub mod classifier;
pub mod diff;
pub mod parser;
pub mod report;
pub mod stats;

/// Detects the programming language based on the file extension.
///
/// Returns "Other" if the extension is unknown.
pub fn detect_language(path: &str) -> String {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    match extension {
        "py" => "Python",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "c" | "h" => "C",
        "cpp" | "cc" | "hpp" | "hh" => "C++",
        "cs" => "C#",
        "java" => "Java",
        "go" => "Go",
        "swift" => "Swift",
        "kt" | "kts" => "Kotlin",
        "php" => "PHP",
        "scala" => "Scala",
        "rb" => "Ruby",
        "sh" | "bash" | "zsh" => "Shell",
        "ps1" => "PowerShell",
        "css" | "scss" => "CSS",
        "html" | "htm" => "HTML",
        "vue" => "Vue",
        _ => "Other",
    }
    .to_string()
}
