//! The refusing acceptance policy: [`AcceptNone`].

use super::{Admission, ConnectionAcceptanceStrategy};
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The refusing acceptance policy: accept **no** inbound requests, ever.
///
/// The relay seam's off-switch for push-only configurations (M1 / the M5
/// `k_in = 0` boundary): every request is silently refused as illegitimate —
/// no reply, nothing leaked to the requester, mirroring the hash-gated
/// silent-drop convention.
pub struct AcceptNone;

impl ConnectionAcceptanceStrategy for AcceptNone {
    fn admit(&self, _emitter: &PeerId, _topic: &TopicId, _view: &NodeView<'_>) -> Admission {
        Admission::RejectIllegitimate
    }
}

#[cfg(test)]
mod tests {
    use super::AcceptNone;
    use crate::strategies::acceptance::{Admission, ConnectionAcceptanceStrategy};
    use crate::strategies::test_support::{candidates, no_links, peer, subscriptions, topic, view};

    // Even a membership-valid request is silently refused.
    #[test]
    fn refuses_a_valid_member() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a"])]);
        assert_eq!(
            AcceptNone.admit(&peer("a"), &topic("t1"), &view(&subs, &cands, no_links())),
            Admission::RejectIllegitimate,
        );
    }
}
