use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("failed to read config file at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config {0}")]
    Parse(#[from] toml::de::Error),

    #[error("invalid watch path: {0}")]
    InvalidWatchPath(PathBuf),

    #[error("unsupported hash algorithm: {0}")]
    UnsupportedHashAlgorithm(String),
}

#[derive(thiserror::Error, Debug)]
pub enum WatcherError {}

#[derive(thiserror::Error, Debug)]
pub enum BaselineError {}

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
