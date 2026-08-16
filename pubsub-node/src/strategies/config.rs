//! Selection-plane strategy construction (ADR 0028's principle — construction
//! and parameter validation live with the strategy, not scattered across the
//! edge — over the 017 knob surface).
//!
//! [`NodeStrategies::new`] builds the whole set from per-seam plane knobs
//! ([`SelectionParams`] / [`AcceptanceParams`]) — already-parsed values, no
//! `clap` in the core — in one fallible call: the relay pair always, the
//! publisher pair when its params are supplied. The first
//! [`StrategyConfigError`] surfaces to the edge, which maps it **once**.
//! (Fan-out stays injected separately; it is not built through this seam.)

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::connection_state::LinkKind;
use crate::message::push_len_prefixed;
use crate::peer::PeerId;
use crate::strategies::acceptance::{ConnectionAcceptanceStrategy, UnifiedAcceptance};
use crate::strategies::connection::{ConnectionStrategy, Selection};

/// Expand an operator-supplied u64 sampling seed into the 32-byte
/// constructor seed [`SelectionParams::seed`] takes:
/// `SHA-256( lp("pubsub/selection-seed/v1") ‖ seed_le8 )` with `lp` the
/// crate's one length-prefix primitive.
///
/// A pure format expansion: self-identity and the epoch nonce are **not**
/// mixed here — they enter in the selection instance's per-topic draw
/// preimage, so the independence and re-randomisation properties live in
/// one place for every construction site (the experiments driver injects
/// its own per-participant 32-byte seeds and never calls this).
// 017-T025; research R8. The operator flag is a reproducibility knob, not
// secret material — the privacy posture is recorded with the seed
// derivation decision.
#[must_use]
pub fn selection_seed_bytes(seed: u64) -> [u8; 32] {
    let mut preimage = Vec::new();
    push_len_prefixed(&mut preimage, b"pubsub/selection-seed/v1");
    preimage.extend_from_slice(&seed.to_le_bytes());
    Sha256::digest(&preimage).into()
}

/// The error strategy construction raises when the configuration lacks or
/// mis-values a parameter.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StrategyConfigError {
    /// The named strategy requires a parameter that was not supplied.
    #[error("the '{strategy}' strategy requires {parameter}")]
    MissingParameter {
        /// The strategy that needs the parameter (its config name).
        strategy: &'static str,
        /// The missing parameter, in operator-facing terms.
        parameter: &'static str,
    },
    /// The named strategy was supplied a parameter it cannot use.
    #[error("the '{strategy}' strategy requires {parameter} to be {constraint}")]
    InvalidParameter {
        /// The strategy that rejects the value (its config name).
        strategy: &'static str,
        /// The offending parameter, in operator-facing terms.
        parameter: &'static str,
        /// The constraint the value violated, in operator-facing terms.
        constraint: &'static str,
    },
    /// The relay pair's two sides disagree on the symmetric switch.
    #[error("the relay selection and acceptance parameters must agree on the symmetric switch: one switch drives the dial gate, the verification predicate, and the handshake vocabulary together")]
    SymmetricMismatch,
    /// The relay pair's two sides disagree on the ordered-predicate switch.
    #[error("the relay selection and acceptance parameters must agree on the ordered-predicate switch: one switch drives the dial draw and its verification together")]
    OrderedMismatch,
    /// The ordered comparison predicate was requested without the symmetric
    /// switch.
    #[error("the ordered comparison predicate requires the symmetric switch: it is a variant of the symmetric-handshake gate")]
    OrderedRequiresSymmetric,
    /// Publisher parameters carried the symmetric switch.
    #[error("publisher parameters cannot be symmetric: publisher links are directional")]
    SymmetricPublisher,
}

/// The concrete strategy set handed to [`Node::new`](crate::Node::new), produced
/// by [`NodeStrategies::new`] (the selection-plane construction) or by
/// [`NodeStrategies::relay_only`] for
/// direct construction. Four link seams: the relay pair (required) and the
/// publisher pair (optional — `None` disables publisher links: no dials on the
/// selection side, inbound publisher requests dropped on the acceptance side).
/// Fan-out stays injected separately — it is not built through this seam.
pub struct NodeStrategies {
    /// The relay-link selection (dial/upstream) strategy.
    pub relay_connection: Arc<dyn ConnectionStrategy>,
    /// The relay-link acceptance (downstream) strategy.
    pub relay_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    /// The publisher-link selection strategy (standing initiation dials).
    pub publisher_connection: Option<Arc<dyn ConnectionStrategy>>,
    /// The publisher-link acceptance strategy (inbound initiation links).
    pub publisher_acceptance: Option<Arc<dyn ConnectionAcceptanceStrategy>>,
    /// Whether relay links are established with the **symmetric**
    /// (bidirectional) handshake — M4 (ADR 0034): the dial pass speaks the
    /// symmetric vocabulary and one accept decision records each link in both
    /// directions on both ends. `false` (the default) on every directional
    /// model; inbound symmetric handshakes are then dropped outright.
    pub symmetric_edges: bool,
}

impl NodeStrategies {
    /// A relay-only strategy set from already-constructed instances — the M2
    /// baseline shape (publisher links disabled), and the concise form for
    /// tests that inject concrete strategies directly.
    #[must_use]
    pub fn relay_only(
        relay_connection: Arc<dyn ConnectionStrategy>,
        relay_acceptance: Arc<dyn ConnectionAcceptanceStrategy>,
    ) -> Self {
        Self {
            relay_connection,
            relay_acceptance,
            publisher_connection: None,
            publisher_acceptance: None,
            symmetric_edges: false,
        }
    }

    /// Switch the set to the symmetric (bidirectional) relay handshake — M4.
    /// Pair it with relay strategies drawing the symmetric predicate.
    #[must_use]
    pub fn with_symmetric_edges(mut self, symmetric: bool) -> Self {
        self.symmetric_edges = symmetric;
        self
    }
}

/// Already-parsed parameters for one seam's [`Selection`] instance: the
/// selection-plane knobs plus the sampling seed.
// 017-FR-001/FR-002 knob domains; research R1/R5.
#[derive(Clone, Debug)]
pub struct SelectionParams {
    /// The node's own identity (the requester side of the edge predicate).
    pub self_id: PeerId,
    /// The link kind the built instance dials — selects the hash domain.
    pub kind: LinkKind,
    /// Use the symmetric edge predicate on the gate. One flag sets this on
    /// the relay selection AND acceptance params together; publisher params
    /// leave it `false` (no published model defines symmetric publisher
    /// links — ADR 0034's boundary).
    pub symmetric: bool,
    /// Use the **ordered** comparison predicate on a symmetric gate
    /// (ADR 0043) — the experiments-only measurement arm; requires
    /// `symmetric` and is set on both relay params together.
    pub symmetric_ordered: bool,
    /// The bucket count (hash-gate width). `None` = ungated. `Some(0)` is
    /// rejected at construction; `Some(1)` is legal here (≡ ungated — the
    /// sweep config's axis point) even where the operator CLI rejects it.
    pub bucket_count: Option<usize>,
    /// The pick count: `None` = dial every gate survivor; `Some(0)` = dial
    /// none; `Some(k)` = exactly `min(k, survivors)` seeded uniform picks.
    pub pick_count: Option<usize>,
    /// The 32-byte sampling seed; read only when the pick count is `≥ 1`.
    pub seed: [u8; 32],
}

/// Already-parsed parameters for one seam's [`UnifiedAcceptance`] instance.
// 017-FR-010/FR-011/FR-012; research R1/R5.
#[derive(Clone, Debug)]
pub struct AcceptanceParams {
    /// The node's own identity (the candidate side of the verified edge).
    pub self_id: PeerId,
    /// The link kind the built instance admits — selects the hash domain and
    /// which accepted-link class its capacity counts.
    pub kind: LinkKind,
    /// Verify with the symmetric edge predicate — set from the same flag as
    /// the dial side (both relay seams switch together).
    pub symmetric: bool,
    /// Verify with the **ordered** comparison predicate (ADR 0043) — set
    /// from the same flag as the dial side; requires `symmetric`.
    pub symmetric_ordered: bool,
    /// The bucket count the acceptor verifies at — the **post-opt-out gate
    /// value**: the edge passes the seam's bucket count so acceptors verify
    /// exactly the `B` the dialers use, or `None` when verification is
    /// opted out (or the seam is ungated). Domain as on
    /// [`SelectionParams::bucket_count`].
    pub bucket_count: Option<usize>,
    /// The absolute per-topic serving cap: `None` = unbounded; `Some(0)` =
    /// serve none (every new link refused with the explicit rejection).
    pub accept_cap: Option<usize>,
}

/// Validate a plane bucket count for construction: `Some(0)` is rejected (a
/// zero-bucket gate is meaningless); `Some(1)` is legal (≡ ungated — the
/// sweep config's boundary axis point; the operator CLI applies its own
/// stricter rule at the edge).
fn validate_plane_bucket_count(
    strategy: &'static str,
    kind: LinkKind,
    bucket_count: Option<usize>,
) -> Result<Option<usize>, StrategyConfigError> {
    if bucket_count == Some(0) {
        return Err(StrategyConfigError::InvalidParameter {
            strategy,
            parameter: match kind {
                LinkKind::Relay => "the relay bucket count",
                LinkKind::Publisher => "the publisher bucket count",
            },
            constraint: "at least 1",
        });
    }
    Ok(bucket_count)
}

/// Build one seam's [`Selection`] instance from its plane params.
fn build_selection(
    params: SelectionParams,
) -> Result<Arc<dyn ConnectionStrategy>, StrategyConfigError> {
    let bucket_count = validate_plane_bucket_count("selection", params.kind, params.bucket_count)?;
    Ok(Arc::new(
        Selection::new(params.self_id, params.seed)
            .for_kind(params.kind)
            .with_symmetric(params.symmetric)
            .with_symmetric_ordered(params.symmetric_ordered)
            .with_bucket_count(bucket_count)
            .with_pick_count(params.pick_count),
    ))
}

/// Build one seam's [`UnifiedAcceptance`] instance from its plane params.
fn build_unified_acceptance(
    params: AcceptanceParams,
) -> Result<Arc<dyn ConnectionAcceptanceStrategy>, StrategyConfigError> {
    let gate = validate_plane_bucket_count("acceptance", params.kind, params.bucket_count)?;
    Ok(Arc::new(
        UnifiedAcceptance::new(params.self_id)
            .for_kind(params.kind)
            .with_symmetric(params.symmetric)
            .with_symmetric_ordered(params.symmetric_ordered)
            .with_gate(gate)
            .with_accept_cap(params.accept_cap),
    ))
}

impl NodeStrategies {
    /// Construct the full strategy set from selection-plane knobs — one
    /// fallible call building the relay pair always and the publisher pair
    /// when its params are supplied (`None` keeps the publisher seam off by
    /// construction: no dial pass, inbound publisher requests dropped). The
    /// first [`StrategyConfigError`] surfaces here, so the edge maps it
    /// once — the publisher pair no longer bypasses construction.
    // 017-FR-008 (seam off by construction), 017-FR-016 (seed threading);
    // research R5 (absorbs §1.2 item 6).
    pub fn new(
        relay_selection: SelectionParams,
        relay_acceptance: AcceptanceParams,
        publisher: Option<(SelectionParams, AcceptanceParams)>,
    ) -> Result<Self, StrategyConfigError> {
        // One flag configures the predicate on both relay params AND the
        // handshake vocabulary (a symmetric dial pass is what makes the
        // symmetric draws materialise as constructed pairs) — so a pair
        // disagreeing on the switch would dial symmetric while verifying
        // directional (or vice versa), silently. Rejected here, where
        // construction validates (ADR 0028's principle).
        if relay_selection.symmetric != relay_acceptance.symmetric {
            return Err(StrategyConfigError::SymmetricMismatch);
        }
        // The ordered comparison predicate (ADR 0043) rides the same
        // one-source doctrine, and is a symmetric-gate variant only.
        if relay_selection.symmetric_ordered != relay_acceptance.symmetric_ordered {
            return Err(StrategyConfigError::OrderedMismatch);
        }
        if relay_selection.symmetric_ordered && !relay_selection.symmetric {
            return Err(StrategyConfigError::OrderedRequiresSymmetric);
        }
        let symmetric_edges = relay_selection.symmetric;
        let relay_connection = build_selection(relay_selection)?;
        let relay_acceptance = build_unified_acceptance(relay_acceptance)?;
        let (publisher_connection, publisher_acceptance) = match publisher {
            None => (None, None),
            Some((selection, acceptance)) => {
                // Publisher instances are never symmetric (ADR 0034's
                // boundary — no flag exists; params default false).
                if selection.symmetric || acceptance.symmetric {
                    return Err(StrategyConfigError::SymmetricPublisher);
                }
                (
                    Some(build_selection(selection)?),
                    Some(build_unified_acceptance(acceptance)?),
                )
            }
        };
        Ok(Self {
            relay_connection,
            relay_acceptance,
            publisher_connection,
            publisher_acceptance,
            symmetric_edges,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        selection_seed_bytes, AcceptanceParams, NodeStrategies, SelectionParams,
        StrategyConfigError,
    };
    use crate::connection_state::LinkKind;
    use crate::strategies::test_support::peer;

    // 017-T025 (research R8): the operator-seed expansion is the documented
    // domain-separated derivation — deterministic, seed-sensitive, and
    // byte-for-byte the length-prefixed construction.
    #[test]
    fn selection_seed_expansion_is_pinned() {
        use sha2::{Digest, Sha256};

        assert_eq!(selection_seed_bytes(7), selection_seed_bytes(7));
        assert_ne!(selection_seed_bytes(7), selection_seed_bytes(8));

        let mut preimage = Vec::new();
        crate::message::push_len_prefixed(&mut preimage, b"pubsub/selection-seed/v1");
        preimage.extend_from_slice(&7u64.to_le_bytes());
        let expected: [u8; 32] = Sha256::digest(&preimage).into();
        assert_eq!(selection_seed_bytes(7), expected);
    }

    fn selection_params(kind: LinkKind) -> SelectionParams {
        SelectionParams {
            self_id: peer("self"),
            kind,
            symmetric: false,
            symmetric_ordered: false,
            bucket_count: None,
            pick_count: None,
            seed: [0u8; 32],
        }
    }

    fn acceptance_params(kind: LinkKind) -> AcceptanceParams {
        AcceptanceParams {
            self_id: peer("self"),
            kind,
            symmetric: false,
            symmetric_ordered: false,
            bucket_count: None,
            accept_cap: None,
        }
    }

    // 017-FR-008: no publisher params ⇒ the publisher seam stays off by
    // construction; the relay pair always builds.
    #[test]
    fn relay_only_construction_leaves_the_publisher_seam_off() {
        let strategies = NodeStrategies::new(
            selection_params(LinkKind::Relay),
            acceptance_params(LinkKind::Relay),
            None,
        )
        .expect("plane origin builds");
        assert!(strategies.publisher_connection.is_none());
        assert!(strategies.publisher_acceptance.is_none());
        assert!(!strategies.symmetric_edges);
    }

    // Publisher params supplied ⇒ both publisher seams are built through the
    // same call (no bypass — one error-map site).
    #[test]
    fn publisher_params_build_the_publisher_pair() {
        let strategies = NodeStrategies::new(
            selection_params(LinkKind::Relay),
            acceptance_params(LinkKind::Relay),
            Some((
                selection_params(LinkKind::Publisher),
                acceptance_params(LinkKind::Publisher),
            )),
        )
        .expect("publisher pair builds");
        assert!(strategies.publisher_connection.is_some());
        assert!(strategies.publisher_acceptance.is_some());
    }

    // The relay selection's symmetric knob drives the handshake vocabulary.
    #[test]
    fn symmetric_knob_threads_into_symmetric_edges() {
        let mut relay_selection = selection_params(LinkKind::Relay);
        relay_selection.symmetric = true;
        let mut relay_acceptance = acceptance_params(LinkKind::Relay);
        relay_acceptance.symmetric = true;
        let strategies = NodeStrategies::new(relay_selection, relay_acceptance, None)
            .expect("symmetric plane point builds");
        assert!(strategies.symmetric_edges);
    }

    // The relay pair must agree on the symmetric switch — a mismatched pair
    // would dial symmetric while verifying directional (or vice versa),
    // silently; construction rejects it in both directions.
    #[test]
    fn mismatched_relay_symmetric_is_rejected() {
        let mut symmetric_dial = selection_params(LinkKind::Relay);
        symmetric_dial.symmetric = true;
        assert_eq!(
            NodeStrategies::new(symmetric_dial, acceptance_params(LinkKind::Relay), None).err(),
            Some(StrategyConfigError::SymmetricMismatch),
        );

        let mut symmetric_accept = acceptance_params(LinkKind::Relay);
        symmetric_accept.symmetric = true;
        assert_eq!(
            NodeStrategies::new(selection_params(LinkKind::Relay), symmetric_accept, None).err(),
            Some(StrategyConfigError::SymmetricMismatch),
        );
    }

    // ADR 0043: the ordered switch follows the same one-source doctrine —
    // a pair disagreeing on it is rejected, and ordered without symmetric
    // is rejected (the ordered predicate is a symmetric-gate variant).
    #[test]
    fn ordered_switch_is_validated_at_construction() {
        let mut ordered_dial = selection_params(LinkKind::Relay);
        ordered_dial.symmetric = true;
        ordered_dial.symmetric_ordered = true;
        let mut symmetric_accept = acceptance_params(LinkKind::Relay);
        symmetric_accept.symmetric = true;
        assert_eq!(
            NodeStrategies::new(ordered_dial, symmetric_accept, None).err(),
            Some(StrategyConfigError::OrderedMismatch),
        );

        let mut ordered_without_symmetric = selection_params(LinkKind::Relay);
        ordered_without_symmetric.symmetric_ordered = true;
        let mut ordered_accept = acceptance_params(LinkKind::Relay);
        ordered_accept.symmetric_ordered = true;
        assert_eq!(
            NodeStrategies::new(ordered_without_symmetric, ordered_accept, None).err(),
            Some(StrategyConfigError::OrderedRequiresSymmetric),
        );
    }

    // Publisher instances are never symmetric (params default false; no flag
    // exists) — construction enforces the recorded invariant on either side.
    #[test]
    fn symmetric_publisher_params_are_rejected() {
        let mut symmetric_dial = selection_params(LinkKind::Publisher);
        symmetric_dial.symmetric = true;
        assert_eq!(
            NodeStrategies::new(
                selection_params(LinkKind::Relay),
                acceptance_params(LinkKind::Relay),
                Some((symmetric_dial, acceptance_params(LinkKind::Publisher))),
            )
            .err(),
            Some(StrategyConfigError::SymmetricPublisher),
        );

        let mut symmetric_accept = acceptance_params(LinkKind::Publisher);
        symmetric_accept.symmetric = true;
        assert_eq!(
            NodeStrategies::new(
                selection_params(LinkKind::Relay),
                acceptance_params(LinkKind::Relay),
                Some((selection_params(LinkKind::Publisher), symmetric_accept)),
            )
            .err(),
            Some(StrategyConfigError::SymmetricPublisher),
        );
    }

    // Core-domain validation: a bucket count of 0 is rejected on either
    // side of either seam; 1 is legal (≡ ungated — the sweep config's
    // boundary axis point, rejected only by the operator CLI's own rule).
    #[test]
    fn zero_bucket_count_is_rejected_and_one_is_legal() {
        let mut gated_zero = selection_params(LinkKind::Relay);
        gated_zero.bucket_count = Some(0);
        assert!(matches!(
            NodeStrategies::new(gated_zero, acceptance_params(LinkKind::Relay), None),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));

        let mut accept_zero = acceptance_params(LinkKind::Publisher);
        accept_zero.bucket_count = Some(0);
        assert!(matches!(
            NodeStrategies::new(
                selection_params(LinkKind::Relay),
                acceptance_params(LinkKind::Relay),
                Some((selection_params(LinkKind::Publisher), accept_zero)),
            ),
            Err(StrategyConfigError::InvalidParameter { .. }),
        ));

        let mut gated_one = selection_params(LinkKind::Relay);
        gated_one.bucket_count = Some(1);
        assert!(
            NodeStrategies::new(gated_one, acceptance_params(LinkKind::Relay), None).is_ok(),
            "bucket count 1 is the ungated axis point — legal in core construction",
        );
    }
}
