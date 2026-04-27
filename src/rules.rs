use crate::config::{RuleAction, RuleCondition, RuleConfig};
use crate::log_entry::{LogEntry, LogLevel};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub rule: String,
    pub action: RuleAction,
    pub message: String,
    pub source: String,
    pub level: LogLevel,
    pub count: usize,
    pub threshold: usize,
    pub window_seconds: u64,
    pub timestamp: DateTime<Utc>,
}

impl Alert {
    pub fn compact(&self) -> String {
        format!(
            "{} [{}] {}: {} ({}/{}, {}s)",
            self.timestamp,
            self.level,
            self.rule,
            self.message,
            self.count,
            self.threshold,
            self.window_seconds
        )
    }
}

#[derive(Debug, Clone)]
pub struct RuleState {
    config: RuleConfig,
    hits: BTreeMap<String, VecDeque<DateTime<Utc>>>,
    total_hits: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RuleEngine {
    rules: Vec<RuleState>,
    counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default)]
pub struct RuleDecision {
    pub ignored: bool,
    pub alerts: Vec<Alert>,
}

impl RuleEngine {
    pub fn new(rules: Vec<RuleConfig>) -> Self {
        Self {
            rules: rules
                .into_iter()
                .map(|config| RuleState {
                    config,
                    hits: BTreeMap::new(),
                    total_hits: 0,
                })
                .collect(),
            counts: BTreeMap::new(),
        }
    }

    pub fn evaluate(&mut self, entry: &LogEntry) -> RuleDecision {
        let mut decision = RuleDecision::default();
        for state in &mut self.rules {
            if !condition_matches(&state.config.condition, entry) {
                continue;
            }

            state.total_hits += 1;
            *self.counts.entry(state.config.name.clone()).or_default() += 1;

            match state.config.action {
                RuleAction::Ignore => {
                    decision.ignored = true;
                }
                RuleAction::Count => {}
                RuleAction::Warn => {
                    let key = fingerprint(&state.config.condition, entry);
                    let cutoff =
                        entry.timestamp - Duration::seconds(state.config.window_seconds as i64);
                    let queue = state.hits.entry(key).or_default();
                    queue.push_back(entry.timestamp);
                    while queue.front().map(|value| *value < cutoff).unwrap_or(false) {
                        queue.pop_front();
                    }
                    let threshold = state.config.threshold.max(1);
                    if queue.len() >= threshold {
                        decision.alerts.push(Alert {
                            rule: state.config.name.clone(),
                            action: state.config.action.clone(),
                            message: format!("rule matched {} events in window", queue.len()),
                            source: entry.source_name(),
                            level: entry.level,
                            count: queue.len(),
                            threshold,
                            window_seconds: state.config.window_seconds,
                            timestamp: Utc::now(),
                        });
                        while queue.len() > threshold.saturating_sub(1) {
                            queue.pop_front();
                        }
                    }
                }
            }
        }
        decision
    }

    pub fn counts(&self) -> BTreeMap<String, usize> {
        self.counts.clone()
    }
}

fn condition_matches(condition: &RuleCondition, entry: &LogEntry) -> bool {
    if let Some(level) = condition.level.as_deref() {
        let Ok(expected) = LogLevel::from_str(level) else {
            return false;
        };
        if entry.level != expected {
            return false;
        }
    }
    if let Some(keyword) = condition.keyword.as_deref() {
        if !entry.matches_keyword(keyword) {
            return false;
        }
    }
    if let Some(source) = condition.source.as_deref() {
        if entry.source_name() != source {
            return false;
        }
    }
    true
}

fn fingerprint(condition: &RuleCondition, entry: &LogEntry) -> String {
    let level = condition
        .level
        .clone()
        .unwrap_or_else(|| entry.level.to_string());
    let source = condition
        .source
        .clone()
        .unwrap_or_else(|| entry.source_name());
    let keyword = condition.keyword.clone().unwrap_or_else(|| {
        entry
            .message
            .split_whitespace()
            .take(8)
            .collect::<Vec<_>>()
            .join(" ")
    });
    format!("{level}|{source}|{keyword}")
}
