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
    Rust,
    Yaml,
    Toml,
    Other,
}

impl Language {
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("py") | Some("pyi") => Language::Python,
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Language::JavaScript,
            Some("ts") | Some("tsx") | Some("mts") | Some("cts") => Language::TypeScript,
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
            Some("rs") => Language::Rust,
            Some("yml") | Some("yaml") => Language::Yaml,
            Some("toml") => Language::Toml,
            _ => {
                // Check filename for special cases
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name == "Dockerfile"
                    || name.starts_with("Dockerfile.")
                    || matches!(name, "Makefile" | "makefile" | "GNUmakefile")
                    || name.ends_with(".mk")
                {
                    Language::Shell
                } else {
                    Language::Other
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
            Language::Rust => "Rust",
            Language::Yaml => "YAML",
            Language::Toml => "TOML",
            Language::Other => "Other",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_variant_and_uppercase_extensions() {
        for (path, lang) in [
            ("a.cjs", Language::JavaScript),
            ("a.mts", Language::TypeScript),
            ("a.cts", Language::TypeScript),
            ("a.pyi", Language::Python),
            ("A.PY", Language::Python),
            ("makefile", Language::Shell),
            ("GNUmakefile", Language::Shell),
            ("README", Language::Other),
        ] {
            assert_eq!(Language::from_path(Path::new(path)), lang, "{path}");
        }
    }
}
