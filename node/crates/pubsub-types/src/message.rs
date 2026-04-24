use std::collections::BTreeMap;
use std::fmt;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Format tag for the message envelope.
/// Allows future encoding formats without breaking the wire protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum EncodingFormat {
    Cbor = 0,
    // Future: Protobuf = 1, FlatBuffers = 2, etc.
}

/// Core message envelope as defined in the architecture doc.
/// Every field except `metadata` is mandatory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Topic this message belongs to (from on-chain Topic Registry)
    pub topic_id: TopicId,

    /// Per-publisher, per-topic monotonic counter
    pub sequence_nr: u64,

    /// Millisecond timestamp (publisher's local clock)
    pub timestamp_ms: u64,

    /// Publisher's public key or DID
    pub publisher_id: PublisherId,

    /// Signature over (topic_id, sequence_nr, timestamp_ms, publisher_id, payload, metadata)
    pub signature: Bytes,

    /// Application content
    pub payload: Bytes,

    /// Reserved for future extensions (hop count, attestation digest, etc.)
    /// Empty map in Phase 1.
    pub metadata: BTreeMap<String, Bytes>,
}

/// 256-bit topic identifier assigned by the on-chain Topic Registry
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct TopicId(pub [u8; 32]);

impl fmt::Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in &self.0[..4] {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

/// Publisher identity — public key bytes for now, extensible to DIDs later
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublisherId(pub Bytes);

impl fmt::Display for PublisherId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.0.len().min(4);
        for b in &self.0[..len] {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

/// Unique identifier for a message within the network
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId {
    pub topic_id: TopicId,
    pub publisher_id: PublisherId,
    pub sequence_nr: u64,
}

impl Message {
    /// Compute the deterministic message ID (used for deduplication and DHT indexing)
    /// key = BLAKE2b(topicId || publisherId || sequenceNr)
    pub fn id(&self) -> MessageId {
        MessageId {
            topic_id: self.topic_id.clone(),
            publisher_id: self.publisher_id.clone(),
            sequence_nr: self.sequence_nr,
        }
    }

    /// Byte representation of the fields that are signed
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.topic_id.0);
        buf.extend_from_slice(&self.sequence_nr.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        buf.extend_from_slice(&self.publisher_id.0);
        buf.extend_from_slice(&self.payload);
        // Include metadata keys in deterministic order (BTreeMap guarantees this)
        for (k, v) in &self.metadata {
            buf.extend_from_slice(k.as_bytes());
            buf.extend_from_slice(v);
        }
        buf
    }
}
