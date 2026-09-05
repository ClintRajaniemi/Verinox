use crate::config::Config;
use crate::events::Event;

use time::macros::format_description;

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
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
        let (opened_file, full_path) = Writer::log_file_opener(&mut log_file_path, file_counter)?;

        Ok(Writer {
            log_directory: PathBuf::from(config.log_dir()),
            rotation_threshold: config.max_log_size(),
            opened_file: opened_file,
            opened_file_path: full_path,
            current_file_size: 0,
            file_counter: file_counter,
        })
    }

    pub fn write_event(&mut self, event: &Event) -> Result<(), WriterError> {
        let mut serialized_event = serde_json::to_string(event)?;
        // Need to account for the trailing '\n' char in current_file_size.
        let length = serialized_event.len() as u64 + 1;
        if length + self.current_file_size > self.rotation_threshold.as_u64() {
            self.rotate_log_file(&mut self.log_directory.clone())?;
        }
        // The trailing '\n' we accounted for in the above length calculation.
        serialized_event.push('\n');
        self.opened_file
            .write_all(serialized_event.as_bytes())
            .map_err(|source| WriterError::Write {
                path: self.opened_file_path.clone(),
                source,
            })?;
        self.current_file_size += length;

        Ok(())
    }

    fn rotate_log_file(&mut self, path: &PathBuf) -> Result<(), WriterError> {
        self.file_counter += 1;
        let mut log_file_path = PathBuf::from(path);
        let (file, full_path) = Writer::log_file_opener(&mut log_file_path, self.file_counter)?;
        self.opened_file = file;
        self.opened_file_path = full_path;
        self.current_file_size = 0;
        // Now that we have a successful file write, we can reset the file_counter to 0.
        self.file_counter = 0;

        Ok(())
    }

    fn log_file_opener(
        log_file_path: &mut PathBuf,
        file_counter: u64,
    ) -> Result<(File, PathBuf), WriterError> {
        // Build the full log path filename
        let format =
            format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]").to_owned();
        let now = time::OffsetDateTime::now_utc();
        let mut log_file_name: OsString = now
            .format(&format)
            .map_err(|source| WriterError::Time { source })?
            .into();
        log_file_name.push("_");
        // We use file_counter in the name to prevent naming collisions
        log_file_name.push(file_counter.to_string());
        log_file_name.push(".log");
        log_file_path.push(log_file_name);

        let opened_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            // Fail if the filename exists.
            .open(log_file_path.clone())
            .map_err(|source| WriterError::Create {
                path: log_file_path.clone(),
                source,
            })?;

        Ok((opened_file, log_file_path.to_path_buf()))
    }
}
