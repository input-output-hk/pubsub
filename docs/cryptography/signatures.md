# Preliminary Analysis on Key Management for Cardano PubSub

There are two types of actions in the currently proposed Cardano PubSub 
architecture that require cryptographic keys: node authentication when
gossiping at the peer sampling layer (implemented via SecureCyclon), and
message authentication when publishing a message in some topic. We separately
analyze them next.

[Digital Signatures for Gossiping](gossiping.md)
[Digital Signatures for Publishing](publishing.md)
