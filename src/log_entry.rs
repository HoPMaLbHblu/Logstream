use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
    Trace,
    Unknown,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
            LogLevel::Unknown => "UNKNOWN",
        }
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LogLevel {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "INFO" => Ok(LogLevel::Info),
            "WARN" | "WARNING" => Ok(LogLevel::Warn),
            "ERROR" | "ERR" => Ok(LogLevel::Error),
            "DEBUG" => Ok(LogLevel::Debug),
            "TRACE" => Ok(LogLevel::Trace),
            "" => Err(()),
            _ => Ok(LogLevel::Unknown),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub original_timestamp: Option<String>,
    pub level: LogLevel,
    pub message: String,
    pub fields: BTreeMap<String, String>,
    pub source: Option<String>,
    pub file: PathBuf,
    pub line_number: u64,
    pub raw: String,
}

impl LogEntry {
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    pub fn service(&self) -> Option<&str> {
        self.field("service")
    }

    pub fn source_name(&self) -> String {
        self.source
            .clone()
            .or_else(|| self.field("source").map(ToOwned::to_owned))
            .unwrap_or_else(|| self.file.display().to_string())
    }

    pub fn compact(&self) -> String {
        let source = self.source_name();
        let fields = self
            .fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        if fields.is_empty() {
            format!(
                "[{}] {} {}: {}",
                self.timestamp, source, self.level, self.message
            )
        } else {
            format!(
                "[{}] {} {}: {} {}",
                self.timestamp, source, self.level, self.message, fields
            )
        }
    }

    pub fn matches_keyword(&self, keyword: &str) -> bool {
        if keyword.trim().is_empty() {
            return true;
        }
        let needle = keyword.to_ascii_lowercase();
        self.message.to_ascii_lowercase().contains(&needle)
            || self.raw.to_ascii_lowercase().contains(&needle)
            || self.fields.iter().any(|(key, value)| {
                key.to_ascii_lowercase().contains(&needle)
                    || value.to_ascii_lowercase().contains(&needle)
            })
    }
}
