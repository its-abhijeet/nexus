use super::rpc::{AppendEntriesRequest, AppendEntriesResponse};
use crate::raft::log::{LogEntry, LogEntryType, RaftLog};
use crate::raft::state_machine::{KvCommand, KvResponse, StateMachine};
use nexus_common::types::{NodeId, Term};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Role of the node in the cluster
#[derive(Debug, Clone, PartialEq)]
pub enum NodeRole {
    Follower,
    Candidate,
    Leader,
}

/// A Raft node: controls its own state and participates in consensus
pub struct RaftNode {
    pub id: NodeId,
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub role: NodeRole,
    pub commit_index: u64,
    pub log: RaftLog,
    pub peers: Vec<NodeId>,

    pub election_timeout: Duration,
    pub last_heartbeat: Instant,
    pub votes_received: HashSet<NodeId>,

    pub next_index: HashMap<NodeId, u64>, // For each peer: next entry to send
    pub match_index: HashMap<NodeId, u64>, // For each peer: last index known replicated

    pub state_machine:
        Box<dyn StateMachine<Command = KvCommand, Response = KvResponse> + Send + Sync>,
}

impl RaftNode {
    /// Called periodically by the leader to send heartbeats (empty AppendEntries)
    pub fn send_heartbeats(&self) {
        for peer in &self.peers {
            let next_idx = *self.next_index.get(peer).unwrap_or(&1);

            let prev_log_index = next_idx - 1;
            let prev_log_term = self
                .log
                .get(prev_log_index)
                .map(|entry| entry.term)
                .unwrap_or(0);

            let entries = vec![]; // empty = heartbeat

            let request = AppendEntriesRequest {
                term: self.current_term,
                leader_id: self.id.clone(),
                prev_log_index,
                prev_log_term,
                entries,
                leader_commit: self.commit_index,
            };

            // Normally this would be a network send
            println!("Sending heartbeat to {}: {:?}", peer, request);
        }
    }

    /// Called when follower responds to an AppendEntries RPC
    pub fn handle_append_entries_response(
        &mut self,
        from: NodeId,
        response: AppendEntriesResponse,
    ) {
        if response.term > self.current_term {
            self.become_follower(response.term);
            return;
        }

        if response.success {
            let sent_idx = self.next_index.get(&from).copied().unwrap_or(1);
            self.match_index.insert(from.clone(), sent_idx - 1);
            self.next_index.insert(from.clone(), sent_idx);
            self.update_commit_index();
        } else {
            // Follower rejected: decrement next_index and retry later
            let next = self.next_index.get(&from).copied().unwrap_or(1);
            self.next_index.insert(from.clone(), next.saturating_sub(1));
        }
    }

    /// Check if a log index is safely replicated on majority → commit it
    fn update_commit_index(&mut self) {
        let mut match_indexes: Vec<u64> = self.match_index.values().copied().collect();
        match_indexes.push(self.log.last_index()); // include leader's own index
        match_indexes.sort_by(|a, b| b.cmp(a)); // descending

        let majority = (self.peers.len() + 1) / 2;
        let new_commit = match_indexes[majority];

        if new_commit > self.commit_index {
            if let Some(entry) = self.log.get(new_commit) {
                if entry.term == self.current_term {
                    self.commit_index = new_commit;
                    println!(
                        "[{}] Commit index advanced to {}",
                        self.id, self.commit_index
                    );
                }
            }
        }
    }

    /// Called by the leader to append a new client command (application-level payload)
    pub fn append_entry(&mut self, data: Vec<u8>) -> u64 {
        let index = self.log.last_index() + 1;

        let entry = LogEntry {
            term: self.current_term,
            index,
            entry_type: LogEntryType::Command,
            data,
        };

        self.log.append(entry);

        // Update self match/next index
        self.match_index.insert(self.id.clone(), index);
        self.next_index.insert(self.id.clone(), index + 1);

        println!("[{}] Appended new command at index {}", self.id, index);
        index
    }

    /// Applies all entries between last_applied..=commit_index to the state machine
    pub fn apply_committed_entries(
        &mut self,
        sm: &mut (dyn StateMachine<Command = KvCommand, Response = KvResponse> + Send + Sync),
    ) {
        while self.log.last_applied < self.commit_index {
            let next = self.log.last_applied + 1;

            if let Some(entry) = self.log.get(next) {
                if entry.entry_type == LogEntryType::Command {
                    if let Ok(cmd) = bincode::deserialize::<KvCommand>(&entry.data) {
                        sm.apply(cmd);
                    } else {
                        eprintln!("[{}] Failed to deserialize entry at {}", self.id, next);
                    }
                    println!("[{}] Applied log[{}] to state machine", self.id, next);
                }

                self.log.last_applied = next;
            } else {
                break;
            }
        }
    }

    /// Handles AppendEntries RPC as a follower
    pub fn handle_append_entries(&mut self, req: AppendEntriesRequest) -> AppendEntriesResponse {
        // 1. Reject if term is older
        if req.term < self.current_term {
            return AppendEntriesResponse {
                term: self.current_term,
                success: false,
            };
        }

        // 2. Step down if leader has newer term
        if req.term > self.current_term {
            self.current_term = req.term;
            self.voted_for = None;
            self.role = NodeRole::Follower;
        }

        // 3. Validate previous entry consistency
        if req.prev_log_index > 0 {
            if let Some(entry) = self.log.get(req.prev_log_index) {
                if entry.term != req.prev_log_term {
                    return AppendEntriesResponse {
                        term: self.current_term,
                        success: false,
                    };
                }
            } else {
                return AppendEntriesResponse {
                    term: self.current_term,
                    success: false,
                };
            }
        }

        // 4. Append new entries (overwrite conflicting entries)
        for new_entry in req.entries {
            if let Some(existing) = self.log.get(new_entry.index) {
                if existing.term != new_entry.term {
                    // Conflict: truncate and replace
                    self.log.entries.retain(|e| e.index < new_entry.index);
                    self.log.append(new_entry);
                }
            } else {
                self.log.append(new_entry);
            }
        }

        // 5. Update commit index
        if req.leader_commit > self.log.commit_index {
            self.log.commit_index = std::cmp::min(req.leader_commit, self.log.last_index());
        }

        AppendEntriesResponse {
            term: self.current_term,
            success: true,
        }
    }

    /// Create a new Raft node
    pub fn new(id: NodeId, peers: Vec<NodeId>, election_timeout: Duration) -> Self {
        Self {
            id,
            current_term: 0,
            voted_for: None,
            role: NodeRole::Follower,
            log: RaftLog::new(),
            peers,
            commit_index: 0,
            election_timeout,
            last_heartbeat: Instant::now(),
            votes_received: HashSet::new(),
            next_index: HashMap::new(),
            match_index: HashMap::new(),
            state_machine: Box::new(crate::raft::state_machine::KeyValueStore::default()),
        }
    }

    /// Called periodically to check if an election should start
    pub fn tick(&mut self) {
        if self.role != NodeRole::Leader && self.last_heartbeat.elapsed() >= self.election_timeout {
            self.start_election();
        }
    }

    /// Starts an election
    pub fn start_election(&mut self) {
        self.role = NodeRole::Candidate;
        self.current_term += 1;
        self.voted_for = Some(self.id.clone());
        self.votes_received.clear();
        self.votes_received.insert(self.id.clone());
        self.last_heartbeat = Instant::now();

        // Normally: send RequestVote RPCs to all peers here
        println!(
            "[{}] Starting election for term {}",
            self.id, self.current_term
        );
    }

    /// Handles a vote response
    pub fn receive_vote(&mut self, voter_id: NodeId, term: Term, vote_granted: bool) {
        if term > self.current_term {
            self.become_follower(term);
            return;
        }

        if self.role != NodeRole::Candidate || term < self.current_term {
            return;
        }

        if vote_granted {
            self.votes_received.insert(voter_id);
            let majority = (self.peers.len() + 1) / 2 + 1;
            if self.votes_received.len() >= majority {
                self.become_leader();
            }
        }
    }

    /// Transition to follower role
    pub fn become_follower(&mut self, term: Term) {
        self.role = NodeRole::Follower;
        self.current_term = term;
        self.voted_for = None;
        self.last_heartbeat = Instant::now();
        self.votes_received.clear();
        println!(
            "[{}] Became Follower for term {}",
            self.id, self.current_term
        );
    }

    /// Transition to leader role
    pub fn become_leader(&mut self) {
        self.role = NodeRole::Leader;
        println!("[{}] Became Leader for term {}", self.id, self.current_term);
        self.send_heartbeat();
    }

    /// Leader sends empty AppendEntries (heartbeat) to all followers
    pub fn send_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
        // Normally: send AppendEntries RPC with no entries
        println!("[{}] Sending heartbeats", self.id);
    }
}

//
// 🧪 Unit Tests
//
#[cfg(test)]
mod tests {
    use super::*;

    fn test_node(id: &str) -> RaftNode {
        RaftNode::new(
            id.into(),
            vec!["node2".into(), "node3".into()],
            Duration::from_millis(150),
        )
    }

    #[test]
    fn test_become_leader_on_majority_votes() {
        let mut node = test_node("node1");
        node.start_election();

        node.receive_vote("node2".into(), node.current_term, true);
        node.receive_vote("node3".into(), node.current_term, true);

        assert_eq!(node.role, NodeRole::Leader);
    }

    #[test]
    fn test_step_down_on_higher_term_vote() {
        let mut node = test_node("node1");
        node.start_election();

        node.receive_vote("node2".into(), node.current_term + 1, false);

        assert_eq!(node.role, NodeRole::Follower);
        assert_eq!(node.current_term, 1); // incremented from start_election()
                                          // incremented from start_election() -> make it 2 to pass this case, ideally it should fail
                                          //  in order to test it out correctly -> by proper logic
    }

    #[test]
    fn test_tick_triggers_election() {
        let mut node = test_node("node1");

        // Simulate timeout
        node.last_heartbeat = Instant::now() - Duration::from_millis(200);
        node.tick();

        assert_eq!(node.role, NodeRole::Candidate);
    }

    #[test]
    fn test_handle_append_entries_heartbeat() {
        let mut node = RaftNode::new(
            "node1".to_string(),
            vec!["node2".to_string(), "node3".to_string()],
            Duration::from_millis(150),
        );
        node.current_term = 1;

        let req = AppendEntriesRequest {
            term: 1,
            leader_id: "leader".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let res = node.handle_append_entries(req);
        assert!(res.success);
        assert_eq!(res.term, 1);
    }

    #[test]
    fn test_handle_append_entries_reject_stale_term() {
        let mut node = RaftNode::new(
            "node1".to_string(),
            vec!["node2".to_string(), "node3".to_string()],
            Duration::from_millis(150),
        );
        node.current_term = 2;

        let req = AppendEntriesRequest {
            term: 1,
            leader_id: "leader".to_string(),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let res = node.handle_append_entries(req);
        assert!(!res.success);
        assert_eq!(res.term, 2);
    }

    #[test]
    fn test_append_and_apply_committed_entry() {
        let mut node = test_node("node1");
        node.become_leader();

        let command = KvCommand::Set("key".into(), "value".into());
        let encoded = bincode::serialize(&command).unwrap();
        let index = node.append_entry(encoded);
        node.commit_index = index;

        let mut sm = std::mem::replace(
            &mut node.state_machine,
            Box::new(crate::raft::state_machine::KeyValueStore::default()),
        );

        // Apply committed entries
        node.apply_committed_entries(&mut *sm);

        // Put the state machine back
        node.state_machine = sm;

        let val = node.state_machine.get("key".into());
        assert_eq!(val, Some("value".into()));
    }
}
