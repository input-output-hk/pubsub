#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    Ping(u64),
}
