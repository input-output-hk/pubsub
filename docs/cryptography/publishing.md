# Digital Signatures for Publishing Messages

For the publishing functionality, we assume that there is an (overlay) network
of connected peers, abstracting ourselves from the way in which this network is 
composed. This includes simplifications like the list-based approach. Thus, in 
publishing, we are only concerned with providing subscribers with the means to
verify that the messages they receive come from a legitimate (and intended)
publisher.

## Threat Model

Publishers are registered entities whose public keys are listed in the 
[on-chain topic registry](../staged-design-synthesis.md#21-topic-registry-on-chain). 
Some concrete threats to address:

- **Forgery.** A non-publisher constructs a message accepted by subscribers as 
coming from a legitimate publisher.
- **Replay / cross-topic replay.** A legitimately signed message is re-injected 
at a different position or on a different topic ([I-17](../aueb-gap-analysis-final.md#part-ii--implementation-level-observations)).
- **Equivocation.** A (possibly compromised) publisher signs two conflicting 
messages at the same sequence position, causing different subscribers to hold 
diverging histories. The [message envelope](../staged-design-synthesis.md#23-message-envelope) 
makes this detectable: each message commits to its predecessor via `parentHash`, 
forming a per-publisher chain; two messages sharing the same `parentHash` but 
differing in content constitute a verifiable equivocation proof without needing
to observe both branches simultaneously.
- **Key compromise.** An adversary obtains the publisher's signing key and can 
impersonate the publisher going forward. Post-compromise recovery requires a 
[registry key-rotation transaction](../staged-design-synthesis.md#21-topic-registry-on-chain). 
Retroactive forgery of *past* messages is a lower-priority concern than for 
[gossip nodes](gossiping.md): there is no mechanism for punishing a publisher's 
past behavior, so forward security buys little here. Additionally, the chain of
hashes (of sent messages) per publisher provides certain protection in this
regard as well.

## Proposed Scheme

**Scheme:** Ed25519 (SUF). Each publisher holds a long-term signing key 
registered in the [topic registry](../staged-design-synthesis.md#21-topic-registry-on-chain) 
per topic.

**Why not KES:** Forward security (or even more "advanced" schemes, e.g. 
providing proactive security) is less critical than for [gossip authentication](gossiping.md). 
There is no punish-past-behavior mechanism, and key rotation is already 
available via a [registry update transaction](../staged-design-synthesis.md#21-topic-registry-on-chain). 
[KES](gossiping.md#key-evolving-signatures)'s weight (704 B keys, 320 B 
signatures) is not justified.

**Signed surface:** The full [message envelope](../staged-design-synthesis.md#23-message-envelope) 
— `(topicId, publisherId, parentHash, sequence, timestamp, payload)`. Because 
the signature covers `topicId`, `parentHash`, and `sequence`, cross-topic replay
and reordering are closed by construction (see [envelope property 3](../staged-design-synthesis.md#23-message-envelope)).

## Trust anchor

The registry is coordination infrastructure only ([gap S-13](../aueb-gap-analysis-final.md#s-13--the-topic-registry-provides-coordination-not-identity-trust)); 
off-chain curation maps registered keys to real-world publishers, mirroring 
native-token metadata. 

However, when possible, a publisher can bind its pubsub key to an 
existing Cardano credential (cold key, DRep cert) via a self-signed certificate,
without coupling the protocol to on-chain validation. This depends on the use
case. For instance, use cases in which publishers (or subscribers) are SPOs or
dReps, they may use their SPO or dRep certificate as a root of trust for their
pubsub keys. Other cases may not have such a clear mapping.

### Key Management

Since SUF signatures seem enough in for publishing, and we expect many (or, at
least, relevant) entities in the Cardano ecosystem to be eventually interested
in PubSub, it is attractive to leverage the already existing Ed25519 signature
scheme, and extend [CIP 1852](https://cips.cardano.org/cip/CIP-1852) with a new
`PubSub` value for `role` to enable simple key management. Whether these keys
can be used for gossiping as well, it is still an open question and depends on
the chosen gossip layer (see [Digital Signatures for Gossiping](gossiping.md)).
