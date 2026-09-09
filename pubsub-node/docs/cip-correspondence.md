# Where the reference node decides each CIP rule

The correspondence between the [CIP's Specification](../../docs/cip/README.md#specification) and the reference node, rule by rule, at one pinned commit. It is maintained here rather than in the CIP because every line number in it is that commit's and another revision of the tree will differ.

Each rule below is decided by the block of the [reference node](https://github.com/input-output-hk/pubsub/tree/fe75f49338487377fcf54180988f498c87770df9/pubsub-node) its row links to. The table carries rules of mechanism and only those, because a rule about behaviour is settled by reading the code that decides it where a rule fixing a byte string is not, so the encoding rules are left out of it. Every link is pinned to one commit, and the line numbers are that commit's; another revision of the tree will differ.

Three departures run through the whole table rather than any one row of it.

- **Signing is a mock scheme.** Every signature check is real in shape and not in cryptography.
- **Both registries are in-memory projections folded from live deltas.** Nothing is read at a fixed chain position and no epoch has a registration cutoff.
- **Every parameter is supplied as configuration.** The node derives neither the bucket count from [Table 2 of the CIP](../../docs/cip/README.md#table-2), nor the serving cap from its sizing rule, nor the pick count from a failure target.

The last column carries what is particular to one rule.

The byte encodings are the node's own throughout. Neither signature preimage carries the domain tag [Canonical encoding](../../docs/cip/README.md#canonical-encoding-and-domain-separation) fixes, the gate hashes under tags of the node's own naming and reduces its digest little-endian, and a handshake carries no epoch index, which is why the acceptor's second check has nothing below to decide it.

<div align="center">
<a name="table-1" id="table-1"></a>

| Rule | Decided in | Departure |
| --- | --- | --- |
| [Selection](../../docs/cip/README.md#selection): the pick randomness is private and unpredictable, and two nodes with identical registry entries do not pick alike | [`src/strategies/connection/selection.rs:144-169`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/strategies/connection/selection.rs#L144-L169) | — |
| [The relay link and the pick count](../../docs/cip/README.md#the-relay-link-and-the-pick-count): the dialler mirrors on acceptance, and an entry already active is re-affirmed rather than dropped as unsolicited | [`src/state/handlers/symmetric.rs:119-135`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/state/handlers/symmetric.rs#L119-L135) | the directional designs remain configurable |
| [Link establishment](../../docs/cip/README.md#link-establishment): one Terminated for each link held, on shutdown | [`src/state.rs:608-651`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/state.rs#L608-L651) | — |
| [Link establishment](../../docs/cip/README.md#link-establishment): the signature verifies under the key carried in the preimage, and a self-emitted request is dropped | [`src/state/handlers/mod.rs:27-75`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/state/handlers/mod.rs#L27-L75) | — |
| [Link establishment](../../docs/cip/README.md#link-establishment): a link already held is re-accepted idempotently, ahead of gate and cap | [`src/strategies/acceptance/mod.rs:117-153`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/strategies/acceptance/mod.rs#L117-L153) | — |
| [Link establishment](../../docs/cip/README.md#link-establishment): an acceptor evaluates a Request in the numbered order | [`src/strategies/acceptance/unified.rs:130-194`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/strategies/acceptance/unified.rs#L130-L194) | step 1 sits in the handler and step 2 is absent; the prototype also checks its configurable link kind, and a crossing short-circuits ahead of membership and the gate |
| [Link establishment](../../docs/cip/README.md#link-establishment): a crossing completes regardless of the budget and spends none of it, and an admission is counted as it is granted, only on a first insertion | [`src/state/handlers/symmetric.rs:59-117`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/state/handlers/symmetric.rs#L59-L117) | the budget is never refunded on severance, only on rotation |
| [Messages](../../docs/cip/README.md#messages): a recipient checks topic, authorisation, revocation and signature in that order, before acting on a message | [`src/state.rs:1022-1120`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/state.rs#L1022-L1120) | revocation, step 3, is absent; a link gate precedes step 1, and a signature failure severs the admitting link |
| [Publication policy](../../docs/cip/README.md#the-topic-registry): open or restricted, with an empty restricted list authorising nobody | [`src/topic_registry/topic_entry.rs:22-56`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/topic_registry/topic_entry.rs#L22-L56) | the prototype infers openness from an empty publisher set and cannot represent a restricted policy authorising nobody |
| [Dissemination](../../docs/cip/README.md#dissemination-recovery-and-retention): duplicates are suppressed by content hash, so two messages sharing a triple both propagate | [`src/state.rs:933-972`](https://github.com/input-output-hk/pubsub/blob/fe75f49338487377fcf54180988f498c87770df9/pubsub-node/src/state.rs#L933-L972) | the cache is unbounded, with no retention window |

<em>Table 1: Where the reference node decides each rule</em>

</div>

**What has no counterpart.** [Services](../../docs/cip/README.md#services) as transactions: the parameter output, the four registry-entry operations and the credentials that authorise them, the deposit, and topic creation. [The randomness beacon](../../docs/cip/README.md#the-randomness-beacon) and the epoch it fixes, the node's epoch being an opaque value a driver supplies, with no epoch length and no scheduled rotation. [The registration cutoff](../../docs/cip/README.md#lifecycle-and-the-registration-cutoff), membership being folded at the tip, so no derivation is pinned to a snapshot. [Address resolution](../../docs/cip/README.md#address-resolution), the node holding no endpoint of any kind. Gap detection, recovery and [retention](../../docs/cip/README.md#dissemination-recovery-and-retention): there is no range request, no per-publisher sequence mark and no bounded cache. The [proof of possession](../../docs/cip/README.md#identity-and-keys), the Bech32 display form, and revocation. And the sizing rules themselves, nothing in the node evaluating a coverage law, [Table 2 of the CIP](../../docs/cip/README.md#table-2), or the admissions-budget rule.
