use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::raft::state_machine::KvCommand;
use nexus_common::error::NexusError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RaftSnapshot {
    pub last_included_index: u64,
    pub last_included_term: u64,
    pub state: Vec<u8>, // Serialized state machine data
}

/// Defines the behavior for any snapshot storage backend.
pub trait SnapshotStorage {
    fn save(&self, snapshot: &RaftSnapshot) -> Result<(), NexusError>;
    fn load(&self) -> Result<Option<RaftSnapshot>, NexusError>;
}

/// Saves snapshots as a binary file.
pub struct FileSnapshotStorage {
    pub path: PathBuf,
}

impl FileSnapshotStorage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl SnapshotStorage for FileSnapshotStorage {
    fn save(&self, snapshot: &RaftSnapshot) -> Result<(), NexusError> {
        let encoded = bincode::serialize(snapshot)?;
        let mut file = File::create(&self.path)?;
        file.write_all(&encoded)?;
        Ok(())
    }

    fn load(&self) -> Result<Option<RaftSnapshot>, NexusError> {
        if !self.path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&self.path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let snapshot = bincode::deserialize(&bytes)?;
        Ok(Some(snapshot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_save_and_load() {
        let temp_path = PathBuf::from("test_snapshot.bin");
        let store = FileSnapshotStorage::new(&temp_path);

        let snap = RaftSnapshot {
            last_included_index: 42,
            last_included_term: 3,
            state: bincode::serialize(&KvCommand::Set("x".into(), "y".into())).unwrap(),
        };

        store.save(&snap).expect("Failed to save snapshot");
        let loaded = store.load().expect("Failed to load snapshot").unwrap();

        assert_eq!(snap.last_included_index, loaded.last_included_index);
        assert_eq!(snap.last_included_term, loaded.last_included_term);
        assert_eq!(snap.state, loaded.state);

        // cleanup
        let _ = fs::remove_file(temp_path);
    }
}
