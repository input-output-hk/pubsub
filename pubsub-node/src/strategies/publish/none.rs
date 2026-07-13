//! The default publish policy: [`NoPublishLinks`].

use std::collections::BTreeSet;

use super::PublishStrategy;
use crate::peer::PeerId;
use crate::strategies::view::NodeView;
use crate::topic::TopicId;

/// The default publish-target policy: **no publishing links** (feature 015).
///
/// The behaviour-preserving configuration — a node running this policy forms
/// exactly the topology it formed before publishing links existed. Nodes that
/// need an injection route opt in via
/// [`HashGatedPublish`](super::HashGatedPublish).
pub struct NoPublishLinks;

impl PublishStrategy for NoPublishLinks {
    fn expected_publish(&self, _view: &NodeView<'_>) -> BTreeSet<(PeerId, TopicId)> {
        BTreeSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::NoPublishLinks;
    use crate::strategies::publish::PublishStrategy;
    use crate::strategies::test_support::{candidates, downstream, subscriptions, view};

    // 015 FR-012: the default policy selects nothing, on any view.
    #[test]
    fn selects_no_targets() {
        let subs = subscriptions(&["t1"]);
        let cands = candidates(&[("t1", &["a", "b"])]);
        let store = downstream(&[]);
        assert!(NoPublishLinks
            .expected_publish(&view(&subs, &cands, &store))
            .is_empty());
    }
}
