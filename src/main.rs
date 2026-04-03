use anyhow::Result;
use clap::{Args, Parser, Subcommand};
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
            println!("stats command: {:?}", args);
        }
        Command::Sessions => {
            println!("sessions command");
        }
        Command::Config { command } => {
            println!("config command: {:?}", command);
        }
    }
    Ok(())
}
