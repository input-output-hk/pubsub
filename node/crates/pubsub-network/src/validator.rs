use std::sync::Arc;

use async_trait::async_trait;
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use tracing::{debug, warn};

use pubsub_types::error::PubSubError;
use pubsub_types::message::Message;
use pubsub_types::traits::{ChainState, MessageValidator};

/// Validates messages by checking ed25519 signatures and
/// publisher authorization against on-chain topic configuration.
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
        // 1. Verify ed25519 signature
        let pub_key_bytes: &[u8] = &msg.publisher_id.0;
        let verifying_key = VerifyingKey::from_bytes(
            pub_key_bytes
                .try_into()
                .map_err(|_| PubSubError::InvalidSignature)?,
        )
        .map_err(|e| {
            warn!("Invalid public key in publisher_id: {e}");
            PubSubError::InvalidSignature
        })?;

        let signature = Signature::from_bytes(
            msg.signature
                .as_ref()
                .try_into()
                .map_err(|_| PubSubError::InvalidSignature)?,
        );

        let signable = msg.signable_bytes();
        verifying_key
            .verify(&signable, &signature)
            .map_err(|e| {
                warn!("Signature verification failed: {e}");
                PubSubError::InvalidSignature
            })?;

        debug!(
            topic = %msg.topic_id,
            seq = msg.sequence_nr,
            "Signature verified"
        );

        // 2. Look up topic configuration from chain state
        let topic_config = self
            .chain_state
            .get_topic_config(&msg.topic_id)
            .await?
            .ok_or_else(|| {
                let hex: String = msg.topic_id.0.iter().map(|b| format!("{b:02x}")).collect();
                PubSubError::TopicNotFound(hex)
            })?;

        // 3. Check publisher is authorized for this topic
        if !topic_config.is_authorized(&msg.publisher_id) {
            warn!(
                topic = %msg.topic_id,
                publisher = ?msg.publisher_id,
                "Publisher not authorized for topic"
            );
            return Err(PubSubError::Unauthorized);
        }

        debug!(
            topic = %msg.topic_id,
            seq = msg.sequence_nr,
            "Message validation passed"
        );
        Ok(())
    }
}
