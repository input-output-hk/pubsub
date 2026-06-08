use proptest::prelude::*;
use pubsub_node::{
    derive_public, MockCryptoScheme, PublicKey, Signature, Signer, TestSigner, TestVerifier,
    Verifier, VerifyError,
};

// US4 AS-1: the same seed yields byte-identical keypair sequences.
#[test]
fn same_seed_yields_byte_identical_keypair_sequences() {
    let mut scheme_1 = MockCryptoScheme::with_seed([0u8; 32]);
    let mut scheme_2 = MockCryptoScheme::with_seed([0u8; 32]);

    for i in 0..10 {
        let kp1 = scheme_1.generate_keypair();
        let kp2 = scheme_2.generate_keypair();
        assert_eq!(
            kp1.public.as_bytes(),
            kp2.public.as_bytes(),
            "public keys differ at index {i}",
        );
        assert_eq!(
            kp1.private.as_bytes(),
            kp2.private.as_bytes(),
            "private keys differ at index {i}",
        );
    }
}

// US4 AS-2: different seeds yield differing first keypairs.
#[test]
fn different_seeds_yield_differing_keypairs() {
    let kp_a = MockCryptoScheme::with_seed([0u8; 32]).generate_keypair();
    let kp_b = MockCryptoScheme::with_seed([1u8; 32]).generate_keypair();

    assert_ne!(
        (kp_a.public.as_bytes(), kp_a.private.as_bytes()),
        (kp_b.public.as_bytes(), kp_b.private.as_bytes()),
        "a seed change must propagate into the key bytes",
    );
}

// US4 AS-3: the public key is derivable from the private via derive_public.
#[test]
fn derive_public_invariant_holds_on_generated_keypairs() {
    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    for _ in 0..5 {
        let kp = scheme.generate_keypair();
        assert_eq!(derive_public(&kp.private), kp.public);
    }
}

// US4 AS-4: TestVerifier accepts a TestSigner signature under the matching key.
#[test]
fn test_verifier_accepts_test_signer_signatures() {
    let mut scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let kp = scheme.generate_keypair();
    let signer = TestSigner::new(kp.private);
    let sig = signer.sign(b"arbitrary message bytes");

    assert!(TestVerifier
        .verify(&kp.public, b"arbitrary message bytes", &sig)
        .is_ok());
}

// US4 AS-5: a public key whose bytes lack the `_public` suffix is rejected.
#[test]
fn test_verifier_rejects_keys_without_public_suffix() {
    let not_a_public = PublicKey::new(vec![0xAB, 0xCD, 0xEF]);
    let result = TestVerifier.verify(&not_a_public, b"anything", &Signature::new(vec![0u8; 32]));
    assert!(matches!(result, Err(VerifyError::Invalid)));
}

proptest! {
    // Signature binding: a matching (signer, public) pair verifies for any
    // message; changing the key, message, or signature causes rejection.
    #[test]
    fn signature_binding_proptest(seed in any::<[u8; 32]>(), msg in any::<Vec<u8>>()) {
        let mut scheme = MockCryptoScheme::with_seed(seed);
        let kp = scheme.generate_keypair();
        // A second, distinct keypair from the same advanced RNG.
        let other = scheme.generate_keypair();

        let signer = TestSigner::new(kp.private.clone());
        let sig = signer.sign(&msg);
        let verifier = TestVerifier;

        // The matching pair verifies.
        prop_assert!(verifier.verify(&kp.public, &msg, &sig).is_ok());

        // A different message rejects.
        let mut other_msg = msg.clone();
        other_msg.push(0xFF);
        prop_assert!(verifier.verify(&kp.public, &other_msg, &sig).is_err());

        // A modified signature rejects (flip one byte of the 32-byte digest).
        let mut sig_bytes = sig.as_bytes().to_vec();
        sig_bytes[0] ^= 0xFF;
        prop_assert!(verifier
            .verify(&kp.public, &msg, &Signature::new(sig_bytes))
            .is_err());

        // A different public key rejects.
        prop_assert!(verifier.verify(&other.public, &msg, &sig).is_err());
    }
}
