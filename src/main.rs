use std::error::Error;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::config::Config;
use crate::daemon::Message;

mod daemon;
mod config;
mod history;
mod metadata;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Queue a song to play.
    Play {
        path: PathBuf,
        /// Clear the queue and play immediately.
        #[clap(long)]
        now: bool,
    },
    /// Pause or unpause the current song.
    Pause,
    /// Stop the player.
    Stop,
    /// Skip to the next song.
    Skip,
    /// Start the player daemon. This should not be used directly.
    Daemon,
    /// Print the location of the configuration directory.
    Config,
}

fn main() -> crate::Result<()> {
    let config = &match config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Config error (using defaults): {}", error);
            Config::default()
        }
    };

    let cli = Cli::parse();
    match cli.command {
        Commands::Play { path, now } => play(config, path, now),
        Commands::Pause => daemon::send(config, &Message::Pause),
        Commands::Stop => daemon::send(config, &Message::Stop),
        Commands::Skip => daemon::send(config, &Message::Skip),
        Commands::Daemon => daemon::daemon(config),
        Commands::Config => {
            println!("{}", config::directory().display());
            Ok(())
        }
    }
}

fn play(config: &Config, input: PathBuf, now: bool) -> crate::Result<()> {
    let path = match input.exists() {
        true => Some(input.clone()),
        false => history::find(&input)?,
    };

    let path = path
        .map(|path| path.canonicalize())
        .transpose()?;

    match path {
        None => Err("Audio file does not exist.".into()),
        Some(path) => {
            history::insert(&input, &path)?;
            daemon::send(config, &Message::Play { path, now })
        }
    }
}
