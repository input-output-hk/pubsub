use crate::message::Message;
use crate::peer::PeerId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivedDelivery {
    pub from: PeerId,
    pub message: Message,
}
