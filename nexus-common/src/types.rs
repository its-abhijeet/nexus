pub type NodeId = String;
pub type StreamId = String;
pub type EventId = uuid::Uuid;
pub type AggregatedId = String;
pub type Version = u64;
pub type Term = u64;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAddress {
    pub host: String,
    pub port: u16,
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub nodes: Vec<NodeAddress>,
    pub replication_factor: usize,
    pub election_timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_address_serialization() {
        let node = NodeAddress {
            host: "127.0.0.1".into(),
            port: 8080,
            node_id: "node-1".into(),
        };

        let json = serde_json::to_string(&node).unwrap();
        let deserialized: NodeAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(node.node_id, deserialized.node_id);
    }
}
