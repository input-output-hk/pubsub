//! **MOCK — not unforgeable.**
//!
//! The mock implementations below are designed to mirror the *shape* of real
//! asymmetric crypto without imposing its security properties. Anyone with read
//! access to this module's source can produce a forged signature that
//! [`TestVerifier::verify`] accepts. The mock exists solely to differentiate
//! correct-vs-incorrect key+message bindings in tests. Real authenticity
//! arrives in feature 011.

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

use super::{PrivateKey, PublicKey, Signature, Signer, Verifier, VerifyError};

/// The fixed suffix appended to a private key's bytes to derive its public key.
///
/// Declared once and shared between [`derive_public`] (which appends it) and
/// [`TestVerifier::verify`] (which strips it), so the two operations stay
/// byte-symmetric by construction.
pub(crate) const PUBLIC_SUFFIX: &[u8] = b"_public";

/// Derive a [`PublicKey`] from a [`PrivateKey`] by appending `PUBLIC_SUFFIX`.
///
/// Deterministic and total. Exposed so tests can assert the
/// `derive_public(&kp.private) == kp.public` invariant directly.
#[must_use]
pub fn derive_public(private: &PrivateKey) -> PublicKey {
    let mut bytes = private.as_bytes().to_vec();
    bytes.extend_from_slice(PUBLIC_SUFFIX);
    PublicKey::new(bytes)
}

/// A public/private key pair produced by [`MockCryptoScheme::generate_keypair`].
///
/// Public fields by design — tests destructure freely. `Debug` delegates to
/// [`PrivateKey`]'s redacting impl, so the private bytes never appear in a
/// formatted `KeyPair`.
#[derive(Clone, Debug)]
pub struct KeyPair {
    /// The public key, equal to `derive_public(&self.private)`.
    pub public: PublicKey,
    /// The private key.
    pub private: PrivateKey,
}

/// **MOCK — not unforgeable.** Factory for mock keypairs, signers, and
/// verifiers.
///
/// Owns a seeded `ChaCha20` RNG so generated keypairs are reproducible from a
/// seed. See the module-level documentation for the security caveat.
pub struct MockCryptoScheme {
    rng: ChaCha20Rng,
}

impl MockCryptoScheme {
    /// Construct a scheme whose keypairs are deterministic in `seed`.
    ///
    /// Two schemes built from the same seed produce byte-identical keypair
    /// sequences.
    #[must_use]
    pub fn with_seed(seed: [u8; 32]) -> Self {
        Self {
            rng: ChaCha20Rng::from_seed(seed),
        }
    }

    /// Construct a scheme seeded from OS entropy, for non-deterministic keys.
    #[must_use]
    pub fn from_entropy() -> Self {
        Self {
            rng: ChaCha20Rng::from_entropy(),
        }
    }

    /// Draw a fresh keypair, advancing the internal RNG.
    ///
    /// The private key is 32 fresh random bytes; the public key is
    /// [`derive_public`] of it.
    #[must_use]
    pub fn generate_keypair(&mut self) -> KeyPair {
        let mut private_bytes = [0u8; 32];
        self.rng.fill_bytes(&mut private_bytes);
        let private = PrivateKey::new(private_bytes.to_vec());
        let public = derive_public(&private);
        KeyPair { public, private }
    }

    /// Construct the [`KeyPair`] for a string alias: the private key is the
    /// alias's bytes, the public key is [`derive_public`] of it. Deterministic;
    /// does not advance the RNG.
    ///
    /// The mock-stage identity convenience: an alias-derived keypair signs and
    /// verifies through the unmodified [`TestSigner`]/[`TestVerifier`], and its
    /// public key equals the key inside the same alias parsed as a
    /// [`PeerId`](crate::PeerId) by construction.
    #[allow(clippy::unused_self)]
    #[must_use]
    pub fn keypair_from_alias(&self, alias: &str) -> KeyPair {
        let private = PrivateKey::new(alias.as_bytes().to_vec());
        let public = derive_public(&private);
        KeyPair { public, private }
    }

    // `signer` and `verifier` take `&self` by design: the scheme is the factory
    // entry point for both, and neither call advances the RNG. The `&self`
    // receiver keeps the factory shape uniform and leaves room for future
    // schemes whose signer/verifier construction depends on scheme state.
    /// Construct a [`TestSigner`] wrapping `private`. Does not advance the RNG.
    #[allow(clippy::unused_self)]
    #[must_use]
    pub fn signer(&self, private: PrivateKey) -> TestSigner {
        TestSigner::new(private)
    }

    /// Construct a [`TestVerifier`]. Does not advance the RNG.
    #[allow(clippy::unused_self)]
    #[must_use]
    pub fn verifier(&self) -> TestVerifier {
        TestVerifier
    }
}

/// **MOCK — not unforgeable.** A signer wrapping a [`PrivateKey`].
///
/// `sign(msg)` returns `Signature(sha256(private || msg))`. See the
/// module-level documentation for the security caveat.
pub struct TestSigner {
    private: PrivateKey,
}

impl TestSigner {
    /// Construct a signer wrapping `private`.
    #[must_use]
    pub fn new(private: PrivateKey) -> Self {
        Self { private }
    }
}

impl Signer for TestSigner {
    fn public_key(&self) -> PublicKey {
        derive_public(&self.private)
    }

    fn sign(&self, msg: &[u8]) -> Signature {
        let mut input = self.private.as_bytes().to_vec();
        input.extend_from_slice(msg);
        Signature::new(Sha256::digest(&input).to_vec())
    }
}

/// **MOCK — not unforgeable.** A stateless verifier for [`TestSigner`]
/// signatures.
///
/// `verify` strips `PUBLIC_SUFFIX` from the public key to recover the private
/// bytes, recomputes `sha256(private || msg)`, and compares byte-for-byte. See
/// the module-level documentation for the security caveat.
pub struct TestVerifier;

impl Verifier for TestVerifier {
    fn verify(&self, key: &PublicKey, msg: &[u8], sig: &Signature) -> Result<(), VerifyError> {
        let stripped = key
            .as_bytes()
            .strip_suffix(PUBLIC_SUFFIX)
            .ok_or(VerifyError::Invalid)?;
        let mut input = stripped.to_vec();
        input.extend_from_slice(msg);
        let expected = Sha256::digest(&input);
        if sig.as_bytes() == expected.as_slice() {
            Ok(())
        } else {
            Err(VerifyError::Invalid)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{derive_public, MockCryptoScheme, TestVerifier};
    use crate::crypto::{Signer, Verifier};

    // R9: an alias keypair satisfies the same derive invariant as a generated
    // one — the public key is `derive_public` of the private (alias-bytes) key.
    #[test]
    fn alias_keypair_satisfies_derive_invariant() {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let kp = scheme.keypair_from_alias("node-a");
        assert_eq!(kp.public, derive_public(&kp.private));
        assert_eq!(kp.private.as_bytes(), b"node-a");
    }

    // R9: alias identities sign and verify through the unmodified mock pair.
    #[test]
    fn alias_keypair_signs_and_verifies_through_mock_pair() {
        let scheme = MockCryptoScheme::with_seed([0u8; 32]);
        let kp = scheme.keypair_from_alias("node-a");
        let signer = scheme.signer(kp.private);
        assert_eq!(signer.public_key(), kp.public);

        let msg = b"payload bytes";
        let sig = signer.sign(msg);
        assert!(TestVerifier.verify(&kp.public, msg, &sig).is_ok());
        assert!(TestVerifier
            .verify(&kp.public, b"other bytes", &sig)
            .is_err());
    }

    // R9: `keypair_from_alias` does not advance the RNG — a scheme that called
    // it still generates the same keypair sequence as a fresh same-seed scheme.
    #[test]
    fn keypair_from_alias_does_not_advance_the_rng() {
        let mut with_alias_call = MockCryptoScheme::with_seed([3u8; 32]);
        let _ = with_alias_call.keypair_from_alias("node-a");
        let mut fresh = MockCryptoScheme::with_seed([3u8; 32]);
        let a = with_alias_call.generate_keypair();
        let b = fresh.generate_keypair();
        assert_eq!(a.public, b.public);
        assert_eq!(a.private, b.private);
    }
}
