mod commands;
mod protocol;

use clap::{Parser, Subcommand};
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "jurnalis-cli", version, about = "Jurnalis text-based CRPG")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an interactive game session
    Play {
        /// Override the directory where save files are stored
        #[arg(long = "save-dir")]
        save_dir: Option<PathBuf>,

        /// Inject a pre-crafted GameState from a JSON file (dev only)
        #[cfg(feature = "dev")]
        #[arg(long = "dev-state")]
        dev_state: Option<String>,
    },
    /// Run as a JSONL protocol server (machine interface)
    StdioJson {
        /// Override the directory where save files are stored
        #[arg(long = "save-dir")]
        save_dir: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Play {
            save_dir,
            #[cfg(feature = "dev")]
            dev_state,
        }) => {
            #[cfg(feature = "dev")]
            {
                commands::play::execute(dev_state.as_deref(), save_dir);
            }
            #[cfg(not(feature = "dev"))]
            {
                commands::play::execute(None, save_dir);
            }
        }
        Some(Commands::StdioJson { save_dir }) => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let stderr = io::stderr();
            let mut reader = stdin.lock();
            let mut writer = stdout.lock();
            let mut err_writer = stderr.lock();
            let save_dir = save_dir.unwrap_or_else(|| PathBuf::from("saves"));

            if let Err(e) = protocol::run_protocol(&mut reader, &mut writer, &mut err_writer, &save_dir) {
                eprintln!("Protocol error: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            // Default to play when no subcommand given
            commands::play::execute(None, None);
        }
    }
}
