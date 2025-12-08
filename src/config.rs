use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        "dist/**".to_string(),
        "target/**".to_string(),
        "node_modules/**".to_string(),
        ".git/**".to_string(),
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

pub fn load_config() -> Config {
    let path = Path::new(".purecode.toml");
    if path.exists() {
        match fs::read_to_string(path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => return config,
                Err(e) => eprintln!("Warning: Failed to parse .purecode.toml: {}", e),
            },
            Err(e) => eprintln!("Warning: Failed to read .purecode.toml: {}", e),
        }
    }
    Config::default()
}
