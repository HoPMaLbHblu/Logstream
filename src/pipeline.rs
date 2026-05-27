use crate::config::AppConfig;
use crate::log_entry::{LogEntry, LogLevel, ParseContext, parse_log_line};
use crate::rules::{Alert, RuleEngine};
use crate::stats::Stats;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Entry(LogEntry),
    Alert(Alert),
    Status(String),
    Error(String),
}

#[derive(Clone)]
pub struct AppChannels {
    events: broadcast::Sender<AppEvent>,
}

impl AppChannels {
    pub fn new(buffer: usize) -> Self {
        let (events, _) = broadcast::channel(buffer.max(64));
        Self { events }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    pub fn publish(&self, event: AppEvent) {
        let _ = self.events.send(event);
    }
}

#[derive(Clone)]
pub struct ShutdownToken {
    sender: Arc<watch::Sender<bool>>,
}

impl ShutdownToken {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.sender.subscribe()
    }

    pub fn cancel(&self) {
        let _ = self.sender.send(true);
    }
}

#[derive(Debug, Clone)]
pub struct RawLine {
    pub file: PathBuf,
    pub line_number: u64,
    pub text: String,
}

pub struct PipelineHandle {
    pub stats: Arc<Mutex<Stats>>,
    pub channels: AppChannels,
    pub shutdown: ShutdownToken,
    tasks: Vec<JoinHandle<()>>,
}

impl PipelineHandle {
    pub async fn stop(mut self) -> Stats {
        self.shutdown.cancel();
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
        self.stats.lock().expect("stats mutex poisoned").clone()
    }

    pub fn snapshot(&self) -> crate::stats::StatsSnapshot {
        self.stats.lock().expect("stats mutex poisoned").snapshot()
    }
}

pub async fn start_pipeline(
    files: Vec<PathBuf>,
    config: AppConfig,
    channels: AppChannels,
    shutdown: ShutdownToken,
) -> Result<PipelineHandle> {
    if files.is_empty() {
        anyhow::bail!("no log files were provided");
    }

    for file in &files {
        ensure_file(file).await?;
    }

    let stats = Arc::new(Mutex::new(Stats::new(config.retained_events)));
    let engine = Arc::new(Mutex::new(RuleEngine::new(config.rules.clone())));
    let (raw_tx, raw_rx) = mpsc::channel::<RawLine>(4096);
    let raw_rx = Arc::new(tokio::sync::Mutex::new(raw_rx));
    let mut tasks = Vec::new();

    for file in files {
        let tx = raw_tx.clone();
        let config = config.clone();
        let channels = channels.clone();
        let shutdown_rx = shutdown.subscribe();
        tasks.push(tokio::spawn(async move {
            if let Err(error) =
                tail_file(file.clone(), config, tx, shutdown_rx, channels.clone()).await
            {
                channels.publish(AppEvent::Error(format!(
                    "reader {} failed: {error:#}",
                    file.display()
                )));
            }
        }));
    }
    drop(raw_tx);

    for worker_id in 0..config.worker_count() {
        let rx = raw_rx.clone();
        let stats = stats.clone();
        let engine = engine.clone();
        let channels = channels.clone();
        let config = config.clone();
        let shutdown_rx = shutdown.subscribe();
        tasks.push(tokio::spawn(async move {
            worker_loop(worker_id, rx, stats, engine, channels, shutdown_rx, config).await;
        }));
    }

    channels.publish(AppEvent::Status("pipeline started".to_string()));
    Ok(PipelineHandle {
        stats,
        channels,
        shutdown,
        tasks,
    })
}

async fn ensure_file(path: &PathBuf) -> Result<()> {
    if path.exists() {
        File::open(path)
            .await
            .with_context(|| format!("cannot open {}", path.display()))?;
        return Ok(());
    } else if !path.exists() && path.parent().is_none() {
        anyhow::bail!(
            "path {} does not exist and has no parent directory",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    Ok(())
}

async fn tail_file(
    path: PathBuf,
    config: AppConfig,
    sender: mpsc::Sender<RawLine>,
    mut shutdown: watch::Receiver<bool>,
    channels: AppChannels,
) -> Result<()> {
    let mut position = if config.read_from_end {
        tokio::fs::metadata(&path).await?.len()
    } else {
        0
    };
    let mut line_number = 0;
    loop {
        if *shutdown.borrow() {
            break;
        }
        let mut file = File::open(&path).await?;
        if file.seek(SeekFrom::Start(position)).await.is_err() {
            position = 0;
            let _ = file.seek(SeekFrom::Start(0)).await;
        }
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();
        let mut read_any = false;
        loop {
            buffer.clear();
            let bytes = reader.read_line(&mut buffer).await?;
            if bytes == 0 {
                break;
            }
            read_any = true;
            position += bytes as u64;
            line_number += 1;
            if sender
                .send(RawLine {
                    file: path.clone(),
                    line_number,
                    text: buffer.clone(),
                })
                .await
                .is_err()
            {
                return Ok(());
            }
        }
        if !read_any {
            tokio::select! {
                _ = sleep(Duration::from_millis(config.poll_interval_ms.max(50))) => {}
                _ = shutdown.changed() => {}
            }
        }
    }
    channels.publish(AppEvent::Status(format!(
        "reader stopped for {}",
        path.display()
    )));
    Ok(())
}

async fn worker_loop(
    worker_id: usize,
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<RawLine>>>,
    stats: Arc<Mutex<Stats>>,
    engine: Arc<Mutex<RuleEngine>>,
    channels: AppChannels,
    mut shutdown: watch::Receiver<bool>,
    config: AppConfig,
) {
    channels.publish(AppEvent::Status(format!("worker {worker_id} started")));
    loop {
        let next = tokio::select! {
            value = async {
                let mut guard = receiver.lock().await;
                guard.recv().await
            } => value,
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    None
                } else {
                    continue;
                }
            }
        };
        let Some(raw) = next else {
            break;
        };
        stats
            .lock()
            .expect("stats mutex poisoned")
            .observe_raw_line();
        match parse_log_line(
            &raw.text,
            ParseContext {
                file: raw.file.clone(),
                line_number: raw.line_number,
            },
        ) {
            Ok(entry) => handle_entry(entry, &config, &stats, &engine, &channels),
            Err(error) => {
                stats
                    .lock()
                    .expect("stats mutex poisoned")
                    .observe_parse_error();
                channels.publish(AppEvent::Error(format!(
                    "parse error in {}:{}: {}",
                    raw.file.display(),
                    raw.line_number,
                    error
                )));
            }
        }
    }
    channels.publish(AppEvent::Status(format!("worker {worker_id} stopped")));
}

fn handle_entry(
    entry: LogEntry,
    config: &AppConfig,
    stats: &Arc<Mutex<Stats>>,
    engine: &Arc<Mutex<RuleEngine>>,
    channels: &AppChannels,
) {
    if !passes_filters(&entry, config) {
        stats
            .lock()
            .expect("stats mutex poisoned")
            .observe_ignored();
        return;
    }
    let decision = engine
        .lock()
        .expect("rule engine mutex poisoned")
        .evaluate(&entry);
    {
        let mut stats = stats.lock().expect("stats mutex poisoned");
        stats.set_rule_counts(engine.lock().expect("rule engine mutex poisoned").counts());
        if decision.ignored {
            stats.observe_ignored();
        } else {
            stats.observe_entry(entry.clone());
        }
        for alert in decision.alerts.iter().cloned() {
            stats.observe_alert(alert);
        }
    }
    if !decision.ignored {
        channels.publish(AppEvent::Entry(entry));
    }
    for alert in decision.alerts {
        channels.publish(AppEvent::Alert(alert));
    }
}

fn passes_filters(entry: &LogEntry, config: &AppConfig) -> bool {
    if let Some(levels) = config.filters.levels.as_ref() {
        let allowed = levels
            .iter()
            .filter_map(|level| LogLevel::from_str(level).ok())
            .any(|level| level == entry.level);
        if !allowed {
            return false;
        }
    }
    if let Some(keyword) = config.filters.keyword.as_deref() {
        if !entry.matches_keyword(keyword) {
            return false;
        }
    }
    if let Some(source) = config.filters.source.as_deref() {
        if entry.source_name() != source {
            return false;
        }
    }
    true
}
