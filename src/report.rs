use crate::stats::StatsSnapshot;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn write_report(path: &Path, snapshot: &StatsSnapshot) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, render_report(snapshot))
        .with_context(|| format!("cannot write report {}", path.display()))?;
    Ok(())
}

pub fn render_report(snapshot: &StatsSnapshot) -> String {
    let mut text = String::new();
    text.push_str("Logstream report\n");
    text.push_str("================\n\n");
    for line in snapshot.summary_lines() {
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str("\nRule counters:\n");
    for (rule, count) in &snapshot.rule_counts {
        text.push_str(&format!("  {rule}: {count}\n"));
    }
    text.push_str("\nRecent alerts:\n");
    for alert in snapshot.alerts.iter().rev().take(50) {
        text.push_str(&format!("  {}\n", alert.compact()));
    }
    text.push_str("\nRecent entries:\n");
    for entry in snapshot.recent_entries.iter().rev().take(50) {
        text.push_str(&format!("  {}\n", entry.compact()));
    }
    text
}
