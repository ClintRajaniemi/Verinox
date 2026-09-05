use crate::config::Config;
use crate::events::Event;

use time::macros::format_description;

use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum WriterError {
    #[error("failed to create/open log file at {path}: {source}")]
    Create {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write log file at {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize object: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("failed to format time: {source}")]
    Time { source: time::error::Format },
}

pub struct Writer {
    log_directory: PathBuf,
    // file size before the file is rotated
    rotation_threshold: bytesize::ByteSize,
    opened_file: std::fs::File,
    opened_file_path: PathBuf,
    current_file_size: u64,
    // Prevent filename collisions with a counter each time a file is opened.
    file_counter: u64,
}

impl Writer {
    pub fn new(config: &Config) -> Result<Self, WriterError> {
        let file_counter: u64 = 1;
        let mut log_file_path = PathBuf::from(config.log_dir());
        let format =
            format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]").to_owned();
        let now = time::OffsetDateTime::now_utc();
        let mut log_file_name: OsString = now
            .format(&format)
            .map_err(|source| WriterError::Time { source })?
            .into();
        log_file_name.push("_");
        log_file_name.push(file_counter.to_string());
        log_file_name.push(".log");
        log_file_path.push(log_file_name);
        let opened_file = File::create(&log_file_path).map_err(|source| WriterError::Create {
            path: log_file_path.clone(),
            source,
        })?;

        Ok(Writer {
            log_directory: PathBuf::from(config.log_dir()),
            rotation_threshold: config.max_log_size(),
            opened_file: opened_file,
            opened_file_path: log_file_path,
            current_file_size: 0,
            file_counter: file_counter,
        })
    }

    pub fn write_event(&mut self, event: &Event) -> Result<(), WriterError> {
        let mut serialized_event = serde_json::to_string(&event)?;
        // Need to account for the '\n' char in current_file_size.
        let length = serialized_event.len() as u64 + 1;
        if length + self.current_file_size >= self.rotation_threshold.as_u64() {
            //TODO: Log file has reached rotation_threshold. Rotate the log file.
            //self.rotate();
        }
        serialized_event.push('\n');
        self.opened_file
            .write(serialized_event.as_bytes())
            .map_err(|source| WriterError::Write {
                path: self.log_directory.clone(),
                source,
            })?;
        self.current_file_size += length;

        Ok(())
    }
}
