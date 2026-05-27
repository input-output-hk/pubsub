/// A message kind that nodes can exchange.
///
/// Currently only [`Message::Ping`] is defined; the enum is marked
/// `#[non_exhaustive]` so future iterations can add variants without
/// breaking external consumers that match non-exhaustively.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    /// A connectivity-probe message carrying an opaque numeric value.
    Ping(u64),
}
