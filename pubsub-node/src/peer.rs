use std::fmt;
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum PeerIdError {
    #[error("peer id must not be empty")]
    Empty,
    #[error("peer id must not contain internal NUL bytes")]
    ContainsNul,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize)]
pub struct PeerId(String);

impl PeerId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PeerId {
    type Err = PeerIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(PeerIdError::Empty);
        }
        if s.contains('\0') {
            return Err(PeerIdError::ContainsNul);
        }
        Ok(Self(s.to_owned()))
    }
}

impl<'de> serde::Deserialize<'de> for PeerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        PeerId::from_str(&raw).map_err(serde::de::Error::custom)
    }
}

pub trait PeerDescriptor: Clone + Send + Sync + 'static {
    fn id(&self) -> &PeerId;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicPeerDescriptor {
    pub id: PeerId,
}

impl PeerDescriptor for BasicPeerDescriptor {
    fn id(&self) -> &PeerId {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::{PeerId, PeerIdError};
    use std::str::FromStr;

    #[test]
    fn empty_string_is_rejected() {
        assert!(matches!(PeerId::from_str(""), Err(PeerIdError::Empty)));
    }

    #[test]
    fn internal_nul_is_rejected() {
        assert!(matches!(
            PeerId::from_str("node\0a"),
            Err(PeerIdError::ContainsNul)
        ));
    }

    #[test]
    fn ordinary_utf8_is_accepted() {
        let id = PeerId::from_str("node-a").expect("valid id");
        assert_eq!(id.as_str(), "node-a");
        assert_eq!(format!("{id}"), "node-a");
    }
}
