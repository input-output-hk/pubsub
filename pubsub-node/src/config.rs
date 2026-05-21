use crate::peer::PeerId;

#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub id: PeerId,
}

#[derive(Debug, Clone)]
pub struct PeerListConfig {
    pub peers: Vec<PeerEntry>,
}
