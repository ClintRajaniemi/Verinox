use crate::baseline::BaselineError;
use crate::config::ConfigError;
use crate::watcher::WatcherError;
use crate::writer::WriterError;

#[derive(thiserror::Error, Debug)]
pub enum VerinoxError {
    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Watcher(#[from] WatcherError),

    #[error(transparent)]
    Baseline(#[from] BaselineError),

    #[error(transparent)]
    Writer(#[from] WriterError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
