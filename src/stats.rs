use crate::log_entry::{LogEntry, LogLevel};
use crate::rules::Alert;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_lines_seen: u64,
    pub total_entries_kept: u64,
    pub parse_errors: u64,
    pub ignored_entries: u64,
    pub by_level: BTreeMap<LogLevel, u64>,
    pub by_source: BTreeMap<String, u64>,
    pub by_service: BTreeMap<String, u64>,
    pub by_file: BTreeMap<String, u64>,
    pub rule_counts: BTreeMap<String, usize>,
    pub recent_entries: Vec<LogEntry>,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone)]
pub struct Stats {
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total_lines_seen: u64,
    total_entries_kept: u64,
    parse_errors: u64,
    ignored_entries: u64,
    by_level: BTreeMap<LogLevel, u64>,
    by_source: BTreeMap<String, u64>,
    by_service: BTreeMap<String, u64>,
    by_file: BTreeMap<String, u64>,
    rule_counts: BTreeMap<String, usize>,
    recent_entries: VecDeque<LogEntry>,
    alerts: VecDeque<Alert>,
    retained_events: usize,
}

impl Stats {
    pub fn new(retained_events: usize) -> Self {
        let now = Utc::now();
        Self {
            started_at: now,
            updated_at: now,
            total_lines_seen: 0,
            total_entries_kept: 0,
            parse_errors: 0,
            ignored_entries: 0,
            by_level: BTreeMap::new(),
            by_source: BTreeMap::new(),
            by_service: BTreeMap::new(),
            by_file: BTreeMap::new(),
            rule_counts: BTreeMap::new(),
            recent_entries: VecDeque::new(),
            alerts: VecDeque::new(),
            retained_events: retained_events.clamp(50, 10_000),
        }
    }

    pub fn observe_raw_line(&mut self) {
        self.total_lines_seen += 1;
        self.updated_at = Utc::now();
    }

    pub fn observe_parse_error(&mut self) {
        self.parse_errors += 1;
        self.updated_at = Utc::now();
    }
    pub fn observe_ignored(&mut self) {
        self.ignored_entries += 1;
        self.updated_at = Utc::now();
    }

    pub fn observe_entry(&mut self, entry: LogEntry) {
        self.total_entries_kept += 1;
        *self.by_level.entry(entry.level).or_default() += 1;
        *self.by_source.entry(entry.source_name()).or_default() += 1;
        if let Some(service) = entry.service() {
            *self.by_service.entry(service.to_string()).or_default() += 1;
        }
        *self
            .by_file
            .entry(entry.file.display().to_string())
            .or_default() += 1;
        self.recent_entries.push_back(entry);
        while self.recent_entries.len() > self.retained_events {
            self.recent_entries.pop_front();
        }
        self.updated_at = Utc::now();
    }

    pub fn observe_alert(&mut self, alert: Alert) {
        self.alerts.push_back(alert);
        while self.alerts.len() > self.retained_events {
            self.alerts.pop_front();
        }
        self.updated_at = Utc::now();
    }
    pub fn set_rule_counts(&mut self, counts: BTreeMap<String, usize>) {
        self.rule_counts = counts;
        self.updated_at = Utc::now();
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            started_at: self.started_at,
            updated_at: self.updated_at,
            total_lines_seen: self.total_lines_seen,
            total_entries_kept: self.total_entries_kept,
            parse_errors: self.parse_errors,
            ignored_entries: self.ignored_entries,
            by_level: self.by_level.clone(),
            by_source: self.by_source.clone(),
            by_service: self.by_service.clone(),
            by_file: self.by_file.clone(),
            rule_counts: self.rule_counts.clone(),
            recent_entries: self.recent_entries.iter().cloned().collect(),
            alerts: self.alerts.iter().cloned().collect(),
        }
    }
}

impl StatsSnapshot {
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("started_at: {}", self.started_at));
        lines.push(format!("updated_at: {}", self.updated_at));
        lines.push(format!("total_lines_seen: {}", self.total_lines_seen));
        lines.push(format!("total_entries_kept: {}", self.total_entries_kept));
        lines.push(format!("parse_errors: {}", self.parse_errors));
        lines.push(format!("ignored_entries: {}", self.ignored_entries));
        lines.push("levels:".to_string());
        for (level, count) in &self.by_level {
            lines.push(format!("  {level}: {count}"));
        }
        lines.push("sources:".to_string());
        for (source, count) in &self.by_source {
            lines.push(format!("  {source}: {count}"));
        }
        lines
    }
}
