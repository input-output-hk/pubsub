//! Experiments-only strategy instances: [`SilentRelay`] (fan-out).
//!
//! It implements the protocol's fan-out seam but is available to experiment
//! configurations only — never a protocol CLI kind. The silent relay is the
//! models' dissemination-optimal (worst-case) adversary: it accepts and
//! records like an honest node but forwards to no one. (The experiments-only
//! uniform-sampler dial policy this module once carried was promoted to the
//! node's own `Selection` — the pick-count knob of the selection plane.)
// 016-FR-012 (silent relay); 017-FR-005 (sampler promoted, type deleted).

use std::collections::BTreeMap;

use crate::connection_state::{LinkKey, LinkState};
use crate::peer::PeerId;
use crate::received::Origin;
use crate::strategies::fanout::FanoutStrategy;
use crate::topic::TopicId;

/// A fan-out policy that selects no targets: the silent relay.
///
/// A participant running it behaves exactly like an honest node on every
/// other seam — it dials, accepts, records, and dedups — but never forwards
/// a dissemination message. This is the worst-case (dissemination-optimal)
/// adversary of the analytical models.
#[derive(Clone, Copy, Debug, Default)]
pub struct SilentRelay;

impl FanoutStrategy for SilentRelay {
    fn targets(
        &self,
        _topic: &TopicId,
        _downstream: &BTreeMap<LinkKey, LinkState>,
        _origin: &Origin,
        _exclude: Option<&PeerId>,
    ) -> Vec<PeerId> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::str::FromStr;

    use super::SilentRelay;
    use crate::connection_state::{LinkKey, LinkKind, LinkState};
    use crate::peer::PeerId;
    use crate::received::Origin;
    use crate::strategies::fanout::FanoutStrategy;
    use crate::topic::TopicId;

    fn peer(s: &str) -> PeerId {
        PeerId::from_str(s).expect("valid peer id")
    }

    fn topic(s: &str) -> TopicId {
        TopicId::from_str(s).expect("valid topic id")
    }

    // 016-FR-012: the silent relay selects no targets, whatever downstream
    // holds and whatever the delivery origin.
    #[test]
    fn silent_relay_selects_no_targets() {
        let downstream: BTreeMap<LinkKey, LinkState> = [
            (
                LinkKey::new(topic("t0"), peer("a"), LinkKind::Relay),
                LinkState::Active,
            ),
            (
                LinkKey::new(topic("t0"), peer("b"), LinkKind::Relay),
                LinkState::Active,
            ),
        ]
        .into_iter()
        .collect();
        assert!(SilentRelay
            .targets(&topic("t0"), &downstream, &Origin::Local, None)
            .is_empty());
        assert!(SilentRelay
            .targets(
                &topic("t0"),
                &downstream,
                &Origin::Peer(peer("a")),
                Some(&peer("a"))
            )
            .is_empty());
    }
}
