use rand::Rng;

use crate::descriptor::{Descriptor, NodeId};

/// Bounded peer view backing the Cyclon protocol.
///
/// On every insertion the view enforces:
///
/// - A descriptor whose `node` equals the local `self_id` is silently
///   rejected, preventing an attacker from feeding the target's own
///   descriptor back to it.
/// - Inserting a second descriptor for an already-known peer replaces the
///   existing entry only if the incoming `created_at` is strictly newer; a
///   peer never occupies more than one slot.
/// - Once the view holds `view_len` descriptors, further inserts are
///   dropped.
pub struct View {
    slots: Vec<Descriptor>,
    view_len: usize,
    self_id: NodeId,
}

impl View {
    pub fn new(view_len: usize, self_id: NodeId) -> Self {
        Self {
            slots: Vec::with_capacity(view_len),
            view_len,
            self_id,
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.view_len
    }

    /// Insert a descriptor. Returns `true` if a slot changed (inserted or
    /// replaced).
    pub fn insert(&mut self, descriptor: Descriptor) -> bool {
        if descriptor.node == self.self_id {
            return false;
        }
        if let Some(pos) = self.slots.iter().position(|d| d.node == descriptor.node) {
            if descriptor.created_at > self.slots[pos].created_at {
                self.slots[pos] = descriptor;
                return true;
            }
            return false;
        }
        if self.slots.len() >= self.view_len {
            return false;
        }
        self.slots.push(descriptor);
        true
    }

    /// Removes and returns the descriptor with the smallest `created_at`.
    /// This is the partner-selection step of the Cyclon gossip cycle
    /// (paper §II.B, Fig. 1).
    pub fn take_oldest(&mut self) -> Option<Descriptor> {
        if self.slots.is_empty() {
            return None;
        }
        let mut idx = 0;
        let mut oldest = self.slots[0].created_at;
        for (i, d) in self.slots.iter().enumerate().skip(1) {
            if d.created_at < oldest {
                oldest = d.created_at;
                idx = i;
            }
        }
        Some(self.slots.swap_remove(idx))
    }

    /// Removes and returns up to `k` random descriptors, excluding
    /// `partner`. If the partner is drawn during sampling it is restored
    /// to the view at the end.
    pub fn take_random_excluding<R: Rng>(
        &mut self,
        k: usize,
        partner: &NodeId,
        rng: &mut R,
    ) -> Vec<Descriptor> {
        let mut selected = Vec::with_capacity(k);
        let mut ineligible: Vec<Descriptor> = Vec::new();
        while selected.len() < k && !self.slots.is_empty() {
            let r = rng.gen_range(0..self.slots.len());
            let d = self.slots.swap_remove(r);
            if d.node == *partner {
                ineligible.push(d);
            } else {
                selected.push(d);
            }
        }
        self.slots.extend(ineligible);
        selected
    }

    pub fn contains(&self, id: &NodeId) -> bool {
        self.slots.iter().any(|d| d.node == *id)
    }

    pub fn iter_descriptors(&self) -> impl Iterator<Item = &Descriptor> {
        self.slots.iter()
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        self.slots.iter().map(|d| d.node).collect()
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use rand::{rngs::StdRng, SeedableRng};

    use super::*;
    use crate::descriptor::Descriptor;

    fn node_id(byte: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = byte;
        NodeId::from_bytes(bytes)
    }

    fn addr() -> SocketAddr {
        "127.0.0.1:9999".parse().unwrap()
    }

    fn desc(byte: u8, created_at: u64) -> Descriptor {
        Descriptor::fresh(node_id(byte), addr(), created_at)
    }

    #[test]
    fn insert_respects_capacity() {
        let mut view = View::new(3, node_id(0));
        assert!(view.insert(desc(1, 100)));
        assert!(view.insert(desc(2, 100)));
        assert!(view.insert(desc(3, 100)));
        assert!(!view.insert(desc(4, 100)));
        assert_eq!(view.len(), 3);
    }

    #[test]
    fn insert_excludes_self() {
        let self_id = node_id(0);
        let mut view = View::new(5, self_id);
        let echoed = Descriptor::fresh(self_id, addr(), 42);
        assert!(!view.insert(echoed));
        assert!(view.is_empty());
    }

    #[test]
    fn insert_dedupes_by_node_id_keeping_newer() {
        let mut view = View::new(5, node_id(0));
        let original = desc(1, 100);
        let newer = desc(1, 200);
        let older = desc(1, 50);
        assert!(view.insert(original));
        assert!(view.insert(newer));
        assert!(!view.insert(older));
        assert_eq!(view.len(), 1);
        let kept = view.iter_descriptors().next().unwrap();
        assert_eq!(kept.created_at, 200);
    }

    #[test]
    fn take_oldest_returns_smallest_created_at() {
        let mut view = View::new(5, node_id(0));
        view.insert(desc(1, 100));
        view.insert(desc(2, 50));
        view.insert(desc(3, 200));
        let oldest = view.take_oldest().unwrap();
        assert_eq!(oldest.node, node_id(2));
        assert_eq!(view.len(), 2);
    }

    #[test]
    fn take_random_excluding_partner_skips_partner() {
        let mut view = View::new(5, node_id(0));
        view.insert(desc(1, 100));
        view.insert(desc(2, 100));
        view.insert(desc(3, 100));
        view.insert(desc(4, 100));
        let mut rng = StdRng::seed_from_u64(7);
        let partner = node_id(2);
        let picked = view.take_random_excluding(3, &partner, &mut rng);
        assert_eq!(picked.len(), 3);
        for d in &picked {
            assert_ne!(d.node, partner);
        }
        assert_eq!(view.len(), 1);
        assert!(view.contains(&partner));
    }

    #[test]
    fn take_random_excluding_handles_undersized_view() {
        let mut view = View::new(5, node_id(0));
        view.insert(desc(1, 100));
        view.insert(desc(2, 100));
        let mut rng = StdRng::seed_from_u64(1);
        let picked = view.take_random_excluding(10, &node_id(99), &mut rng);
        assert_eq!(picked.len(), 2);
        assert!(view.is_empty());
    }
}
