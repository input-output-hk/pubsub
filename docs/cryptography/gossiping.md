# Digital Signatures for Gossiping

Briefly, the purpose of the peer sampling layer is to help each node gain a 
sufficient (uniformly) random view of the peer to peer network, robust against 
byzantine behavior. To reduce the impact of byzantine behavior, SecureCyclon --
the chosen instantiation for the peer sampling layer -- heavily relies on
digital signatures. The intuition is that having each node digitally sign
the information they gossip later facilitates detecting misbehavior. Once some
misbehavior by a given node is detected, this node can be blacklisted.
For instance, one concrete misbehavior is a node sharing more peer descriptors
than allowed per period. In order to non-repudiably detect this, each time a
descriptor is shared, it is signed by the sender -- including any previous 
signature by previous owners up to the creator of the descriptor.

One explicit assumption in the SecureCyclon paper is that identities are hard
to acquire. Cryptographically, this translates into a broad claim that every
single peer has access to only a (very) small number of digital signature keys. 
Technically, this is referred to as Sybil resistance, although there is a more
subtle sub-aspect in this context, which we refer to as grinding. We get back to
this later.

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
without a forward-secure scheme, albeit with delayed effect, providing a 
recovery window. In addition, proactive security can be targetted, but that 
would require integrating into Cardano's stack a scheme that is not directly 
supported. A more manual alternative would be to require that each peer 
registers (and authenticates via some long-term key) fresh SUF or KES key, 
periodically. Something similar is done now in Mithril. Either case, and 
assuming the efficiency provided by each option is acceptable, which option fits
best the target threat needs to be discussed.

## Candidate schemes

Ed25519, as implemented in Cardano, provides Strong Unforgeability (it has
canonical encodings, avoiding malleability.)

Cardano's KES signatures using Ed25519 as a base scheme is both SUF, and 
forward-secure. The Rust [input-output-hk/kes](https://github.com/input-output-hk/kes/tree/master)
crate supports KES instantiations of up to `2^7=128` periods. In this case,
the size of a secret key is `32+7*32+2*7*32=704` bytes, and the size of a
signature is `64+8*32=320` bytes. Validation via prototyping seems necessary
in order to assess whether these sizes -- and the costs of the associated
process -- are acceptable.

For proactive security, further exploration would be needed. ([ia.cr/2004/052](https://eprint.iacr.org/2004/052.pdf))
may be a good starting point.


## Sybil Resistance

A Sybil attack is one in which an adversary creates arbitrary identities as 
needed, to support further attacks. For instance, in the context of peer to
peer systems where nodes set connections to random other peers, a typical attack
consists in an adversary copying all connections of some target peer `T`, thus
isolating `T` from the rest of the network (this is an _eclipse attack_.)

There are two traditional ways to avoid this:

1. Require that the generated identities are anchored in some authorized
identity. For instance, new identities have to be certified (digitally signed)
by some trusted authority. Let's call this the _vertical_ approach, since it
requires the existence of "higher degrees of trust."

2. Require that generating an identity consumes some scarce resource. For 
instance, some computationally heavy task needs to be done -- like in the
case of proof of work -- or some financial risk has to be assumed -- like in
the case of proof of stake, or collateralized protocols. Let's call this the
_horizontal_ approach, as no prior authority needs to exist, and anybody can
just create new identities without any endorsement other than consuming the 
required resource.

Without entering into a detailed analysis, the vertical approach is easier when
the target use case has some pre-existing trust structure; whereas the 
horizontal approach is typically easier when there is no such structure, or when
creating it would be too cumbersome.

In the [use cases](https://docs.google.com/document/d/1wVUpgeAKWCC8Iy6DWStJRlTVX8ods3Upcpo1A-P08cI/edit?pli=1&tab=t.0) 
targetted initially for the Cardano PubSub system, we have:

| Use case | Publishers | Direct subscriber nodes | End-user reach | Message profile |
|---|---|---|---|---|
| IOG to SPOs | 1 | ~3000 | same as nodes | Bursts only. Latency: minutes. Lifetime: hours. |
| DReps to delegators | approx. 150 active | 10 to 50 wallet backends | hundreds of thousands | 1 to 10 per DRep per week. Latency: hours. Lifetime: days to weeks. |
| SPOs to delegators | 500 to 800 | 10 to 50 wallet backends | approx. 1M active delegators | 0 to 2 per pool per month. Latency: hours. Lifetime: days. |
| dApps to users | 10 to 50 active | low tens | tens of thousands | Highly variable, transactional possible. Latency: seconds. Lifetime: seconds to days. |

### Trust Anchors in the Vertical Approach

Importantly, both publishers and subscribers are required to run nodes in the 
peer sampling layer, which means that we need some sort of pre-existing trust
structure for both, in the case of choosing the vertical approach. Assuming a 
small number of publishers, it can be feasible to store their "roots of trust"
in the topic registry described in the [Cardano Pub/Sub Framework - Design and
Architecture report](https://drive.google.com/file/d/1oZiKWbW1mXgRbl8iYxsBzkapvSxlfVwF/view?usp=sharing),
and then require that any subsequent gossip exchange in the peer sampling layer
be rooted at those identities. Note, though, that the peer sampling layer should
be agnostic of topics. Thus, having to check concrete topics for determining the
root of trust for a peer sampling node seems counter intuitive. For subscribers
(and for publishers, if we want to avoid the previous discrepancy,) it seems 
reasonable to try relying on existing key material already present at the 
Cardano blockchain _just for the root of trust_. For instance, SPOs (resp. 
DReps) could use their SPO (resp. DRep) keys _over domain-separated and 
fixed-structure messages_. This would be enough for the "IOG to SPOs" use case in 
the previous table. For the use cases "DReps to delegators," and "SPOs to 
delegators," the publishers are DReps and SPOs, respectively, which are also 
covered by the previous analysis. However, wallet backends are not -- although 
as long as they are in the order of 10 to 50, it still seems feasible to store 
their root of trust in the topic registry.

Independently of whether the root of trust is stored in the topic registry, 
derived from op-certs, or from a DRep certificate, it is stored on chain. Hence,
prior to validating any PubSub identity using either of these as a root of 
trust, the PubSub node would have to query a Cardano (full) node.

#### Grinding Resistance

In the vertical case, even assuming that one PubSub node can only have one (or 
few) "main" root of trust -- e.g., the SPO cold key under which an opcert is 
verifiable -- grinding attacks may still be possible. For instance, if the SPO
cold key is just used to sign the concrete key pair that would be used for 
somehow deriving the PubSub node (gossiping) descriptor, a malicious node can 
just generate as many gossiping key pairs as needed, until reaching one that
produces some desirable descriptor (e.g., one that places it semantically
close to a target descriptor.) 

One option to address this may be to combine a vertical root of trust with
some horizontal approach. Although this would seem to impose "the worst" of
both worlds -- that is, requiring some fixed trust structure, and consuming
some scarce resource.

Alternatively, an option may be to introduce _even more structure_. For 
instance, requiring that the keys used to gossip per period are derived in
a deterministic way from information publicly accessible by anyone (in addition
to the known root of trust, known only to the node). Some notion of 
sequentiality is known to be useful in similar situations -- see, e.g. [Group
Signatures with User-Controlled and Sequential Linkability](https://eprint.iacr.org/2021/181).
This would require more work to be fully defined and analyzed. In the context
of Cardano, we may leverage the existing HD wallet structure to facilitate key
management and sequential ordering.


### Trust Anchors in the Horizontal Approach

If we don't rely on some known key/identity to prevent Sybil and grinding 
attacks via structure, the other known option is to force nodes to consume or
lock resources for each identity they create. For instance, following the
proof-of-work paradigm, we could have the descriptor be computed as a 
cryptographic hash of some values (including the gossiping public key and some
unpredictable randomness -- e.g. on-chain beacon -- to prevent precomputation) 
and force it to start with a concrete number of `0` bits. Or, alternatively, 
following the proof of stake paradigm, force the node operator to use the 
identifier of a transaction locking some amount of ADA (and specifying the
chosen public key in the metadata, for instance) as input to the hash
function used to derive the node descriptor. Other similar options may be 
possible.

The advantage of this type of approach is that the required structure (and thus,
deployment complexity) is simpler than in the vertical approach. The 
disadvantage is that it imposes wasting or locking resources that are otherwise 
scarce.

### Key Management

Independently of whether the keys used for gossiping are anchored vertically
or horizontally, a node would need to derive and/or share its keys. Relying on
[CIP 1852](https://cips.cardano.org/cip/CIP-1852) for this seems attractive at
first thought. For instance, a new `PubSub` value for `role` would enable
simple key management, and would be directly compatible with existing wallet
libraries. However, note that CIP 1852 restricts to Ed25519 keys, and it is
not clear at this point if Ed25519 is enough for PubSub gossiping -- although,
if we use Ed25519-based KES, this may still be an option.


## Summary and Next Steps

The analysis above identifies three a priori orthogonal dimensions that jointly
determine the cryptographic design of the gossiping layer:

1. **Signature scheme:** whether SUF alone, forward security (KES), or proactive
   security is required.
2. **Trust anchoring:** whether to adopt a vertical approach (rooting identities
   in existing chain credentials), a horizontal approach (consuming scarce
   resources), or a combination.
3. **Key derivation:** how gossiping keys are generated, managed, and rotated.

While orthogonal at first glance, these dimensions interact in the following
ways:

- **1 → 3:** The signature scheme directly constrains key derivation. KES keys
  are composite structures that cannot be used as CIP-1852 leaf keys directly.
  If KES is chosen, suitability of CIP-1852 for deriving base keys would need
  to be analyzed.

- **1 → 2 (vertical):** If periodic key refresh is adopted (manual proactive
  security), each refresh must be authorized by the trust anchor -- e.g., the
  SPO cold key must certify each new KES or SUF key. This parallels the existing
  opcert mechanism but needs explicit design for the PubSub context, and
  imposes a recurring operational burden on the root key holder.

- **2 (vertical) → 3:** Grinding resistance under the vertical approach may
  require structured key derivation (e.g., sequential HD derivation). In that
  case the trust anchor choice and the key derivation scheme are not independent:
  the derivation scheme is part of the grinding resistance mechanism, so 2 and 3
  must be co-designed.

- **2 (vertical) → operational dependency:** Validating any PubSub identity
  against a vertically anchored root of trust requires querying a Cardano full
  node. This introduces a latency and availability dependency that is independent
  of the cryptographic choices, and must be assessed against the liveness
  requirements of the peer sampling layer. Also, depending on the root of trust,
  there may not be one single solution (SPO cold keys, dRep certs, etc.),
  complicating the solution further.

- **2 (horizontal) → quantitative analysis:** Choosing between horizontal
  approaches (PoW-style descriptor grinding cost, or collateral) requires
  understanding their concrete effectiveness. Specifically: how much does
  restricting an adversary to a small number of identities (e.g., 3 instead of
  300) degrade their ability to carry out an eclipse attack? The answer depends
  on network size and SecureCyclon protocol parameters, and should be evaluated
  before committing to a specific mechanism.

**Suggested next steps:**

- [ ] **On 1:** Evaluate the practical impact of forward security and proactive
  security on recovery time after a node compromise, and assess whether the
  overhead of KES or some more advance proactively secure scheme (key and 
  signature sizes, update cost) is acceptable for the target deployment scale.
  See [gossiping-threat-model.md](gossiping-threat-model.md) for an extended
  analysis of how the scheme choice affects recoverability.
- [ ] **On 2 (horizontal):** Quantify the effectiveness of identity-limiting
  mechanisms in the SecureCyclon setting as a function of network size and
  protocol parameters, to establish whether the horizontal approach provides
  sufficient Sybil and grinding resistance in practice.
- [ ] **On 2 (vertical):** Assess the operational feasibility of on-chain trust
  anchor lookups, including the latency and availability requirements imposed on
  the Cardano node dependency.
- [ ] **On 3:** Defer key derivation design until the signature scheme (1) is
  decided. If KES (or a proactively secure scheme) is chosen, explore whether 
  CIP-1852 can serve as a seed source, and whether the resulting scheme is 
  compatible with the grinding resistance requirements of the chosen trust 
  anchoring approach (2).
