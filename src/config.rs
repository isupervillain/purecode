use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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

/// Loads `.purecode.toml` from the working directory, or defaults when it is absent.
///
/// A config that cannot be read or parsed is an error: silently falling back to defaults
/// would disable the thresholds a CI gate relies on.
pub fn load_config() -> Result<Config, String> {
    let path = Path::new(".purecode.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read .purecode.toml: {e}"))?;
    parse_config(&content).map_err(|e| format!("Invalid .purecode.toml: {e}"))
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
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_excludes_match_at_any_depth() {
        let excludes: Vec<glob::Pattern> = default_exclude()
            .iter()
            .map(|p| glob::Pattern::new(p).unwrap())
            .collect();
        let hit = |path: &str| excludes.iter().any(|p| p.matches(path));
        assert!(hit("target/debug/x"));
        assert!(hit("web/node_modules/a/b.js"));
        assert!(hit(".git/config"));
        assert!(!hit("src/main.rs"));
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
    }
}
