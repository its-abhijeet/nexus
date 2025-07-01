use crate::error::Result;
use crate::types::ClusterConfig;
use std::fs;

/// Loads a cluster configuration from a JSON file.
pub fn load_config(path: &str) -> Result<ClusterConfig> {
    let data = fs::read_to_string(path)?;
    let config: ClusterConfig = serde_json::from_str(&data)?;
    Ok(config)
}
