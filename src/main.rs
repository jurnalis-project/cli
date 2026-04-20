mod commands;

use clap::{Parser, Subcommand};
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
        None => {
            // Default to play when no subcommand given
            commands::play::execute(None, None);
        }
    }
}
