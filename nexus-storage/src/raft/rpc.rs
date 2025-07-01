use super::log::LogEntry;
use nexus_common::types::{NodeId, Term};
use serde::{Deserialize, Serialize};

/// Sent by leader to replicate log entries or as heartbeat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    pub term: Term,             // Leader’s term
    pub leader_id: NodeId,      // Leader's ID
    pub prev_log_index: u64,    // Index of log entry before new ones
    pub prev_log_term: Term,    // Term of that entry
    pub entries: Vec<LogEntry>, // New log entries to store
    pub leader_commit: u64,     // Leader’s commit index
}

/// Response from follower to AppendEntries RPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: Term,    // Current term (may be newer)
    pub success: bool, // True if follower appended entries
}
