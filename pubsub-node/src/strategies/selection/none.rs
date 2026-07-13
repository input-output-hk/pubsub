//! The empty selection policy: [`NoLinks`].

use std::collections::BTreeSet;

use super::LinkSelectionStrategy;
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// Select **no** outbound links (feature 015).
///
/// The publish-slot default — a node that publishes nothing needs no
/// initiation links, and the configuration is behaviour-preserving. On the
/// relay slot it yields an accept-only node (it dials nobody and serves
/// whoever picks it — the passive-amplifier shape the golden-node anchor
/// describes).
pub struct NoLinks;

impl LinkSelectionStrategy for NoLinks {
    fn expected_links(&self, _view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        BTreeSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::NoLinks;
    use crate::strategies::selection::LinkSelectionStrategy;
    use crate::strategies::test_support::{candidates, downstream, subscriptions, view};

    // 015 FR-012: the empty policy selects nothing, on any view.
    #[test]
    fn selects_no_links() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b"])]);
        let store = downstream(&[]);
        assert!(NoLinks
            .expected_links(&view(&subs, &cands, &store))
            .is_empty());
    }
}
