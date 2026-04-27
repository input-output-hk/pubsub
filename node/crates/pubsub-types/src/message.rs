use std::collections::BTreeMap;
use std::fmt;

use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// Discriminant committed to in `signable_bytes()` so signatures are
/// credential-type-specific and cannot be replayed across types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum CredentialType {
    /// Generic Ed25519 key — testnet publishers and dApp operators.
    /// Authorization is checked against the Topic Registry's
    /// `authorized_publishers` list (or open if that list is empty).
    Ed25519 = 0,
    /// SPO KES key — operational certificate chain proves pool membership.
    /// Phase 1: Ed25519 signature verified; key checked against mock pool
    /// KES registry.  Real KES chain verification is future work.
    PoolKes = 1,
    /// CIP-1694 DRep signing key — verified against on-chain DRep registration.
    /// Phase 1: Ed25519 signature verified; key checked against mock DRep registry.
    DRepCredential = 2,
    /// Emergency alert authority key.  May be a single key or eventually
    /// upgraded to multi-sig.  Checked against a curated authority list.
    AuthorityKey = 3,
}

impl CredentialType {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialType::Ed25519 => "ed25519",
            CredentialType::PoolKes => "pool-kes",
            CredentialType::DRepCredential => "drep",
            CredentialType::AuthorityKey => "authority",
        }
    }
}

/// Typed publisher credential carried in every message.
///
/// All four types use Ed25519 as the signing primitive in Phase 1.
/// What differs is which on-chain registry is queried to confirm the key
/// is legitimate for the network role it claims.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublisherCredential {
    pub credential_type: CredentialType,
    /// The Ed25519 verification key (32 bytes).
    pub key_bytes: Bytes,
    /// Optional auxiliary proof material.
    /// PoolKes: serialised operational certificate bytes.
    /// AuthorityKey: multi-sig witness (future).
    /// Ed25519 / DRepCredential: `None`.
    pub proof_bytes: Option<Bytes>,
}

impl PublisherCredential {
    /// Convenience constructor for a plain Ed25519 credential.
    pub fn ed25519(key: Bytes) -> Self {
        Self {
            credential_type: CredentialType::Ed25519,
            key_bytes: key,
            proof_bytes: None,
        }
    }

    pub fn pool_kes(key: Bytes, opcert: Option<Bytes>) -> Self {
        Self {
            credential_type: CredentialType::PoolKes,
            key_bytes: key,
            proof_bytes: opcert,
        }
    }

    pub fn drep(key: Bytes) -> Self {
        Self {
            credential_type: CredentialType::DRepCredential,
            key_bytes: key,
            proof_bytes: None,
        }
    }

    pub fn authority(key: Bytes, witness: Option<Bytes>) -> Self {
        Self {
            credential_type: CredentialType::AuthorityKey,
            key_bytes: key,
            proof_bytes: witness,
        }
    }
}

/// Publisher identity — typed credential instead of raw bytes.
///
/// The inner `PublisherCredential` carries the credential type tag so the
/// validator knows which on-chain registry to check, and `key_bytes` holds
/// the actual Ed25519 verification key used to check the signature.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublisherId(pub PublisherCredential);

impl fmt::Display for PublisherId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.0.credential_type.as_str())?;
        let len = self.0.key_bytes.len().min(4);
        for b in &self.0.key_bytes[..len] {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
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

    /// Publisher's typed credential (key + credential type tag)
    pub publisher_id: PublisherId,

    /// Signature over signable_bytes() — always Ed25519 in Phase 1
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

/// Unique identifier for a message within the network
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId {
    pub topic_id: TopicId,
    pub publisher_id: PublisherId,
    pub sequence_nr: u64,
}

/// Server-side response to a PUBLISH bidirectional stream.
///
/// `Accepted` means the node validated the message and committed it locally
/// (signature verified, publisher authorised, stored in HotCache, broadcast to
/// local subscribers, dispatched to the dissemination layer).  `Rejected`
/// means at least one of those steps failed; `reason` is the human-readable
/// failure description so the publisher can correct and retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PublishAck {
    Accepted {
        topic_id: TopicId,
        sequence_nr: u64,
    },
    Rejected {
        reason: String,
    },
}

/// Control frame sent on a SUBSCRIBE bidirectional stream from client to node.
///
/// The node responds by streaming back encoded `Message` frames: first the
/// replay batch from `HotCache::get_since`, then live messages forwarded from
/// the receive-loop broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub topic_id: TopicId,
    /// Replay starts from sequence numbers strictly greater than this value.
    /// Phase 1: `0` returns the full TTL window held by the node's HotCache.
    pub since_seq: u64,
    /// Soft cap on the replay batch size.
    pub limit: u32,
}

impl Message {
    /// Compute the deterministic message ID (used for deduplication and DHT indexing)
    pub fn id(&self) -> MessageId {
        MessageId {
            topic_id: self.topic_id.clone(),
            publisher_id: self.publisher_id.clone(),
            sequence_nr: self.sequence_nr,
        }
    }

    /// Byte representation of the fields that are signed.
    ///
    /// The credential type discriminant is included before the key bytes so
    /// that a signature produced by one credential type cannot be replayed as
    /// a different type, even if the underlying key bytes happen to be the same.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let cred = &self.publisher_id.0;
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.topic_id.0);
        buf.extend_from_slice(&self.sequence_nr.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        // Credential type tag — bound into the signature to prevent type confusion.
        buf.push(cred.credential_type as u8);
        buf.extend_from_slice(&cred.key_bytes);
        if let Some(ref proof) = cred.proof_bytes {
            buf.extend_from_slice(proof);
        }
        buf.extend_from_slice(&self.payload);
        // Include metadata keys in deterministic order (BTreeMap guarantees this)
        for (k, v) in &self.metadata {
            buf.extend_from_slice(k.as_bytes());
            buf.extend_from_slice(v);
        }
        buf
    }
}
