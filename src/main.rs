use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use logstream::config::{AppConfig, discover_config_path};
use logstream::report::{render_report, write_report};
use logstream::session::SessionStore;
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Start(args) => {
            println!("start command: {:?}", args);
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
