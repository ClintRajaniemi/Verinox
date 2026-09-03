//! # Verinox
//! `Verinox` is an open source file integrity monitoring tool that works on Windows, Linux and macOS.

mod baseline;
mod config;
mod error;
mod events;
mod watcher;
mod writer;

pub use baseline::{Baseline, BaselineError, HashChange};
pub use config::{Config, ConfigError, HashAlgorithm};
pub use error::VerinoxError;
pub use events::Event;
pub use watcher::{ChangeKind, WatchEvent, Watcher, WatcherError};

use std::path::PathBuf;

pub fn run(config_path: &PathBuf) -> Result<(), VerinoxError> {
    // Checks that `config_path` exists or else it writes a default config at `config_path`
    Config::ensure_exists(config_path)?;
    let config = Config::load(config_path)?;

    let patterns: Vec<glob::Pattern> = config
        .watch_patterns()
        .iter()
        .map(|s| glob::Pattern::new(s))
        .collect::<Result<_, _>>()
        .map_err(|source| ConfigError::GlobPattern { source })?;

    let watcher = Watcher::new(&patterns)?;
    let mut baseline = Baseline::load(config.baseline_path())?;
    // This loop runs until the channel is shutdown which is to say the entire time the process is running.
    for event in watcher.events.iter() {
        let result = baseline.process(&event, config.hash_algorithm());
        // Let's explicitely handle the Result/Err and Option so we don't break this loop on a single error.
        match result {
            // TODO: log this condition
            Ok(Some(change)) => {
                let event = Event::new(change);
                // TODO: Process the event with a writer to write to file.
                println!("{:?}", event);
            },
            // TODO: Log this condition
            Ok(None) => println!("Nothing to do here."),
            // TODO: Log this error
            Err(e) => println!("{}", e),
        }
    }

    Ok(())
}
