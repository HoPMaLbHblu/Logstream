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

#[derive(Debug, Clone)]
pub struct ParseContext {
    pub file: PathBuf,
    pub line_number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseProblem {
    EmptyLine,
    MissingLevel,
}

impl Display for ParseProblem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseProblem::EmptyLine => f.write_str("empty log line"),
            ParseProblem::MissingLevel => f.write_str("log line does not contain level"),
        }
    }
}

impl std::error::Error for ParseProblem {}

pub fn parse_log_line(raw: &str, ctx: ParseContext) -> Result<LogEntry, ParseProblem> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Err(ParseProblem::EmptyLine);
    }

    let tokens = split_preserving_quotes(trimmed);

    if tokens.is_empty() {
        return Err(ParseProblem::EmptyLine);
    }

    let mut timestamp = Utc::now();
    let mut original_timestamp = None;
    let mut cursor = 0;

    if let Ok(parsed) = DateTime::parse_from_rfc3339(&tokens[0]) {
        timestamp = parsed.with_timezone(&Utc);
        original_timestamp = Some(tokens[0].clone());
        cursor += 1;
    }

    if cursor >= tokens.len() {
        return Err(ParseProblem::MissingLevel);
    }

    let level = LogLevel::from_str(&tokens[cursor]).map_err(|_| ParseProblem::MissingLevel)?;
    cursor += 1;

    let mut fields = BTreeMap::new();
    let mut message_parts = Vec::new();

    while cursor < tokens.len() {
        let token = &tokens[cursor];

        if let Some((key, value)) = token.split_once('=') {
            if !key.trim().is_empty() {
                fields.insert(
                    key.trim().to_string(),
                    strip_quotes(value.trim()).to_string(),
                );
            }
        } else {
            message_parts.push(strip_quotes(token).to_string());
        }

        cursor += 1;
    }

    let source = fields
        .get("source")
        .cloned()
        .or_else(|| fields.get("service").cloned());

    Ok(LogEntry {
        timestamp,
        original_timestamp,
        level,
        message: message_parts.join(" "),
        fields,
        source,
        file: ctx.file,
        line_number: ctx.line_number,
        raw: trimmed.to_string(),
    })
}

fn split_preserving_quotes(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for ch in input.chars() {
        match quote {
            Some(active) if ch == active => {
                quote = None;
                current.push(ch);
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn strip_quotes(value: &str) -> &str {
    let bytes = value.as_bytes();

    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];

        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }

    value
}
