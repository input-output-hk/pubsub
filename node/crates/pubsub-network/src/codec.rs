use tracing::trace;

use pubsub_types::error::PubSubError;
use pubsub_types::message::Message;
use pubsub_types::traits::Codec;

/// CBOR codec using the ciborium crate.
///
/// Provides simple, deterministic serialization of `Message` values
/// suitable for wire transport and storage.
#[derive(Default)]
pub struct CborCodec;

impl Codec for CborCodec {
    fn encode(&self, msg: &Message) -> Result<Vec<u8>, PubSubError> {
        let mut buf = Vec::new();
        ciborium::into_writer(msg, &mut buf)
            .map_err(|e| PubSubError::Codec(format!("CBOR encode failed: {e}")))?;
        trace!(bytes = buf.len(), "Encoded message to CBOR");
        Ok(buf)
    }

    fn decode(&self, data: &[u8]) -> Result<Message, PubSubError> {
        let msg: Message = ciborium::from_reader(data)
            .map_err(|e| PubSubError::Codec(format!("CBOR decode failed: {e}")))?;
        trace!(topic = %msg.topic_id, seq = msg.sequence_nr, "Decoded message from CBOR");
        Ok(msg)
    }
}
