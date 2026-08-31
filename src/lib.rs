mod baseline;
mod config;
mod error;
mod events;
mod watcher;
mod writer;

pub use baseline::{Baseline, BaselineError, HashChange};
pub use config::{Config, ConfigError, HashAlgorithm};
pub use error::VerinoxError;
pub use watcher::{ChangeKind, WatchEvent, Watcher, WatcherError};

use std::path::PathBuf;

pub fn run(config_path: &PathBuf) -> Result<(), VerinoxError> {
    Config::ensure_exists(config_path)?;
    let config = Config::load(config_path)?;

    let patterns: Vec<glob::Pattern> = config
        .watch_patterns()
        .iter()
        .map(|s| glob::Pattern::new(s))
        .collect::<Result<_, _>>()
        .map_err(|source| ConfigError::GlobPattern { source })?;

    let watcher = Watcher::new(&patterns)?;

    for event in watcher.events.iter() {
        println!("{:?}: {}", event.kind, event.path.display());
        // TODO: replace with baseline.rs hash/diff + events.rs Event + writer.rs append, once those modules exist.
    }

    Ok(())
}
