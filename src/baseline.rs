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
    pub entries: HashMap<PathBuf, String>,
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
                entries: HashMap::new(),
            });
        }
        let contents = fs::read_to_string(path).map_err(|source| BaselineError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let entries: HashMap<PathBuf, String> = serde_json::from_str(&contents)?;
        Ok(Baseline {
            path: path.to_path_buf(),
            entries,
        })
    }

    pub fn process(
        &mut self,
        event: &WatchEvent,
        hash_algorithm: HashAlgorithm,
    ) -> Result<Option<HashChange>, BaselineError> {
        match event.kind {
            ChangeKind::Deleted => {
                // Only report something if we were actually tracking this path.
                // A Deleted event for a path we never knew about isn't meaningful.
                let Some(previous_hash) = self.entries.remove(&event.path) else {
                    return Ok(None);
                };
                self.save()?;
                Ok(Some(HashChange {
                    path: event.path.clone(),
                    kind: ChangeKind::Deleted,
                    previous_hash: Some(previous_hash),
                    new_hash: None,
                    file_size: None,
                }))
            }
            _ => {
                // Created, Modified, Renamed. All 3 get the same treatment:
                // read the file, hash it, see if that hash is actually new.
                let bytes = match fs::read(&event.path) {
                    Ok(bytes) => bytes,
                    Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                        // Race: notify told us about a change, but by the time we got here, the file is gone.
                        // Treat it as a deletion instead of a hard error. Recurse into the Deleted arm above with
                        // a synthesized event. Save to recurse here: this always lands in the Deleted branch, which never recurses further.
                        return self.process(
                            &WatchEvent {
                                path: event.path.clone(),
                                kind: ChangeKind::Deleted,
                            },
                            hash_algorithm,
                        );
                    }
                    Err(source) => {
                        return Err(BaselineError::ReadWatchedFile {
                            path: event.path.clone(),
                            source,
                        });
                    }
                };

                let new_hash = hash_algorithm.hash(&bytes);
                let previous_hash = self.entries.get(&event.path).cloned();

                // Same hash as last time we recorded it? This is the duplicate-event
                // case. Nothing to report.
                if previous_hash.as_deref() == Some(new_hash.as_str()) {
                    return Ok(None);
                }

                let kind = if previous_hash.is_none() {
                    ChangeKind::Created
                } else {
                    ChangeKind::Modified
                };
                self.entries.insert(event.path.clone(), new_hash.clone());
                self.save()?;

                Ok(Some(HashChange {
                    path: event.path.clone(),
                    kind,
                    previous_hash,
                    new_hash: Some(new_hash),
                    file_size: Some(bytes.len() as u64),
                }))
            }
        }
    }

    pub fn save(&self) -> Result<(), BaselineError> {
        let json_string = serde_json::to_string(&self.entries)?;
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
#[test]
fn load_missing_file_returns_empty_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baseline.json");
    let baseline = Baseline::load(&path).unwrap();
    assert!(baseline.entries.is_empty());
}

#[test]
fn load_existing_file_parses_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("baseline.json");
    fs::write(&path, r#"{"/etc/hosts": "sha256:abc123"}"#).unwrap();
    let baseline = Baseline::load(&path).unwrap();
    assert_eq!(
        baseline.entries.get(Path::new("/etc/hosts")),
        Some(&"sha256:abc123".to_string())
    );
}
