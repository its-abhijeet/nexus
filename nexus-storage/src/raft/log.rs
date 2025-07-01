use serde::{Deserialize, Serialize};

/// A single log entry in the Raft log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub term: u64,                // Term number when entry was received by leader
    pub index: u64,               // Index of the log entry in the log
    pub entry_type: LogEntryType, // Type of entry (Command/Config/Noop)
    pub data: Vec<u8>,            // Payload (usually a command)
}

/// Type of log entry — determines how state machine interprets the entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LogEntryType {
    Command,       // Actual command to apply to the state machine
    Configuration, // Change in cluster nodes
    Noop,          // Empty entry to assert leadership
}

/// A complete log for one Raft node
#[derive(Debug)]
pub struct RaftLog {
    pub entries: Vec<LogEntry>, // Ordered log entries
    pub commit_index: u64,      // Index of last committed entry
    pub last_applied: u64,      // Index of last entry applied to state machine
}

impl RaftLog {
    /// Create an empty Raft log
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            commit_index: 0,
            last_applied: 0,
        }
    }

    /// Append a new log entry to the log
    pub fn append(&mut self, entry: LogEntry) {
        self.entries.push(entry);
    }

    /// Get a specific log entry by index (not array index, Raft log index)
    pub fn get(&self, index: u64) -> Option<&LogEntry> {
        self.entries.iter().find(|e| e.index == index)
    }

    /// Returns the last log index, or 0 if the log is empty
    pub fn last_index(&self) -> u64 {
        self.entries.last().map(|e| e.index).unwrap_or(0)
    }

    /// Returns the term of the last entry, or 0 if empty
    pub fn last_term(&self) -> u64 {
        self.entries.last().map(|e| e.term).unwrap_or(0)
    }
}

/// Possible roles a Raft node can assume
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RaftRole {
    Follower,
    Candidate,
    Leader,
}

/// Main Raft node struct holding node state
pub struct RaftNode {
    pub id: String,                // This node's unique ID
    pub current_term: u64,         // Last term node has seen
    pub voted_for: Option<String>, // ID of candidate voted for in current term
    pub role: RaftRole,            // Current role
    pub log: RaftLog,              // Persistent log
}

impl RaftNode {
    /// Create a new Raft node (usually as Follower)
    pub fn new(id: String) -> Self {
        Self {
            id,
            current_term: 0,
            voted_for: None,
            role: RaftRole::Follower,
            log: RaftLog::new(),
        }
    }

    /// Promote to candidate — increment term and vote for self
    pub fn become_candidate(&mut self) {
        self.current_term += 1;
        self.voted_for = Some(self.id.clone());
        self.role = RaftRole::Candidate;
    }

    /// Promote to leader — used when election is won
    pub fn become_leader(&mut self) {
        self.role = RaftRole::Leader;

        // Append a Noop entry to confirm leadership to followers
        let entry = LogEntry {
            term: self.current_term,
            index: self.log.last_index() + 1,
            entry_type: LogEntryType::Noop,
            data: vec![],
        };
        self.log.append(entry);
    }

    /// Step down to follower with updated term
    pub fn become_follower(&mut self, term: u64) {
        self.current_term = term;
        self.voted_for = None;
        self.role = RaftRole::Follower;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_get_entry() {
        let mut log = RaftLog::new();
        log.append(LogEntry {
            term: 1,
            index: 1,
            entry_type: LogEntryType::Command,
            data: vec![1, 2, 3],
        });

        assert_eq!(log.last_index(), 1);
        assert_eq!(log.last_term(), 1);
        assert_eq!(log.get(1).unwrap().data, vec![1, 2, 3]);
    }

    #[test]
    fn test_raft_node_roles() {
        let mut node = RaftNode::new("node-1".to_string());

        assert_eq!(node.role, RaftRole::Follower);

        node.become_candidate();
        assert_eq!(node.role, RaftRole::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(node.voted_for, Some("node-1".to_string()));

        node.become_leader();
        assert_eq!(node.role, RaftRole::Leader);
        assert_eq!(
            node.log.entries.last().unwrap().entry_type,
            LogEntryType::Noop
        );

        node.become_follower(5);
        assert_eq!(node.role, RaftRole::Follower);
        assert_eq!(node.current_term, 5);
        assert_eq!(node.voted_for, None);
    }
}
