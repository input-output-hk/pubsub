//! The empty selection policy: [`DialNone`].

use std::collections::BTreeSet;

use super::ConnectionStrategy;
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The empty selection policy: dial **no** links, ever.
///
/// The relay seam's off-switch for push-only configurations — M1, or an M5
/// sweep at its `k_in = 0` boundary — where a node holds publisher links only.
/// (The publisher seam needs no such policy: it is optional and absent by
/// default.)
pub struct DialNone;

impl ConnectionStrategy for DialNone {
    fn expected_links(&self, _view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        BTreeSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DialNone;
    use crate::strategies::connection::ConnectionStrategy;
    use crate::strategies::test_support::{candidates, no_links, subscriptions, view};

    // The off-switch: candidates present, nothing expected.
    #[test]
    fn expects_nothing_regardless_of_candidates() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b", "c"])]);
        assert!(DialNone
            .expected_links(&view(&subs, &cands, no_links()))
            .is_empty());
    }
}
