//! Length-prefixed framing on QUIC streams.
//!
//! Every frame is `[u32 BE length][payload]`. A 16 MiB cap rejects
//! malicious or runaway lengths before allocating.

use pubsub_types::error::PubSubError;

const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

pub(super) async fn read_framed(stream: &mut quinn::RecvStream) -> Result<Vec<u8>, PubSubError> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| PubSubError::Transport(format!("Failed to read frame length: {e}")))?;

    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(PubSubError::Transport(format!("Frame too large: {len} bytes")));
    }

    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(|e| PubSubError::Transport(format!("Failed to read frame payload: {e}")))?;

    Ok(payload)
}

/// Write a length-prefixed frame and finish the send stream.
pub(super) async fn write_framed(
    stream: &mut quinn::SendStream,
    data: &[u8],
) -> Result<(), PubSubError> {
    write_framed_no_finish(stream, data).await?;
    stream
        .finish()
        .map_err(|e| PubSubError::Transport(format!("Failed to finish stream: {e}")))?;
    Ok(())
}

/// Write a length-prefixed frame without finishing the send stream — used
/// when many frames are written on a single long-lived stream (subscribe).
pub(super) async fn write_framed_no_finish(
    stream: &mut quinn::SendStream,
    data: &[u8],
) -> Result<(), PubSubError> {
    let len = (data.len() as u32).to_be_bytes();
    stream
        .write_all(&len)
        .await
        .map_err(|e| PubSubError::Transport(format!("Failed to write frame length: {e}")))?;
    stream
        .write_all(data)
        .await
        .map_err(|e| PubSubError::Transport(format!("Failed to write frame payload: {e}")))?;
    Ok(())
}
