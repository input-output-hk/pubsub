//! Crypto trait pair + concrete byte-newtype types per ADR 0009.
//!
//! This module defines the *shape* of the signing / verification surface the
//! node depends on: the [`Signer`] / [`Verifier`] trait pair and the concrete
//! byte-newtype types they operate over ([`PublicKey`], [`PrivateKey`],
//! [`Signature`], [`MessageHash`], [`Timestamp`]). The traits take no type
//! parameters and define no associated types — implementations are stored
//! behind `Arc<dyn Verifier>` on a node.
//!
//! Concrete implementations live in [`mock`]. The mock pair mirrors the shape
//! of real asymmetric crypto without imposing its security properties; real
//! authenticity arrives in a later feature.

use std::fmt;

pub mod mock;

/// Write `bytes` to `f` as a contiguous lowercase hex string.
fn write_lower_hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(f, "{byte:02x}")?;
    }
    Ok(())
}

/// An opaque public key.
///
/// The bytes are uninterpreted at this type; concrete [`Signer`] / [`Verifier`]
/// implementations choose how to read them. `Display` renders the bytes as a
/// contiguous lowercase hex string.
///
/// `Ord`/`PartialOrd` order by the raw bytes lexicographically — a stable,
/// arbitrary total order used only to hold keys in ordered collections
/// (e.g. a topic's authorized-publisher `BTreeSet`); it carries no protocol
/// meaning.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    /// Construct a public key from raw bytes. No length constraint is imposed.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<Vec<u8>> for PublicKey {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lower_hex(f, &self.0)
    }
}

/// An opaque private key, shaped for secret discipline.
///
/// Deliberately carries **no** derived `Debug` (the hand-written impl redacts
/// the bytes), **no** `Hash`, and **no** `Display`. There is no operator-facing
/// reason to print a private key; code that attempts `format!("{}", key)` fails
/// to compile.
#[derive(Clone, Eq, PartialEq)]
pub struct PrivateKey(Vec<u8>);

impl PrivateKey {
    /// Construct a private key from raw bytes. No length constraint is imposed.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrivateKey([REDACTED])")
    }
}

/// An opaque signature.
///
/// `Display` renders the bytes as a contiguous lowercase hex string. There is
/// no placeholder constructor: a signature is always produced by signing real
/// bytes. Tests that need a deliberately-wrong value construct one directly via
/// [`Signature::new`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature(Vec<u8>);

impl Signature {
    /// Construct a signature from raw bytes. No length constraint is imposed.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lower_hex(f, &self.0)
    }
}

/// A fixed-width 32-byte content hash.
///
/// `Display` renders the 32 bytes as 64 lowercase hex characters.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MessageHash([u8; 32]);

impl MessageHash {
    /// The all-zero hash, used as the sentinel for an absent parent hash in the
    /// canonical signing-byte encoding.
    pub const ZERO: MessageHash = MessageHash([0u8; 32]);

    /// Construct a hash from a 32-byte array.
    #[must_use]
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Compute the content hash of `plain`: SHA-256 over its canonical signing
    /// bytes.
    ///
    /// Content-anchored — the signature is deliberately excluded from the hash
    /// input, so the hash is stable across signature changes and signing-scheme
    /// migrations. This is the value a subsequent message uses as its
    /// `parent_hash`.
    #[must_use]
    pub fn of(plain: &crate::message::PlainMessage) -> MessageHash {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(plain.signed_bytes());
        MessageHash(digest.into())
    }

    /// Borrow the underlying 32-byte array.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for MessageHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lower_hex(f, &self.0)
    }
}

/// A Unix-epoch millisecond timestamp.
///
/// Advisory only: it is included in the canonical signing bytes so a signature
/// commits to a publication time, but the receive path does not interpret it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Read the system clock, in Unix-epoch milliseconds.
    ///
    /// Returns `Timestamp::from_millis(0)` in the impossible case that the
    /// system clock reports a time before the Unix epoch.
    #[must_use]
    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        // Saturate rather than truncate: u64 milliseconds covers ~584M years
        // past the epoch, so this branch is unreachable in practice.
        Self(u64::try_from(millis).unwrap_or(u64::MAX))
    }

    /// Construct a timestamp from an explicit millisecond value.
    #[must_use]
    pub fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    /// Return the millisecond value.
    #[must_use]
    pub fn as_millis(&self) -> u64 {
        self.0
    }
}

/// Reasons a [`Verifier::verify`] call can reject a signature.
///
/// Marked `#[non_exhaustive]`: future verifier implementations may distinguish
/// further failure modes without breaking downstream callers. Pattern-matches
/// outside this crate must include a catch-all arm.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    /// The signature does not match the key and message.
    #[error("invalid signature")]
    Invalid,
}

/// Produces signatures over arbitrary byte slices.
///
/// Implementations are thread-shareable (`Send + Sync`) and define no
/// associated types per ADR 0009.
pub trait Signer: Send + Sync {
    /// Return the public key matching this signer's secret.
    fn public_key(&self) -> PublicKey;

    /// Sign `msg`, returning the signature. Infallible.
    fn sign(&self, msg: &[u8]) -> Signature;
}

/// Verifies signatures against a public key and message.
///
/// Implementations are thread-shareable (`Send + Sync`) so a node can hold one
/// behind `Arc<dyn Verifier>`. Verification is synchronous — the receive path
/// calls it without an intervening `await`.
pub trait Verifier: Send + Sync {
    /// Return `Ok(())` if `sig` is a valid signature by `key` over `msg`;
    /// otherwise return a [`VerifyError`].
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError>;
}

#[cfg(test)]
mod tests {
    use super::PrivateKey;

    #[test]
    fn private_key_debug_redacts_bytes() {
        let key = PrivateKey::new(vec![1, 2, 3]);
        assert_eq!(format!("{key:?}"), "PrivateKey([REDACTED])");
    }
}
