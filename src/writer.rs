use crate::config::Config;
use crate::events::Event;

use time::macros::format_description;

use std::ffi::OsString;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

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
    #[error("No parent directory at {path}")]
    NoParentDirectory { path: PathBuf },
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
        let log_file_path = PathBuf::from(config.log_dir());
        let (opened_file, full_path) = Writer::log_file_opener(&log_file_path, file_counter)?;

        Ok(Writer {
            log_directory: PathBuf::from(config.log_dir()),
            rotation_threshold: config.max_log_size(),
            opened_file,
            opened_file_path: full_path,
            current_file_size: 0,
            file_counter,
        })
    }

    pub fn write_event(&mut self, event: &Event) -> Result<(), WriterError> {
        let mut serialized_event = serde_json::to_string(event)?;
        // Need to account for the trailing '\n' char in current_file_size.
        let length = serialized_event.len() as u64 + 1;
        if length + self.current_file_size > self.rotation_threshold.as_u64() {
            self.rotate_log_file(&self.log_directory.clone())?;
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
        let log_file_path = PathBuf::from(path);
        let (file, full_path) = Writer::log_file_opener(&log_file_path, self.file_counter)?;
        self.opened_file = file;
        self.opened_file_path = full_path;
        self.current_file_size = 0;

        Ok(())
    }

    fn log_file_opener(
        log_file_path: &Path,
        file_counter: u64,
    ) -> Result<(File, PathBuf), WriterError> {
        let mut file_path = PathBuf::from(log_file_path);
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
        file_path.push(log_file_name);

        let path_parent = file_path.clone();
        let path_parent = path_parent
            .parent()
            .ok_or_else(|| WriterError::NoParentDirectory {
                path: file_path.clone(),
            })?;

        let mut dir_builder = DirBuilder::new();
        dir_builder
            .recursive(true)
            .create(path_parent)
            .map_err(|source| WriterError::Create {
                path: file_path.clone(),
                source,
            })?;

        let opened_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            // Fail if the filename exists.
            .open(file_path.clone())
            .map_err(|source| WriterError::Create {
                path: file_path.clone(),
                source,
            })?;

        Ok((opened_file, file_path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use regex::Regex;

    use std::fs::File;
    use std::path::PathBuf;

    use crate::config::Config;
    use crate::writer::Writer;

    #[test]
    fn create_file() {
        let dir: tempfile::TempDir = tempfile::tempdir().unwrap();
        let config: Config = Config::build_test_config(dir).unwrap();
        let writer: Writer = Writer::new(&config).unwrap();

        // Regex to match filename in a string such as: 2026-09-07_08-55-20_1.log
        let is_matched: bool = Regex::new(r"\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}_1\.log")
            .unwrap()
            .is_match(writer.opened_file_path.to_str().unwrap());
        assert!(is_matched);
    }

    #[test]
    fn event_writes_successfully() {
        use std::io::{BufRead, BufReader};

        use crate::events::Event;
        use crate::watcher::ChangeKind;

        let dir = tempfile::tempdir().unwrap();
        let config = Config::build_test_config(dir).unwrap();

        let event1 = Event {
            time: time::OffsetDateTime::now_utc(),
            action: ChangeKind::Created,
            file_path: PathBuf::from("/tmp/file.txt"),
            file_hash: Some(String::from(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            )),
            previous_hash: Some(String::from(
                "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
            )),
            file_size: Some(1024),
            hostname: gethostname::gethostname().to_string_lossy().into_owned(),
        };

        let mut writer = Writer::new(&config).unwrap();
        let _ = writer.write_event(&event1);

        // Reopen and parse the json lines to ensure the logs are properly formatted.
        let mut log_path: PathBuf = PathBuf::from(config.log_dir());
        // TODO: Fix this as this is the config file and not the log file.
        // Probably will need a regex to match the filename of the actual log file.
        log_path.push("test_config.toml");
        let file: File = File::open(log_path).unwrap();
        let reader: BufReader<File> = BufReader::new(file);

        let json_line: String = reader.lines().next().unwrap().unwrap();
        let event_entry: Event = serde_json::from_str(&json_line).unwrap();

        assert_eq!(event1, event_entry);
    }
}
