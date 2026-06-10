use std::fmt;
use std::str::FromStr;

/// Failure modes returned when parsing a [`TopicId`] from a string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TopicIdError {
    #[error("topic id must not be empty")]
    Empty,
    #[error("topic id must not contain a NUL byte")]
    ContainsNul,
}

/// Logical identifier of a topic carried on every message.
///
/// Non-empty UTF-8, no internal NUL bytes. No additional character-class
/// restrictions: whitespace, punctuation, control characters (other than
/// NUL), and arbitrary Unicode are all permitted. Construct via [`FromStr`]:
///
/// ```
/// use std::str::FromStr;
/// use pubsub_node::TopicId;
/// let topic = TopicId::from_str("governance/announcements").unwrap();
/// assert_eq!(topic.as_str(), "governance/announcements");
/// ```
///
/// Topic ids are case-sensitive: `TopicId::from_str("T1")` and
/// `TopicId::from_str("t1")` parse to distinct values.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TopicId(String);

impl TopicId {
    /// Return the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TopicId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for TopicId {
    type Err = TopicIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(TopicIdError::Empty);
        }
        if s.contains('\0') {
            return Err(TopicIdError::ContainsNul);
        }
        Ok(Self(s.to_owned()))
    }
}

impl TryFrom<String> for TopicId {
    type Error = TopicIdError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            return Err(TopicIdError::Empty);
        }
        if s.contains('\0') {
            return Err(TopicIdError::ContainsNul);
        }
        Ok(Self(s))
    }
}

impl<'de> serde::Deserialize<'de> for TopicId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        TopicId::try_from(raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{TopicId, TopicIdError};
    use std::str::FromStr;

    #[test]
    fn empty_string_is_rejected() {
        assert_eq!(TopicId::from_str(""), Err(TopicIdError::Empty));
    }

    #[test]
    fn internal_nul_is_rejected() {
        assert_eq!(
            TopicId::from_str("bad\0topic"),
            Err(TopicIdError::ContainsNul),
        );
    }

    #[test]
    fn ordinary_utf8_is_accepted() {
        let topic = TopicId::from_str("governance/announcements").expect("valid");
        assert_eq!(topic.as_str(), "governance/announcements");
        assert_eq!(format!("{topic}"), "governance/announcements");
    }
}
