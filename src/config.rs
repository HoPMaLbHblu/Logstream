use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub format: String,
    pub workers: usize,
    pub read_from_end: bool,
    pub poll_interval_ms: u64,
    pub retained_events: usize,
    pub filters: FilterConfig,
    pub rules: Vec<RuleConfig>,
    pub report: Option<ReportConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    pub levels: Option<BTreeSet<String>>,
    pub keyword: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    pub path: Option<PathBuf>,
    pub flush_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    pub name: String,
    pub condition: RuleCondition,
    pub action: RuleAction,
    pub threshold: usize,
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    pub level: Option<String>,
    pub keyword: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    Count,
    Warn,
    Ignore,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            format: "{timestamp?} {level} {message} {fields}".to_string(),
            workers: 4,
            read_from_end: false,
            poll_interval_ms: 500,
            retained_events: 500,
            filters: FilterConfig {
                levels: Some(
                    ["INFO", "WARN", "ERROR"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                ),
                keyword: None,
                source: None,
            },
            rules: vec![RuleConfig {
                name: "error-burst".to_string(),
                condition: RuleCondition {
                    level: Some("ERROR".to_string()),
                    keyword: None,
                    source: None,
                },
                action: RuleAction::Warn,
                threshold: 5,
                window_seconds: 30,
            }],
            report: Some(ReportConfig {
                path: Some(PathBuf::from("reports/logstream-report.txt")),
                flush_interval_seconds: 10,
            }),
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("cannot read config file {}", path.display()))?;
        match extension(path).as_str() {
            "json" => serde_json::from_str(&content).context("invalid JSON config"),
            "yaml" | "yml" => serde_yaml::from_str(&content).context("invalid YAML config"),
            "toml" => toml::from_str(&content).context("invalid TOML config"),
            other => Err(anyhow!("unsupported config extension: {other}")),
        }
    }

    pub fn load_optional(path: Option<&Path>) -> Result<Self> {
        match path {
            Some(path) => Self::load(path),
            None => Ok(Self::default()),
        }
    }

    pub fn write_default(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = Self::default();
        let text = match extension(path).as_str() {
            "json" => serde_json::to_string_pretty(&config)?,
            "yaml" | "yml" => serde_yaml::to_string(&config)?,
            "toml" => toml::to_string_pretty(&config)?,
            other => return Err(anyhow!("unsupported config extension: {other}")),
        };
        fs::write(path, text).with_context(|| format!("cannot write config {}", path.display()))?;
        Ok(())
    }

    pub fn worker_count(&self) -> usize {
        self.workers.clamp(1, 64)
    }
}

pub fn discover_config_path() -> Option<PathBuf> {
    [
        "log-config.json",
        "log-config.yaml",
        "log-config.yml",
        "log-config.toml",
        "log-congif.json",
        "log-congif.yaml",
        "log-congif.yml",
        "log-congif.toml",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.exists())
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("yaml")
        .to_ascii_lowercase()
}
