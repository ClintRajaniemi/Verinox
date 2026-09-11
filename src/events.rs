use crate::{ChangeKind, HashChange};

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Event {
    #[serde(with = "time::serde::rfc3339")]
    pub time: time::OffsetDateTime,
    pub action: ChangeKind,
    pub file_path: PathBuf,
    pub file_hash: Option<String>,
    pub previous_hash: Option<String>,
    pub file_size: Option<u64>,
    pub hostname: String,
}

impl Event {
    pub fn new(change: HashChange) -> Event {
        Event {
            time: time::OffsetDateTime::now_utc(),
            action: change.kind,
            previous_hash: change.previous_hash,
            file_hash: change.new_hash,
            file_path: change.path,
            file_size: change.file_size,
            hostname: gethostname::gethostname().to_string_lossy().into_owned(),
        }
    }
}
