use std::path::Path;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::Deserialize;
use tracing::{debug, info};

use pubsub_types::error::PubSubError;
use pubsub_types::node::{node_id_from_addr, NodeId, NodeInfo};
use pubsub_types::topic::topic_id_from_name;
use pubsub_types::traits::NodeRegistry;

// ---------------------------------------------------------------------------
// JSON schema (what nodes.json looks like on disk)
// ---------------------------------------------------------------------------

/// One entry in nodes.json.
#[derive(Deserialize)]
struct RegistryEntry {
    addr: SocketAddr,
    /// Hex-encoded Ed25519 public key.  If absent or empty the NodeId is
    /// derived deterministically from the socket address (testnet only).
    #[serde(default)]
    public_key: Option<String>,
    #[serde(default)]
    subscribed_topics: Vec<String>,
}

#[derive(Deserialize)]
struct RegistryFile {
    nodes: Vec<RegistryEntry>,
}

// ---------------------------------------------------------------------------
// MockNodeRegistry
// ---------------------------------------------------------------------------

/// In-memory node registry backed by a DashMap.
///
/// Can be initialised from a JSON config file (see `nodes.json` schema above)
/// or from a `Vec<NodeInfo>` for tests.
pub struct MockNodeRegistry {
    nodes: DashMap<NodeId, NodeInfo>,
}

impl Default for MockNodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MockNodeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new(),
        }
    }

    /// Initialise the registry from a slice of `NodeInfo` values.
    pub fn from_nodes(nodes: Vec<NodeInfo>) -> Self {
        let registry = Self::new();
        for node in nodes {
            registry.nodes.insert(node.node_id.clone(), node);
        }
        registry
    }

    /// Load the registry from a JSON file.
    ///
    /// File format:
    /// ```json
    /// {
    ///   "nodes": [
    ///     {
    ///       "addr": "127.0.0.1:9001",
    ///       "public_key": null,
    ///       "subscribed_topics": ["ops/emergency/critical", "gov/drep/test"]
    ///     }
    ///   ]
    /// }
    /// ```
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading registry file {}", path.display()))?;
        Self::from_str(&content)
            .with_context(|| format!("parsing registry file {}", path.display()))
    }

    /// Parse the registry from a JSON string (used by `from_file` and tests).
    pub fn from_str(json: &str) -> Result<Self> {
        let file: RegistryFile = serde_json::from_str(json)
            .context("invalid registry JSON")?;

        let nodes: Vec<NodeInfo> = file
            .nodes
            .into_iter()
            .map(|entry| {
                let node_id = derive_node_id(&entry);
                let subscribed_topics = entry
                    .subscribed_topics
                    .iter()
                    .map(|n| topic_id_from_name(n))
                    .collect();
                NodeInfo {
                    node_id,
                    addr: entry.addr,
                    public_key: entry
                        .public_key
                        .as_deref()
                        .map(hex_decode_or_empty)
                        .unwrap_or_default(),
                    subscribed_topics,
                }
            })
            .collect();

        debug!(count = nodes.len(), "Loaded registry");
        Ok(Self::from_nodes(nodes))
    }
}

#[async_trait]
impl NodeRegistry for MockNodeRegistry {
    async fn register(
        &self,
        info: NodeInfo,
        _commitment_epochs: u32,
    ) -> Result<(), PubSubError> {
        info!(
            node_id = %info.node_id,
            addr = %info.addr,
            "MockNodeRegistry: registered node"
        );
        self.nodes.insert(info.node_id.clone(), info);
        Ok(())
    }

    async fn deregister(&self, node_id: &NodeId) -> Result<(), PubSubError> {
        self.nodes.remove(node_id);
        debug!(node_id = %node_id, "MockNodeRegistry: deregistered node");
        Ok(())
    }

    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError> {
        let nodes: Vec<NodeInfo> = self.nodes.iter().map(|r| r.value().clone()).collect();
        debug!(count = nodes.len(), "MockNodeRegistry: get_registered_nodes");
        Ok(nodes)
    }

    async fn get_node(&self, node_id: &NodeId) -> Result<Option<NodeInfo>, PubSubError> {
        let node = self.nodes.get(node_id).map(|r| r.value().clone());
        debug!(
            node_id = %node_id,
            found = node.is_some(),
            "MockNodeRegistry: get_node"
        );
        Ok(node)
    }

    async fn is_registered(&self, node_id: &NodeId) -> Result<bool, PubSubError> {
        Ok(self.nodes.contains_key(node_id))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a NodeId for a registry entry.
/// Prefers the hex-encoded public key; falls back to a BLAKE2b hash of the
/// socket address string so the id is stable across restarts.
fn derive_node_id(entry: &RegistryEntry) -> NodeId {
    if let Some(pk) = &entry.public_key {
        let bytes = hex_decode_or_empty(pk);
        if bytes.len() >= 32 {
            let mut id = [0u8; 32];
            id.copy_from_slice(&bytes[..32]);
            return NodeId(id);
        }
    }
    node_id_from_addr(entry.addr)
}

fn hex_decode_or_empty(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use pubsub_types::node::NodeId;

    fn make_node(port: u16) -> NodeInfo {
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
        NodeInfo {
            node_id: node_id_from_addr(addr),
            addr,
            public_key: vec![],
            subscribed_topics: vec![],
        }
    }

    #[tokio::test]
    async fn from_nodes_count() {
        let nodes = vec![make_node(9001), make_node(9002), make_node(9003)];
        let reg = MockNodeRegistry::from_nodes(nodes);
        assert_eq!(reg.get_registered_nodes().await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn register_and_retrieve() {
        let reg = MockNodeRegistry::new();
        let node = make_node(9010);
        let id = node.node_id.clone();

        reg.register(node.clone(), 0).await.unwrap();

        assert!(reg.is_registered(&id).await.unwrap());
        let got = reg.get_node(&id).await.unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().addr, node.addr);
    }

    #[tokio::test]
    async fn deregister_removes_node() {
        let reg = MockNodeRegistry::new();
        let node = make_node(9020);
        let id = node.node_id.clone();

        reg.register(node, 0).await.unwrap();
        assert!(reg.is_registered(&id).await.unwrap());

        reg.deregister(&id).await.unwrap();
        assert!(!reg.is_registered(&id).await.unwrap());
    }

    #[tokio::test]
    async fn get_node_missing_returns_none() {
        let reg = MockNodeRegistry::new();
        let id = NodeId([0xFFu8; 32]);
        assert!(reg.get_node(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn register_updates_existing() {
        let reg = MockNodeRegistry::new();
        let mut node = make_node(9030);
        let id = node.node_id.clone();

        reg.register(node.clone(), 0).await.unwrap();

        // Update public key
        node.public_key = vec![1, 2, 3];
        reg.register(node, 0).await.unwrap();

        let got = reg.get_node(&id).await.unwrap().unwrap();
        assert_eq!(got.public_key, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn from_str_parses_json() {
        let json = r#"{
            "nodes": [
                {
                    "addr": "127.0.0.1:9001",
                    "subscribed_topics": ["ops/emergency/critical", "gov/drep/test"]
                },
                {
                    "addr": "127.0.0.1:9002",
                    "subscribed_topics": []
                }
            ]
        }"#;

        let reg = MockNodeRegistry::from_str(json).unwrap();
        let nodes = reg.get_registered_nodes().await.unwrap();
        assert_eq!(nodes.len(), 2);

        // Check topics were parsed
        let n9001 = nodes.iter().find(|n| n.addr.port() == 9001).unwrap();
        assert_eq!(n9001.subscribed_topics.len(), 2);
    }

    #[tokio::test]
    async fn from_str_derives_stable_node_ids() {
        let json = r#"{"nodes": [{"addr": "127.0.0.1:9001"}]}"#;
        let reg1 = MockNodeRegistry::from_str(json).unwrap();
        let reg2 = MockNodeRegistry::from_str(json).unwrap();

        let id1 = reg1.get_registered_nodes().await.unwrap()[0].node_id.clone();
        let id2 = reg2.get_registered_nodes().await.unwrap()[0].node_id.clone();
        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn from_file_roundtrip() {
        let json = r#"{
            "nodes": [
                {"addr": "127.0.0.1:9001", "subscribed_topics": ["topic-a"]},
                {"addr": "127.0.0.1:9002", "subscribed_topics": ["topic-b"]}
            ]
        }"#;

        // Write to a temp file
        let tmp = std::env::temp_dir().join("pubsub_test_registry.json");
        std::fs::write(&tmp, json).unwrap();

        let reg = MockNodeRegistry::from_file(&tmp).unwrap();
        assert_eq!(reg.get_registered_nodes().await.unwrap().len(), 2);

        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn from_file_missing_returns_error() {
        let result = MockNodeRegistry::from_file(Path::new("/nonexistent/path/registry.json"));
        assert!(result.is_err());
    }
}
