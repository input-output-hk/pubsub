pub mod message;
pub mod topic;
pub mod node;
pub mod traits;
pub mod error;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use bytes::Bytes;

    use crate::message::{Message, MessageId, PublisherId, TopicId};
    use crate::node::{NodeId, NodeInfo};
    use crate::topic::TopicConfig;

    fn make_topic_id(seed: u8) -> TopicId {
        TopicId([seed; 32])
    }

    fn make_publisher_id(seed: u8) -> PublisherId {
        PublisherId(Bytes::from(vec![seed; 32]))
    }

    fn make_message(topic_seed: u8, seq: u64) -> Message {
        Message {
            topic_id: make_topic_id(topic_seed),
            sequence_nr: seq,
            timestamp_ms: 1_000_000,
            publisher_id: make_publisher_id(0xAB),
            signature: Bytes::from(vec![0u8; 64]),
            payload: Bytes::from("hello"),
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn topic_id_equality() {
        let a = TopicId([1u8; 32]);
        let b = TopicId([1u8; 32]);
        let c = TopicId([2u8; 32]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn message_id_from_message() {
        let msg = make_message(1, 42);
        let id = msg.id();
        assert_eq!(id.topic_id, msg.topic_id);
        assert_eq!(id.sequence_nr, 42);
        assert_eq!(id.publisher_id, msg.publisher_id);
    }

    #[test]
    fn signable_bytes_deterministic() {
        let msg = make_message(5, 7);
        let b1 = msg.signable_bytes();
        let b2 = msg.signable_bytes();
        assert_eq!(b1, b2);
        assert!(!b1.is_empty());
    }

    #[test]
    fn signable_bytes_includes_all_fields() {
        let mut msg = make_message(5, 7);
        let base = msg.signable_bytes();

        // Changing payload changes signable bytes
        msg.payload = Bytes::from("different");
        assert_ne!(base, msg.signable_bytes());
    }

    #[test]
    fn open_topic_authorizes_any_publisher() {
        let config = TopicConfig {
            topic_id: make_topic_id(1),
            name: "test".into(),
            description: None,
            authorized_publishers: vec![],
            retention_period: Duration::from_secs(60),
            replication_factor: 1,
        };
        assert!(!config.is_moderated());
        assert!(config.is_authorized(&make_publisher_id(0)));
        assert!(config.is_authorized(&make_publisher_id(0xFF)));
    }

    #[test]
    fn restricted_topic_rejects_unauthorized() {
        let allowed = make_publisher_id(1);
        let banned = make_publisher_id(2);
        let config = TopicConfig {
            topic_id: make_topic_id(2),
            name: "restricted".into(),
            description: None,
            authorized_publishers: vec![allowed.clone()],
            retention_period: Duration::from_secs(60),
            replication_factor: 1,
        };
        assert!(config.is_moderated());
        assert!(config.is_authorized(&allowed));
        assert!(!config.is_authorized(&banned));
    }

    #[test]
    fn node_id_hash_and_eq() {
        use std::collections::HashSet;
        let a = NodeId([1u8; 32]);
        let b = NodeId([1u8; 32]);
        let c = NodeId([2u8; 32]);
        assert_eq!(a, b);
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
        assert!(!set.contains(&c));
    }
}
