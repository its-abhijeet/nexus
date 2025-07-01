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

/// RequestVote RPC: Candidate → Peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteRequest {
    pub term: u64,
    pub candidate_id: String,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

/// Response to RequestVote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_append_entries() {
        let req = AppendEntriesRequest {
            term: 1,
            leader_id: "leader1".into(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let encoded = bincode::serialize(&req).unwrap();
        let decoded: AppendEntriesRequest = bincode::deserialize(&encoded).unwrap();

        assert_eq!(decoded.term, 1);
        assert_eq!(decoded.leader_id, "leader1");
    }
}
