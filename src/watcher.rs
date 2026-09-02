use notify::EventKind;
use notify::Watcher as _;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use serde::Serialize;

pub struct Watcher {
    _inner: notify::RecommendedWatcher,
    pub events: mpsc::Receiver<WatchEvent>,
}

impl Watcher {
    pub fn new(globs: &[glob::Pattern]) -> Result<Watcher, WatcherError> {
        let mut validated_paths: HashSet<PathBuf> = HashSet::new();
        for glob in globs {
            let root = watch_root(glob.as_str());
            validated_paths.insert(root);
        }
        let owned_globs: Vec<glob::Pattern> = globs.to_vec();

        let (tx, rx) = mpsc::channel::<WatchEvent>();

        let mut watcher =
            notify::recommended_watcher(move |event_result: notify::Result<notify::Event>| {
                match event_result {
                    Ok(event) => {
                        for watch_event in translate_event(&event, &owned_globs) {
                            let _ = tx.send(watch_event);
                        }
                    }
                    Err(err) => eprintln!("{err}"),
                }
            })?;

        for root in &validated_paths {
            watcher
                .watch(root, notify::RecursiveMode::Recursive)
                .map_err(|source| WatcherError::Watch {
                    path: root.clone(),
                    source,
                })?;
        }
        Ok(Watcher {
            _inner: watcher,
            events: rx,
        })
    }
}

pub fn translate_event(event: &notify::Event, patterns: &[glob::Pattern]) -> Vec<WatchEvent> {
    let Some(kind) = map_kind(&event.kind) else {
        return Vec::new();
    };
    event
        .paths
        .iter()
        .filter(|path| patterns.iter().any(|p| p.matches(&path.to_string_lossy())))
        .map(|path| WatchEvent {
            path: path.clone(),
            kind,
        })
        .collect()
}

#[derive(thiserror::Error, Debug)]
pub enum WatcherError {
    #[error("failed to create filesystem watcher: {0}")]
    Init(#[from] notify::Error),

    #[error("failed to watch path {path}: {source}")]
    Watch {
        path: PathBuf,
        source: notify::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

pub struct WatchEvent {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

fn watch_root(pattern: &str) -> PathBuf {
    let mut root = PathBuf::new();
    for component in Path::new(pattern).components() {
        let s = component.as_os_str().to_string_lossy();
        if s.contains('*') || s.contains('?') || s.contains('[') {
            break;
        }
        root.push(component);
    }
    root
}

fn map_kind(kind: &notify::EventKind) -> Option<ChangeKind> {
    match kind {
        EventKind::Create(_) => Some(ChangeKind::Created),
        EventKind::Modify(_) => Some(ChangeKind::Modified),
        EventKind::Remove(_) => Some(ChangeKind::Deleted),
        _ => None, // Access, Other, Any - not relevant to integrity.
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_watch_root() {
        let path = watch_root("C:\\Windows\\System32\\Tasks\\**");
        assert_eq!("C:\\Windows\\System32\\Tasks", path.to_string_lossy());
    }
    #[test]
    fn test_map_kind() {
        let kind = notify::EventKind::Create(notify::event::CreateKind::File);
        let mapped_kind = map_kind(&kind);
        assert_eq!(mapped_kind, Some(ChangeKind::Created));
    }
    #[test]
    fn translate_event_filters_non_matching_paths() {
        let patterns = vec![glob::Pattern::new("/etc/passwd").unwrap()];
        let event = notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Any))
            .add_path(PathBuf::from("/etc/shadow"));
        assert!(translate_event(&event, &patterns).is_empty());
    }
}
