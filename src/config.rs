use glob::{MatchOptions, Pattern};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_base")]
    pub base: String,
    #[serde(default = "default_format")]
    pub format: String,
    pub max_noise_ratio: Option<f64>,
    pub min_pure_lines: Option<i64>,
    #[serde(default)]
    pub fail_on_decrease: bool,
    #[serde(default)]
    pub warn_only: bool,
    #[serde(default)]
    pub ci: bool,
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
}

fn default_base() -> String {
    "origin/main".to_string()
}
fn default_format() -> String {
    "human".to_string()
}
fn default_include() -> Vec<String> {
    vec!["**/*".to_string()]
}
fn default_exclude() -> Vec<String> {
    vec![
        "**/*.lock".to_string(),
        "**/package-lock.json".to_string(),
        "**/pnpm-lock.yaml".to_string(),
        "**/dist/**".to_string(),
        "**/target/**".to_string(),
        "**/node_modules/**".to_string(),
        "**/.git/**".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base: default_base(),
            format: default_format(),
            max_noise_ratio: None,
            min_pure_lines: None,
            fail_on_decrease: false,
            warn_only: false,
            ci: false,
            include: default_include(),
            exclude: default_exclude(),
        }
    }
}

/// Finds `.purecode.toml` in the working directory or a parent, up to the repository root, and
/// returns the config with the directory it applies to (the project root). Without a config
/// file, the defaults apply to the working directory.
///
/// A config that cannot be read or parsed is an error: silently falling back to defaults
/// would disable the thresholds a CI gate relies on.
pub fn load_config() -> Result<(Config, PathBuf), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("Cannot read working directory: {e}"))?;
    for dir in cwd.ancestors() {
        let path = dir.join(".purecode.toml");
        if path.is_file() {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let config =
                parse_config(&content).map_err(|e| format!("Invalid {}: {e}", path.display()))?;
            return Ok((config, dir.to_path_buf()));
        }
        if dir.join(".git").exists() {
            break;
        }
    }
    Ok((Config::default(), cwd))
}

/// Accepts keys at the top level or under a `[purecode]` table.
fn parse_config(content: &str) -> Result<Config, String> {
    let mut table: toml::Table = toml::from_str(content).map_err(|e| e.to_string())?;
    if table.len() == 1 {
        if let Some(toml::Value::Table(inner)) = table.remove("purecode") {
            table = inner;
        }
    }
    let config: Config = toml::Value::Table(table)
        .try_into()
        .map_err(|e: toml::de::Error| e.to_string())?;
    if !["human", "plain", "json"].contains(&config.format.as_str()) {
        return Err(format!(
            "format must be human, plain or json, got '{}'",
            config.format
        ));
    }
    if let Some(r) = config.max_noise_ratio {
        check_ratio(r).map_err(|e| format!("max_noise_ratio: {e}"))?;
    }
    PathFilter::new(&config.include, &config.exclude)?;
    Ok(config)
}

/// A noise ratio threshold must be a number from 0.0 to 1.0.
pub fn check_ratio(value: f64) -> Result<f64, String> {
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(format!("must be between 0.0 and 1.0, got {value}"))
    }
}

/// Include/exclude globs, matched against paths relative to the project root. `*` stays within
/// one directory; `**/` matches at any depth.
pub struct PathFilter {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

impl PathFilter {
    pub fn new(include: &[String], exclude: &[String]) -> Result<Self, String> {
        let compile = |globs: &[String]| {
            globs
                .iter()
                .map(|g| Pattern::new(g).map_err(|e| format!("invalid glob '{g}': {e}")))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            include: compile(include)?,
            exclude: compile(exclude)?,
        })
    }

    pub fn selects(&self, rel: &Path) -> bool {
        let opts = MatchOptions {
            require_literal_separator: true,
            ..MatchOptions::new()
        };
        let hit = |p: &Pattern| p.matches_path_with(rel, opts);
        self.include.iter().any(hit) && !self.exclude.iter().any(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_filter_excludes_at_any_depth() {
        let f = PathFilter::new(&default_include(), &default_exclude()).unwrap();
        for excluded in [
            "target/debug/x",
            "web/node_modules/a/b.js",
            ".git/config",
            "a/b.lock",
        ] {
            assert!(!f.selects(Path::new(excluded)), "{excluded}");
        }
        assert!(f.selects(Path::new("src/main.rs")));
        assert!(f.selects(Path::new("Cargo.toml")));
    }

    #[test]
    fn single_star_stays_within_a_directory() {
        let f = PathFilter::new(&default_include(), &["*.py".to_string()]).unwrap();
        assert!(!f.selects(Path::new("q.py")));
        assert!(f.selects(Path::new("sub/q.py")));
    }

    #[test]
    fn config_keys_at_top_level_or_under_purecode_table() {
        let top = parse_config("base = \"main\"\nmax_noise_ratio = 0.5\n").unwrap();
        let table = parse_config("[purecode]\nbase = \"main\"\nmax_noise_ratio = 0.5\n").unwrap();
        for c in [top, table] {
            assert_eq!(c.base, "main");
            assert_eq!(c.max_noise_ratio, Some(0.5));
            assert!(c.exclude.contains(&"**/node_modules/**".to_string()));
        }
    }

    #[test]
    fn invalid_config_is_rejected() {
        assert!(parse_config("max_noise_ratoi = 0.5").is_err()); // typo
        assert!(parse_config("format = \"xml\"").is_err());
        assert!(parse_config("base = ").is_err());
        assert!(parse_config("include = [\"src/[\"]").is_err());
        assert!(parse_config("max_noise_ratio = 5.0").is_err());
        assert!(parse_config("max_noise_ratio = nan").is_err());
        assert!(parse_config("max_noise_ratio = -0.1").is_err());
    }
}
