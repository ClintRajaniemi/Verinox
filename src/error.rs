use crate::config::ConfigError;
use crate::baseline::BaselineError;
use crate::watcher::WatcherError;

#[derive(thiserror::Error, Debug)]
pub enum VerinoxError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Watcher(#[from] WatcherError),

    #[error(transparent)]
    Baseline(#[from] BaselineError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
