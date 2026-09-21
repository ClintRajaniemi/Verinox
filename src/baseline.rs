use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::HashAlgorithm;
use crate::watcher::{ChangeKind, WatchEvent};

#[derive(thiserror::Error, Debug)]
pub enum BaselineError {
    #[error("failed to read baseline file at {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write baseline file at {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse baseline file: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("failed to read watched file at {path}: {source}")]
    ReadWatchedFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("no parent directory at {path}")]
    NoParentDirectory { path: PathBuf },
}

#[derive(Deserialize, Debug)]
pub struct Baseline {
    // File path to tracked file.
    pub path: PathBuf,
    // This HashMap contains the path of the tracked file and the file hash.
    pub entry: HashMap<PathBuf, String>,
}

#[derive(Debug)]
pub struct HashChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub previous_hash: Option<String>,
    pub new_hash: Option<String>,
    pub file_size: Option<u64>,
}

impl Baseline {
    pub fn load(path: &Path) -> Result<Baseline, BaselineError> {
        // Opens and reads the baseline file if it exists wh ich allows file hash tracking across runs.
        // Returns a baseline object.
        if !path.exists() {
            return Ok(Baseline {
                path: path.to_path_buf(),
                entry: HashMap::new(),
            });
        }
        let contents = fs::read_to_string(path).map_err(|source| BaselineError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let entry: HashMap<PathBuf, String> = serde_json::from_str(&contents)?;
        Ok(Baseline {
            path: path.to_path_buf(),
            entry,
        })
    }

    pub fn process(
        &mut self,
        watch_event: &WatchEvent,
        hash_algorithm: HashAlgorithm,
    ) -> Result<Option<HashChange>, BaselineError> {
        match watch_event.kind {
            ChangeKind::Deleted => {
                // Only report something if we were actually tracking this path.
                // A Deleted watch_event for a path we never knew about isn't meaningful.
                let Some(previous_hash) = self.entry.remove(&watch_event.path) else {
                    return Ok(None);
                };
                self.save()?;
                Ok(Some(HashChange {
                    path: watch_event.path.clone(),
                    kind: ChangeKind::Deleted,
                    previous_hash: Some(previous_hash),
                    new_hash: None,
                    file_size: None,
                }))
            }
            _ => {
                // Created, Modified, Renamed. All 3 get the same treatment:
                // read the file, hash it, see if that hash is actually new.
                let bytes = match fs::read(&watch_event.path) {
                    Ok(bytes) => bytes,
                    Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                        // Race: notify told us about a change, but by the time we got here, the file is gone.
                        // Treat it as a deletion instead of a hard error. Recurse into the Deleted arm above with
                        // a synthesized watch_event. Save to recurse here: this always lands in the Deleted branch, which never recurses further.
                        return self.process(
                            &WatchEvent {
                                path: watch_event.path.clone(),
                                kind: ChangeKind::Deleted,
                            },
                            hash_algorithm,
                        );
                    }
                    Err(source) => {
                        return Err(BaselineError::ReadWatchedFile {
                            path: watch_event.path.clone(),
                            source,
                        });
                    }
                };

                let new_hash = hash_algorithm.hash(&bytes);
                let previous_hash = self.entry.get(&watch_event.path).cloned();

                // Same hash as last time we recorded it? This is the duplicate-watch_event
                // case. Nothing to report.
                if previous_hash.as_deref() == Some(new_hash.as_str()) {
                    return Ok(None);
                }

                let kind = if previous_hash.is_none() {
                    ChangeKind::Created
                } else {
                    ChangeKind::Modified
                };
                self.entry
                    .insert(watch_event.path.clone(), new_hash.clone());
                self.save()?;

                Ok(Some(HashChange {
                    path: watch_event.path.clone(),
                    kind,
                    previous_hash,
                    new_hash: Some(new_hash),
                    file_size: Some(bytes.len() as u64),
                }))
            }
        }
    }

    pub fn save(&self) -> Result<(), BaselineError> {
        let json_string = serde_json::to_string(&self.entry)?;
        let path_parent = self
            .path
            .parent()
            .ok_or_else(|| BaselineError::NoParentDirectory {
                path: self.path.clone(),
            })?;
        let mut dir_builder = fs::DirBuilder::new();
        // No need to check existence of the path, with recursive(true) the create will not fail if the path exists.
        dir_builder
            .recursive(true)
            .create(path_parent)
            .map_err(|source| BaselineError::Write {
                path: self.path.to_path_buf(),
                source,
            })?;

        let mut file_name = self.path.file_name().unwrap_or_default().to_os_string();
        file_name.push(".tmp");
        let temp_path = self.path.with_file_name(file_name);
        // Write to baseline.json.tmp
        fs::write(temp_path.clone(), json_string).map_err(|source| BaselineError::Write {
            path: temp_path.to_path_buf(),
            source,
        })?;
        // Overwrite baseline.json with baseline.json.tmp
        fs::rename(temp_path, self.path.clone()).map_err(|source| BaselineError::Write {
            path: self.path.to_path_buf(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use std::fs::write;

    use crate::baseline::Baseline;
    use crate::config;
    use crate::watcher::{ChangeKind, Watcher};

    #[test]
    fn load_missing_file_returns_empty_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("baseline.json");
        let baseline = Baseline::load(&path).unwrap();
        assert!(baseline.entry.is_empty());
    }

    #[test]
    fn load_existing_file_parses_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("baseline.json");
        write(&path, r#"{"/etc/hosts": "sha256:abc123"}"#).unwrap();
        let baseline = Baseline::load(&path).unwrap();
        assert_eq!(
            baseline.entry.get(Path::new("/etc/hosts")),
            Some(&"sha256:abc123".to_string())
        );
    }
    #[test]
    fn process_event_file_created() {
        let dir = tempfile::tempdir().unwrap();
        let watch_pattern_path = dir.path().join("watched");
        let _ = std::fs::create_dir(&watch_pattern_path);
        let watch_pattern = watch_pattern_path.join("**");
        let test_config =
            config::Config::build_test_config(&dir, &[&watch_pattern.to_str().unwrap()]).unwrap();

        let patterns: Vec<glob::Pattern> = test_config
            .watch_patterns()
            .iter()
            .map(|s| glob::Pattern::new(s))
            .collect::<Result<_, _>>()
            .map_err(|source| config::ConfigError::GlobPattern { source })
            .unwrap();

        let mut baseline = Baseline::load(&test_config.baseline_path()).unwrap();
        let watcher = Watcher::new(&patterns).unwrap();
        // Change a file in the path
        let watch_pattern_file = watch_pattern_path.join("test.txt");
        // We need to create the file in the directory so that it can be tracked as a new file change but we don't need the resulting file handle.
        let _test_file = File::create(&watch_pattern_file).unwrap();

        // Grab one event
        let watch_event = watcher
            .events
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        // Process the event
        let result = baseline
            .process(&watch_event, test_config.hash_algorithm())
            .unwrap()
            .unwrap();

        assert_eq!(ChangeKind::Created, result.kind);
        assert_eq!(
            result.new_hash.unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
    #[test]
    fn process_event_file_modified() {
        let dir = tempfile::tempdir().unwrap();
        let watch_pattern_path = dir.path().join("watched");
        let _ = std::fs::create_dir(&watch_pattern_path);
        let watch_pattern = watch_pattern_path.join("**");
        let test_config =
            config::Config::build_test_config(&dir, &[&watch_pattern.to_str().unwrap()]).unwrap();

        let patterns: Vec<glob::Pattern> = test_config
            .watch_patterns()
            .iter()
            .map(|s| glob::Pattern::new(s))
            .collect::<Result<_, _>>()
            .map_err(|source| config::ConfigError::GlobPattern { source })
            .unwrap();

        let mut baseline = Baseline::load(&test_config.baseline_path()).unwrap();
        let watcher = Watcher::new(&patterns).unwrap();
        // Change a file in the path
        let watch_pattern_file = watch_pattern_path.join("test.txt");
        let mut test_file = File::create(&watch_pattern_file).unwrap();
        writeln!(test_file, "Brian was here. Briefly.").unwrap();

        // Create a baseline where it already has a file name and file hash so that we can see the ChangeKind::Modified
        baseline.path = watch_pattern_file.clone();
        baseline.entry = HashMap::new();
        baseline.entry.insert(
            watch_pattern_file,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
        );

        // Grab one event
        let watch_event = watcher
            .events
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let result = baseline
            .process(&watch_event, test_config.hash_algorithm())
            .unwrap()
            .unwrap();
        assert_eq!(
            result.previous_hash.unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(ChangeKind::Modified, result.kind);
    }
}
