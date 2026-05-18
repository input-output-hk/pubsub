# Preliminary Analysis on Key Management for Cardano PubSub

There are two types of actions in the currently proposed Cardano PubSub 
architecture that require cryptographic keys: node authentication when
gossiping at the peer sampling layer (implemented via SecureCyclon), and
message authentication when publishing a message in some topic. We separately
analyze them next.

## Digital Signatures for Gossiping

Briefly, the purpose of the peer sampling layer is to help each node gain a 
sufficient (uniformly) random view of the peer to peer network, robust against 
byzantine behavior. To reduce the impact of byzantine behavior, SecureCyclon --
the chosen instantiation for the peer sampling layer -- heavily relies on
digital signatures. The intuition being that having each node digitally sign
the information they gossip later facilitates detecting misbehavior. Once some
misbehavior by a given node is detected, this node can be blacklisted.
For instance, one concrete misbehavior is a node sharing more peer descriptors
than allowed per period. In order to non-repudiably detect this, each time a
descriptor is shared, it is signed by the sender -- including any previous 
signature by previous owners up to the creator of the descriptor.

One explicit assumption in the SecureCyclon paper is that identities are hard
to acquire. Cryptographically, this translates into a broad claim that every
single peer has access to only a (very) small number of digital signature keys --
ideally, only one. Technically, this is referred to as Sybil resistance. We
get back to this later.

Assuming Sybil resistance, the next question is what security properties we
should demand from the chosen digital signature scheme. Intuitively, digital
signatures are used in Cardano PubSub with two goals in mind: (1) authenticate
nodes, and (2) ensure that misbehavior cannot be repudiated. Node authentication
seems less restrictive, but non-repudiation can be more subtle in this context.
For instance, assume that we use an existentially unforgeable digital signature
scheme (e.g., ECDSA with non-canonical signature encodings, where both `(r,s)`
and `(r,-s)`are valid signatures for the same message). A malicious node could
take an `(r,s)` signature by some honest node, and produce the negative 
counterpart. Naive implementations could interpret this as a misbehavior by the
honest node, forcing isolation of an otherwise honest node. Similarly, malicious
nodes could just do that on purpose with the hope of not being detected, and 
claim to have been attacked in case they are caught (effectively voiding 
non-repudiation, and thus misbehavior prevention). Thus, **strong unforgeability
seems necessary for gossip authentication**. 

Similarly, since SecureCyclon explicitly allows punishing malicious nodes for
which malicious past behavior is detected, we need to consider sensitivity to
"past equivocation." In particular, imagine that a malicious node gossips at a
frequency that is higher than the agreed one. If detected, this node could argue
that its keys must have been compromised in the past. Conversely, if an honest
node is compromised, the attacker could easily isolate it by creating signatures
allegedly belonging to a past period in which the compromised node already 
gossiped -- hence exceeding the gossip rate. Thus, it seems appropriate to
require that **the digital signature scheme used for gossip authentication has 
to be forward-secure**. 

Note though that, even with a forward-secure signature scheme, if an adversary
corrupts an honest node, the adversary can just start impersonating the honest
node _for present and future_ signatures -- achieving the same end result as 
without a forward-secure scheme, albeit with more delayed effect. Thus, it is
logical to consider whether it makes sense to require a forward-secure scheme.
In addition, proactive security can be targetted, but that would require 
integrating into Cardano's stack a scheme that is not directly supported. A more
manual  alternative would be to require that each peer registers (and 
authenticate via some long-term key) fresh SUF or KES key, periodically. 
Something similar is done now in Mithril. Either case, and assuming the
efficiency provided by each option is acceptable, which option fits best the 
target threat needs to be discussed.

#### Candidate schemes

Ed25519, as implemented in Cardano, provides Strong Unforgeability (it has
canonical encodings, avoiding malleability.)

Cardano's KES signatures using Ed25519 as a base scheme is both SUF, and 
forward-secure. The Rust [input-outout-hk/kes](https://github.com/input-output-hk/kes/tree/master)
crate supports KES instantiations of up to `2^7=128` periods. In this case,
the size of a secret key is `32+7*32+2*7*32=704` bytes, and the size of a
signature is `64+8*32=320` bytes. Validation via prototyping seems necessary
in order to assess whether these sizes -- and the costs of the associated
process -- are acceptable.

For proactive security, further exploration would be needed. ([ia.cr/2004/052](https://eprint.iacr.org/2004/052.pdf))
may be a good starting point.


### Sybil Resistance



## Digital Signatures for Publishing Messages
