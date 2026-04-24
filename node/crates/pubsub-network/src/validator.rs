use std::sync::Arc;

use async_trait::async_trait;
use pallas_crypto::key::ed25519::{PublicKey, Signature};
use tracing::{debug, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::message::{CredentialType, Message};
use pubsub_types::traits::{ChainState, MessageValidator};

/// Validates messages by verifying their Ed25519 signature and checking the
/// publisher's credential against the appropriate on-chain registry.
///
/// All four credential types use Ed25519 as the signing primitive.  What
/// differs is which registry is queried to confirm the key is legitimate:
///
/// | CredentialType  | Authorization registry              |
/// |-----------------|-------------------------------------|
/// | Ed25519         | Topic Registry `authorized_publishers` |
/// | PoolKes         | On-chain Pool KES key list          |
/// | DRepCredential  | CIP-1694 DRep registration          |
/// | AuthorityKey    | Curated authority key list          |
pub struct SignatureValidator {
    chain_state: Arc<dyn ChainState>,
}

impl SignatureValidator {
    pub fn new(chain_state: Arc<dyn ChainState>) -> Self {
        Self { chain_state }
    }
}

#[async_trait]
impl MessageValidator for SignatureValidator {
    async fn validate(&self, msg: &Message) -> Result<(), PubSubError> {
        let cred = &msg.publisher_id.0;

        // ── Step 1: verify Ed25519 signature ────────────────────────────────
        let public_key = PublicKey::try_from(cred.key_bytes.as_ref()).map_err(|e| {
            warn!(error = %e, "Invalid Ed25519 public key in publisher credential");
            PubSubError::InvalidSignature
        })?;

        let signature = Signature::try_from(msg.signature.as_ref()).map_err(|_| {
            PubSubError::InvalidSignature
        })?;

        if !public_key.verify(&msg.signable_bytes(), &signature) {
            warn!(
                cred_type = cred.credential_type.as_str(),
                topic = %msg.topic_id,
                seq = msg.sequence_nr,
                "Signature verification failed"
            );
            return Err(PubSubError::InvalidSignature);
        }

        debug!(
            cred_type = cred.credential_type.as_str(),
            topic = %msg.topic_id,
            seq = msg.sequence_nr,
            "Signature verified"
        );

        // ── Step 2: credential-type-specific authorization check ────────────
        match cred.credential_type {
            CredentialType::Ed25519 => {
                // Check against the Topic Registry's authorized_publishers.
                // An empty list means the topic is open (anyone can publish).
                let topic_config = self
                    .chain_state
                    .get_topic_config(&msg.topic_id)
                    .await?
                    .ok_or_else(|| {
                        let hex: String =
                            msg.topic_id.0.iter().map(|b| format!("{b:02x}")).collect();
                        PubSubError::TopicNotFound(hex)
                    })?;

                if !topic_config.is_authorized(&msg.publisher_id) {
                    warn!(
                        topic = %msg.topic_id,
                        publisher = %msg.publisher_id,
                        "Ed25519 publisher not in topic's authorized list"
                    );
                    return Err(PubSubError::Unauthorized);
                }
            }

            CredentialType::PoolKes => {
                // A registered KES key may publish SPO announcements to any topic.
                // Phase 1: verify the key is in the mock pool KES registry.
                let kes_keys = self.chain_state.get_pool_kes_keys().await?;
                if !kes_keys.contains(&cred.key_bytes) {
                    warn!(
                        publisher = %msg.publisher_id,
                        "PoolKes key not registered in pool KES registry"
                    );
                    return Err(PubSubError::Unauthorized);
                }
            }

            CredentialType::DRepCredential => {
                // A registered DRep key may publish governance updates to any topic.
                // Phase 1: verify the key is in the mock DRep registry.
                let drep_keys = self.chain_state.get_drep_keys().await?;
                if !drep_keys.contains(&cred.key_bytes) {
                    warn!(
                        publisher = %msg.publisher_id,
                        "DRep credential not registered in DRep registry"
                    );
                    return Err(PubSubError::Unauthorized);
                }
            }

            CredentialType::AuthorityKey => {
                // An authority key may publish emergency alerts to any topic.
                // Phase 1: verify the key is in the curated authority list.
                let auth_keys = self.chain_state.get_authority_keys().await?;
                if !auth_keys.contains(&cred.key_bytes) {
                    warn!(
                        publisher = %msg.publisher_id,
                        "AuthorityKey not in authority key list"
                    );
                    return Err(PubSubError::Unauthorized);
                }
            }
        }

        debug!(
            cred_type = cred.credential_type.as_str(),
            topic = %msg.topic_id,
            seq = msg.sequence_nr,
            "Message validation passed"
        );
        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;
    use pallas_crypto::key::ed25519::SecretKey;

    use pubsub_types::message::{
        Message, PublisherCredential, PublisherId, TopicId,
    };
    use pubsub_types::topic::TopicConfig;
    use pubsub_types::traits::MessageValidator;

    use crate::mock_chain::MockChainState;
    use super::SignatureValidator;

    fn topic_id() -> TopicId {
        TopicId([0x01u8; 32])
    }

    /// Build a signed message using the given signing key and credential.
    fn signed_message(
        signing_key: &SecretKey,
        credential: PublisherCredential,
    ) -> Message {
        let mut msg = Message {
            topic_id: topic_id(),
            sequence_nr: 1,
            timestamp_ms: 0,
            publisher_id: PublisherId(credential),
            signature: Bytes::new(),
            payload: Bytes::from_static(b"test payload"),
            metadata: BTreeMap::new(),
        };
        let sig = signing_key.sign(&msg.signable_bytes());
        msg.signature = Bytes::copy_from_slice(sig.as_ref());
        msg
    }

    fn open_topic() -> TopicConfig {
        TopicConfig {
            topic_id: topic_id(),
            name: "test".into(),
            description: None,
            authorized_publishers: vec![],
            retention_period: Duration::from_secs(60),
            replication_factor: 1,
        }
    }

    // ── Ed25519 ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn ed25519_valid_open_topic() {
        let signing_key = SecretKey::from([0x42u8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::ed25519(Bytes::copy_from_slice(vk.as_ref()));
        let msg = signed_message(&signing_key, cred);

        let chain = Arc::new(MockChainState::new(vec![], vec![open_topic()]));
        let validator = SignatureValidator::new(chain);

        assert!(validator.validate(&msg).await.is_ok());
    }

    #[tokio::test]
    async fn ed25519_restricted_topic_allows_authorized() {
        let signing_key = SecretKey::from([0x11u8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::ed25519(Bytes::copy_from_slice(vk.as_ref()));
        let publisher_id = PublisherId(cred.clone());
        let msg = signed_message(&signing_key, cred);

        let config = TopicConfig {
            topic_id: topic_id(),
            name: "restricted".into(),
            description: None,
            authorized_publishers: vec![publisher_id],
            retention_period: Duration::from_secs(60),
            replication_factor: 1,
        };
        let chain = Arc::new(MockChainState::new(vec![], vec![config]));
        let validator = SignatureValidator::new(chain);

        assert!(validator.validate(&msg).await.is_ok());
    }

    #[tokio::test]
    async fn ed25519_restricted_topic_rejects_unauthorized() {
        let signing_key = SecretKey::from([0x22u8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::ed25519(Bytes::copy_from_slice(vk.as_ref()));
        let msg = signed_message(&signing_key, cred);

        // Authorized list has a DIFFERENT key
        let other_key = SecretKey::from([0x33u8; 32]);
        let other_vk = other_key.public_key();
        let other_pid = PublisherId(PublisherCredential::ed25519(
            Bytes::copy_from_slice(other_vk.as_ref()),
        ));
        let config = TopicConfig {
            topic_id: topic_id(),
            name: "restricted".into(),
            description: None,
            authorized_publishers: vec![other_pid],
            retention_period: Duration::from_secs(60),
            replication_factor: 1,
        };
        let chain = Arc::new(MockChainState::new(vec![], vec![config]));
        let validator = SignatureValidator::new(chain);

        assert!(matches!(
            validator.validate(&msg).await,
            Err(pubsub_types::error::PubSubError::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn ed25519_bad_signature_rejected() {
        let signing_key = SecretKey::from([0x42u8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::ed25519(Bytes::copy_from_slice(vk.as_ref()));
        let mut msg = signed_message(&signing_key, cred);
        // Corrupt the signature
        msg.signature = Bytes::from(vec![0u8; 64]);

        let chain = Arc::new(MockChainState::new(vec![], vec![open_topic()]));
        let validator = SignatureValidator::new(chain);

        assert!(matches!(
            validator.validate(&msg).await,
            Err(pubsub_types::error::PubSubError::InvalidSignature)
        ));
    }

    // ── PoolKes ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn pool_kes_registered_key_accepted() {
        let signing_key = SecretKey::from([0xCEu8; 32]);
        let vk = signing_key.public_key();
        let key_bytes = Bytes::copy_from_slice(vk.as_ref());
        let cred = PublisherCredential::pool_kes(key_bytes.clone(), None);
        let msg = signed_message(&signing_key, cred);

        let chain = Arc::new(
            MockChainState::empty().with_pool_kes_key(key_bytes),
        );
        let validator = SignatureValidator::new(chain);

        assert!(validator.validate(&msg).await.is_ok());
    }

    #[tokio::test]
    async fn pool_kes_unregistered_key_rejected() {
        let signing_key = SecretKey::from([0xCEu8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::pool_kes(
            Bytes::copy_from_slice(vk.as_ref()),
            None,
        );
        let msg = signed_message(&signing_key, cred);

        // Chain has a DIFFERENT key registered
        let other_key = SecretKey::from([0xFFu8; 32]);
        let other_vk = other_key.public_key();
        let chain = Arc::new(
            MockChainState::empty()
                .with_pool_kes_key(Bytes::copy_from_slice(other_vk.as_ref())),
        );
        let validator = SignatureValidator::new(chain);

        assert!(matches!(
            validator.validate(&msg).await,
            Err(pubsub_types::error::PubSubError::Unauthorized)
        ));
    }

    // ── DRepCredential ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn drep_registered_key_accepted() {
        let signing_key = SecretKey::from([0xDEu8; 32]);
        let vk = signing_key.public_key();
        let key_bytes = Bytes::copy_from_slice(vk.as_ref());
        let cred = PublisherCredential::drep(key_bytes.clone());
        let msg = signed_message(&signing_key, cred);

        let chain = Arc::new(MockChainState::empty().with_drep_key(key_bytes));
        let validator = SignatureValidator::new(chain);

        assert!(validator.validate(&msg).await.is_ok());
    }

    #[tokio::test]
    async fn drep_unregistered_key_rejected() {
        let signing_key = SecretKey::from([0xDEu8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::drep(Bytes::copy_from_slice(vk.as_ref()));
        let msg = signed_message(&signing_key, cred);

        let chain = Arc::new(MockChainState::empty()); // no DRep keys
        let validator = SignatureValidator::new(chain);

        assert!(matches!(
            validator.validate(&msg).await,
            Err(pubsub_types::error::PubSubError::Unauthorized)
        ));
    }

    // ── AuthorityKey ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn authority_key_accepted() {
        let signing_key = SecretKey::from([0xAEu8; 32]);
        let vk = signing_key.public_key();
        let key_bytes = Bytes::copy_from_slice(vk.as_ref());
        let cred = PublisherCredential::authority(key_bytes.clone(), None);
        let msg = signed_message(&signing_key, cred);

        let chain = Arc::new(MockChainState::empty().with_authority_key(key_bytes));
        let validator = SignatureValidator::new(chain);

        assert!(validator.validate(&msg).await.is_ok());
    }

    #[tokio::test]
    async fn authority_key_not_in_list_rejected() {
        let signing_key = SecretKey::from([0xAEu8; 32]);
        let vk = signing_key.public_key();
        let cred = PublisherCredential::authority(
            Bytes::copy_from_slice(vk.as_ref()),
            None,
        );
        let msg = signed_message(&signing_key, cred);

        let chain = Arc::new(MockChainState::empty()); // no authority keys
        let validator = SignatureValidator::new(chain);

        assert!(matches!(
            validator.validate(&msg).await,
            Err(pubsub_types::error::PubSubError::Unauthorized)
        ));
    }

    // ── Type confusion prevention ────────────────────────────────────────────
    // A signature produced with one credential type must NOT validate if the
    // same key bytes are presented as a different credential type.

    #[tokio::test]
    async fn credential_type_tag_prevents_type_confusion() {
        let signing_key = SecretKey::from([0x99u8; 32]);
        let vk = signing_key.public_key();
        let key_bytes = Bytes::copy_from_slice(vk.as_ref());

        // Sign as PoolKes
        let kes_cred = PublisherCredential::pool_kes(key_bytes.clone(), None);
        let kes_msg = signed_message(&signing_key, kes_cred);

        // Now present the same key_bytes but as Ed25519 and transplant the signature
        let mut ed_msg = kes_msg.clone();
        ed_msg.publisher_id =
            PublisherId(PublisherCredential::ed25519(key_bytes.clone()));
        // ed_msg.signature still has the PoolKes signature (over different signable_bytes)

        let chain = Arc::new(
            MockChainState::empty()
                .with_pool_kes_key(key_bytes.clone()),
        );
        let validator = SignatureValidator::new(chain);

        // Original PoolKes message validates fine
        assert!(validator.validate(&kes_msg).await.is_ok());
        // Type-confused Ed25519 message fails signature check
        assert!(matches!(
            validator.validate(&ed_msg).await,
            Err(pubsub_types::error::PubSubError::InvalidSignature)
        ));
    }
}
