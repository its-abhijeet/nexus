use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

/// Trait for any Raft-compatible state machine.
/// This allows pluggable logic for different types of services (e.g., key-value store, DB, etc.)
pub trait StateMachine: Send + Sync {
    type Command: Send + Sync;
    type Response: Send + Sync;

    fn get(&self, key: String) -> Option<String>;

    /// Applies a command and returns a response
    fn apply(&mut self, command: Self::Command) -> Self::Response;

    /// Produces a binary snapshot of the current state
    fn snapshot(&self) -> Vec<u8>;

    /// Restores state from a binary snapshot
    fn restore(&mut self, snapshot: Vec<u8>) -> Result<(), Box<dyn Error>>;
}

//
// Example Implementation: In-Memory Key-Value Store
//

/// Commands that the key-value store can handle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KvCommand {
    Set(String, String),
    Get(String),
    Delete(String),
}

/// Response type returned by the state machine
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KvResponse {
    Value(Option<String>),
    Ack,
}

/// The in-memory key-value store with Raft StateMachine trait
#[derive(Debug, Default)]
pub struct KeyValueStore {
    data: HashMap<String, String>,
}

impl StateMachine for KeyValueStore {
    type Command = KvCommand;
    type Response = KvResponse;

    fn apply(&mut self, command: Self::Command) -> Self::Response {
        match command {
            KvCommand::Set(k, v) => {
                self.data.insert(k, v);
                KvResponse::Ack
            }
            KvCommand::Get(k) => KvResponse::Value(self.data.get(&k).cloned()),
            KvCommand::Delete(k) => {
                self.data.remove(&k);
                KvResponse::Ack
            }
        }
    }

    fn snapshot(&self) -> Vec<u8> {
        bincode::serialize(&self.data).unwrap()
    }

    fn restore(&mut self, snapshot: Vec<u8>) -> Result<(), Box<dyn Error>> {
        self.data = bincode::deserialize(&snapshot)?;
        Ok(())
    }

    fn get(&self, key: String) -> Option<String> {
        self.data.get(&key).cloned()
    }
}

//
// Tests
//
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_get_delete() {
        let mut kv = KeyValueStore::default();

        // Set a value
        let resp = kv.apply(KvCommand::Set("foo".into(), "bar".into()));
        assert_eq!(resp, KvResponse::Ack);

        // Get it
        let resp = kv.apply(KvCommand::Get("foo".into()));
        assert_eq!(resp, KvResponse::Value(Some("bar".into())));

        // Delete it
        let resp = kv.apply(KvCommand::Delete("foo".into()));
        assert_eq!(resp, KvResponse::Ack);

        // Ensure it's gone
        let resp = kv.apply(KvCommand::Get("foo".into()));
        assert_eq!(resp, KvResponse::Value(None));
    }

    #[test]
    fn test_snapshot_restore() {
        let mut kv = KeyValueStore::default();
        kv.apply(KvCommand::Set("alpha".into(), "beta".into()));

        let snap = kv.snapshot();

        let mut restored = KeyValueStore::default();
        restored.restore(snap).unwrap();

        let resp = restored.apply(KvCommand::Get("alpha".into()));
        assert_eq!(resp, KvResponse::Value(Some("beta".into())));
    }
}
