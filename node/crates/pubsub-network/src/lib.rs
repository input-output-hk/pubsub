pub mod transport;
pub mod cyclon;
pub mod vicinity;
pub mod dissemination;
pub mod codec;
pub mod validator;
pub mod relay_policy;
pub mod store;
pub mod mock_chain;
pub mod mock_registry;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;

    use pubsub_types::message::{Message, MessageId, PublisherId, TopicId};
    use pubsub_types::node::{NodeId, NodeInfo};
    use pubsub_types::topic::TopicConfig;
    use pubsub_types::traits::{
        ChainState, Codec, MessageStore, RelayDecision, RelayPolicy,
    };

    use crate::codec::CborCodec;
    use crate::mock_chain::MockChainState;
    use crate::relay_policy::DefaultRelayPolicy;
    use crate::store::HotCache;

    fn make_topic_id(seed: u8) -> TopicId {
        TopicId([seed; 32])
    }

    fn make_message(topic_seed: u8, seq: u64) -> Message {
        Message {
            topic_id: make_topic_id(topic_seed),
            sequence_nr: seq,
            timestamp_ms: 42_000,
            publisher_id: PublisherId(Bytes::from(vec![0xABu8; 32])),
            signature: Bytes::from(vec![0u8; 64]),
            payload: Bytes::from(format!("payload-{seq}")),
            metadata: BTreeMap::new(),
        }
    }

    // ── Codec ────────────────────────────────────────────────────────────────

    #[test]
    fn cbor_codec_roundtrip() {
        let codec = CborCodec;
        let msg = make_message(1, 99);
        let encoded = codec.encode(&msg).expect("encode");
        let decoded = codec.decode(&encoded).expect("decode");
        assert_eq!(decoded.topic_id, msg.topic_id);
        assert_eq!(decoded.sequence_nr, msg.sequence_nr);
        assert_eq!(decoded.payload, msg.payload);
    }

    #[test]
    fn cbor_codec_rejects_garbage() {
        let codec = CborCodec;
        assert!(codec.decode(b"not cbor data").is_err());
    }

    // ── HotCache ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn hot_cache_store_and_get() {
        let cache = HotCache::with_defaults();
        let msg = make_message(2, 1);
        let id = msg.id();

        cache.store(msg.clone()).await.expect("store");
        let got = cache.get(&id).await.expect("get");
        assert!(got.is_some());
        assert_eq!(got.unwrap().sequence_nr, 1);
    }

    #[tokio::test]
    async fn hot_cache_get_missing_returns_none() {
        let cache = HotCache::with_defaults();
        let id = MessageId {
            topic_id: make_topic_id(99),
            publisher_id: PublisherId(Bytes::from(vec![0u8; 32])),
            sequence_nr: 0,
        };
        let got = cache.get(&id).await.expect("get");
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn hot_cache_get_since_filters_correctly() {
        let cache = HotCache::with_defaults();
        let topic = make_topic_id(3);

        for seq in 1u64..=5 {
            cache.store(make_message(3, seq)).await.unwrap();
        }

        // get_since(2) should return seqs 3, 4, 5
        let msgs = cache
            .get_since(&topic, 2, 100)
            .await
            .expect("get_since");
        assert_eq!(msgs.len(), 3);
        assert!(msgs.iter().all(|m| m.sequence_nr > 2));
    }

    #[tokio::test]
    async fn hot_cache_get_since_limit_respected() {
        let cache = HotCache::with_defaults();
        for seq in 1u64..=10 {
            cache.store(make_message(4, seq)).await.unwrap();
        }
        let topic = make_topic_id(4);
        let msgs = cache.get_since(&topic, 0, 3).await.expect("get_since");
        assert_eq!(msgs.len(), 3);
    }

    // ── MockChainState ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn mock_chain_topic_lookup_found() {
        let topic_id = make_topic_id(10);
        let config = TopicConfig {
            topic_id: topic_id.clone(),
            name: "test-topic".into(),
            description: None,
            authorized_publishers: vec![],
            retention_period: Duration::from_secs(3600),
            replication_factor: 1,
        };
        let chain = MockChainState::new(vec![], vec![config]);
        let result = chain.get_topic_config(&topic_id).await.expect("query");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "test-topic");
    }

    #[tokio::test]
    async fn mock_chain_topic_lookup_missing() {
        let chain = MockChainState::empty();
        let result = chain
            .get_topic_config(&make_topic_id(99))
            .await
            .expect("query");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn mock_chain_node_stake_always_fixed() {
        let chain = MockChainState::empty();
        let stake = chain
            .get_node_stake(&NodeId([0u8; 32]))
            .await
            .expect("stake");
        assert_eq!(stake, 1_000_000);
    }

    // ── RelayPolicy ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn relay_policy_forwards_all() {
        let policy = DefaultRelayPolicy;
        let msg = make_message(5, 1);
        let from = NodeId([0u8; 32]);
        let decision = policy.should_relay(&msg, &from).await;
        assert_eq!(decision, RelayDecision::Forward);
    }
}
