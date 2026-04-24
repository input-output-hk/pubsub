use async_trait::async_trait;
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
}

impl MockChainState {
    /// Create a new mock chain state from a set of nodes and topic configs.
    pub fn new(nodes: Vec<NodeInfo>, topics: Vec<TopicConfig>) -> Self {
        debug!(
            num_nodes = nodes.len(),
            num_topics = topics.len(),
            "Initialized MockChainState"
        );
        Self { nodes, topics }
    }

    /// Create an empty mock chain state.
    pub fn empty() -> Self {
        Self::new(Vec::new(), Vec::new())
    }

    /// Add a node to the mock registry.
    pub fn add_node(&mut self, node: NodeInfo) {
        self.nodes.push(node);
    }

    /// Add a topic configuration to the mock registry.
    pub fn add_topic(&mut self, topic: TopicConfig) {
        self.topics.push(topic);
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
        debug!(node = ?node, stake = MOCK_STAKE, "MockChainState: get_node_stake (fixed)");
        Ok(MOCK_STAKE)
    }
}
