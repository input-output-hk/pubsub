use async_trait::async_trait;
use bytes::Bytes;
use tracing::debug;

use pubsub_types::error::PubSubError;
use pubsub_types::message::TopicId;
use pubsub_types::node::{NodeId, NodeInfo};
use pubsub_types::topic::TopicConfig;
use pubsub_types::traits::ChainState;

/// Fixed stake value returned for all nodes in mock mode.
const MOCK_STAKE: u64 = 1_000_000;

/// Mock chain state that serves data from in-memory lists.
///
/// Used for testnet and local development. Will be replaced by an
/// ogmios / cardano-node local-socket reader in production.
pub struct MockChainState {
    nodes: Vec<NodeInfo>,
    topics: Vec<TopicConfig>,
    /// Ed25519 keys registered as KES keys for some stake pool.
    pool_kes_keys: Vec<Bytes>,
    /// Ed25519 keys registered as DRep credentials (CIP-1694).
    drep_keys: Vec<Bytes>,
    /// Ed25519 keys authorised to publish emergency alerts.
    authority_keys: Vec<Bytes>,
}

impl MockChainState {
    /// Create a new mock chain state from nodes and topic configs.
    /// Credential registries are empty by default; use the builder
    /// methods to populate them for testing.
    pub fn new(nodes: Vec<NodeInfo>, topics: Vec<TopicConfig>) -> Self {
        debug!(
            num_nodes = nodes.len(),
            num_topics = topics.len(),
            "Initialized MockChainState"
        );
        Self {
            nodes,
            topics,
            pool_kes_keys: Vec::new(),
            drep_keys: Vec::new(),
            authority_keys: Vec::new(),
        }
    }

    /// Create an empty mock chain state.
    pub fn empty() -> Self {
        Self::new(Vec::new(), Vec::new())
    }

    /// Register a key as a valid pool KES key.
    pub fn with_pool_kes_key(mut self, key: impl Into<Bytes>) -> Self {
        self.pool_kes_keys.push(key.into());
        self
    }

    /// Register a key as a valid DRep credential.
    pub fn with_drep_key(mut self, key: impl Into<Bytes>) -> Self {
        self.drep_keys.push(key.into());
        self
    }

    /// Register a key as an authority key.
    pub fn with_authority_key(mut self, key: impl Into<Bytes>) -> Self {
        self.authority_keys.push(key.into());
        self
    }
}

#[async_trait]
impl ChainState for MockChainState {
    async fn get_registered_nodes(&self) -> Result<Vec<NodeInfo>, PubSubError> {
        debug!(count = self.nodes.len(), "MockChainState: get_registered_nodes");
        Ok(self.nodes.clone())
    }

    async fn get_topic_config(
        &self,
        topic: &TopicId,
    ) -> Result<Option<TopicConfig>, PubSubError> {
        let config = self
            .topics
            .iter()
            .find(|tc| tc.topic_id == *topic)
            .cloned();
        debug!(
            topic = %topic,
            found = config.is_some(),
            "MockChainState: get_topic_config"
        );
        Ok(config)
    }

    async fn get_all_topics(&self) -> Result<Vec<TopicConfig>, PubSubError> {
        debug!(count = self.topics.len(), "MockChainState: get_all_topics");
        Ok(self.topics.clone())
    }

    async fn get_node_stake(&self, node: &NodeId) -> Result<u64, PubSubError> {
        debug!(node = %node, stake = MOCK_STAKE, "MockChainState: get_node_stake (fixed)");
        Ok(MOCK_STAKE)
    }

    async fn get_pool_kes_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        debug!(count = self.pool_kes_keys.len(), "MockChainState: get_pool_kes_keys");
        Ok(self.pool_kes_keys.clone())
    }

    async fn get_drep_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        debug!(count = self.drep_keys.len(), "MockChainState: get_drep_keys");
        Ok(self.drep_keys.clone())
    }

    async fn get_authority_keys(&self) -> Result<Vec<Bytes>, PubSubError> {
        debug!(count = self.authority_keys.len(), "MockChainState: get_authority_keys");
        Ok(self.authority_keys.clone())
    }
}
