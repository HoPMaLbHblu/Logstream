use crate::config::AppConfig;
use crate::stats::StatsSnapshot;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub files: Vec<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub config: AppConfig,
    pub report_path: Option<PathBuf>,
    pub last_stats: Option<StatsSnapshot>,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    root: PathBuf,
}

impl Session {
    pub fn new(
        name: Option<String>,
        files: Vec<PathBuf>,
        config_path: Option<PathBuf>,
        config: AppConfig,
        report_path: Option<PathBuf>,
    ) -> Self {
        let now = Utc::now();
        let id = make_session_id();
        Self {
            id: id.clone(),
            name: name.unwrap_or_else(|| id.clone()),
            created_at: now,
            updated_at: now,
            files,
            config_path,
            config,
            report_path,
            last_stats: None,
        }
    }

    pub fn set_stats(&mut self, snapshot: StatsSnapshot) {
        self.last_stats = Some(snapshot);
        self.updated_at = Utc::now();
    }
}

impl SessionStore {
    pub fn default() -> Result<Self> {
        let root = PathBuf::from(".logstream-sessions");
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        let path = self.path_for(&session.name);
        let text = serde_json::to_string_pretty(session)?;
        fs::write(&path, text)
            .with_context(|| format!("cannot write session {}", path.display()))?;
        Ok(())
    }

    pub fn load(&self, name: &str) -> Result<Session> {
        let path = self.path_for(name);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("cannot read session {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("invalid session {}", path.display()))
    }

    pub fn list(&self) -> Result<Vec<Session>> {
        fs::create_dir_all(&self.root)?;
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let text = fs::read_to_string(&path)?;
            if let Ok(session) = serde_json::from_str::<Session>(&text) {
                sessions.push(session);
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.root.join(format!("{}.json", sanitize_name(name)))
    }
}

fn sanitize_name(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            result.push(ch);
        } else {
            result.push('-');
        }
    }
    if result.is_empty() {
        "session".to_string()
    } else {
        result
    }
}

fn make_session_id() -> String {
    let now = Utc::now();
    let nanos = now
        .timestamp_nanos_opt()
        .unwrap_or_else(|| now.timestamp_micros() * 1000);
    format!("session-{nanos}-{}", std::process::id())
}
