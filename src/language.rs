use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Html,
    Css,
    C,
    Cpp,
    Csharp,
    Java,
    Go,
    Php,
    Ruby,
    Swift,
    Kotlin,
    Scala,
    Shell,
    PowerShell,
    Vue,
    Other,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("py") => Language::Python,
            Some("js") | Some("jsx") | Some("mjs") => Language::JavaScript,
            Some("ts") | Some("tsx") => Language::TypeScript,
            Some("html") | Some("htm") => Language::Html,
            Some("css") | Some("scss") => Language::Css,
            Some("c") | Some("h") => Language::C,
            Some("cpp") | Some("hpp") | Some("cc") | Some("cxx") | Some("hh") => Language::Cpp,
            Some("cs") => Language::Csharp,
            Some("java") => Language::Java,
            Some("go") => Language::Go,
            Some("php") => Language::Php,
            Some("rb") => Language::Ruby,
            Some("swift") => Language::Swift,
            Some("kt") | Some("kts") => Language::Kotlin,
            Some("scala") | Some("sc") => Language::Scala,
            Some("sh") | Some("bash") | Some("zsh") => Language::Shell,
            Some("ps1") | Some("psm1") => Language::PowerShell,
            Some("vue") => Language::Vue,
            _ => {
                // Check filename for special cases
                match path.file_name().and_then(|n| n.to_str()) {
                    Some("Dockerfile") => Language::Other, // Or maybe shell-like? keeping Other for now
                    Some("Makefile") => Language::Other,
                    _ => Language::Other,
                }
            }
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Html => "HTML",
            Language::Css => "CSS",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Csharp => "C#",
            Language::Java => "Java",
            Language::Go => "Go",
            Language::Php => "PHP",
            Language::Ruby => "Ruby",
            Language::Swift => "Swift",
            Language::Kotlin => "Kotlin",
            Language::Scala => "Scala",
            Language::Shell => "Shell",
            Language::PowerShell => "PowerShell",
            Language::Vue => "Vue",
            Language::Other => "Other",
        };
        write!(f, "{}", s)
    }
}
