use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use logstream::config::{AppConfig, discover_config_path};
use logstream::pipeline::{AppChannels, AppEvent, ShutdownToken, start_pipeline};
use logstream::report::{render_report, write_report};
use logstream::session::{Session, SessionStore};
use logstream::ui::run_tui;
use std::path::PathBuf;
#[derive(Debug, Parser)]
#[command(name = "logstream")]
#[command(about = "Realtime streaming log analyzer")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Start(StartArgs),
    Stats(StatsArgs),
    Sessions,
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Args)]
struct StartArgs {
    #[arg(value_name = "FILES")]
    files: Vec<PathBuf>,
    #[arg(short, long)]
    config: Option<PathBuf>,
    #[arg(short, long)]
    session: Option<String>,
    #[arg(long)]
    report: Option<PathBuf>,
    #[arg(long)]
    no_tui: bool,
}

#[derive(Debug, Args)]
struct StatsArgs {
    #[arg(short, long)]
    session: String,
    #[arg(long)]
    report: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Show { path: Option<PathBuf> },
    Init { path: PathBuf },
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start(args) => {
            start_command(args).await?;
        }
        Command::Stats(args) => {
            stats_command(args)?;
        }
        Command::Sessions => {
            sessions_command()?;
        }
        Command::Config { command } => {
            config_command(command)?;
        }
    }
    Ok(())
}
async fn start_command(args: StartArgs) -> Result<()> {
    let config_path = args.config.clone().or_else(discover_config_path);
    let config = AppConfig::load_optional(config_path.as_deref())?;
    let report_path = args.report.clone().or_else(|| {
        config
            .report
            .as_ref()
            .and_then(|report| report.path.clone())
    });
    let mut session = Session::new(
        args.session.clone(),
        args.files.clone(),
        config_path,
        config.clone(),
        report_path.clone(),
    );
    if session.files.is_empty() {
        anyhow::bail!("provide at least one log file");
    }

    let channels = AppChannels::new(4096);
    let shutdown = ShutdownToken::new();
    let pipeline =
        start_pipeline(session.files.clone(), config, channels.clone(), shutdown).await?;

    if args.no_tui {
        run_console(pipeline, &mut session).await?;
    } else {
        run_tui(pipeline, &mut session).await?;
    }
    SessionStore::default()?.save(&session)?;
    println!("session saved as {}", session.name);
    Ok(())
}

async fn run_console(
    pipeline: logstream::pipeline::PipelineHandle,
    session: &mut Session,
) -> Result<()> {
    println!("analysis started. press Ctrl+C to stop.");
    let mut events = pipeline.channels.subscribe();
    loop {
        tokio::select! {
            event = events.recv() => {
                match event {
                    Ok(AppEvent::Entry(entry)) => println!("{}", entry.compact()),
                    Ok(AppEvent::Alert(alert)) => eprintln!("ALERT {}", alert.compact()),
                   Ok(AppEvent::Status(status)) => eprintln!("status: {status}"),
                    Ok(AppEvent::Error(error)) => eprintln!("error: {error}"),
                    Err(_) => break,
                }
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                break;
            }
        }
    }
    let final_stats = pipeline.stop().await;
    let snapshot = final_stats.snapshot();
    if let Some(path) = session.report_path.as_ref() {
        write_report(path, &snapshot)?;
        println!("report written to {}", path.display());
    }
    session.set_stats(snapshot);
    Ok(())
}

fn stats_command(args: StatsArgs) -> Result<()> {
    let store = SessionStore::default()?;
    let session = store.load(&args.session)?;
    let Some(snapshot) = session.last_stats.as_ref() else {
        println!("session '{}' does not contain statistics yet", session.name);
        return Ok(());
    };
    println!("{}", render_report(snapshot));
    if let Some(path) = args.report.as_ref() {
        write_report(path, snapshot)?;
        println!("report written to {}", path.display());
    }
    Ok(())
}

fn sessions_command() -> Result<()> {
    let store = SessionStore::default()?;
    let sessions = store.list()?;
    if sessions.is_empty() {
        println!("no sessions found in {}", store.root().display());
        return Ok(());
    }
    for session in sessions {
        println!(
            "{} | updated={} | files={}",
            session.name,
            session.updated_at,
            session
                .files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

fn config_command(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show { path } => {
            let path = path
                .or_else(discover_config_path)
                .ok_or_else(|| anyhow::anyhow!("config path was not provided"))?;
            let config = AppConfig::load(&path)?;
            println!("{}", serde_yaml::to_string(&config)?);
        }
        ConfigCommand::Init { path } => {
            AppConfig::write_default(&path)?;
            println!("config written to {}", path.display());
        }
    }
    Ok(())
}
