#[cfg(test)]
pub use verinox::{ChangeKind, WatchEvent, Watcher};

#[test]
fn watcher_detects_file_creation() {
    let dir = tempfile::tempdir().unwrap();
    let pattern = glob::Pattern::new(&format!("{}/**", dir.path().display())).unwrap();
    let watcher = Watcher::new(&[pattern]).unwrap();

    std::fs::write(dir.path().join("test.txt"), b"hello").unwrap();

    let event = watcher
        .events
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(event.kind, ChangeKind::Created);
}
