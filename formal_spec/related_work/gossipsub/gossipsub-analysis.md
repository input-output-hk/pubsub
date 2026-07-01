# GossipSub

*Topic-based pub/sub overlay for libp2p (deployed in Ethereum & Filecoin). Vyzovitis et al., PL-TechRep 2020 / arXiv 2007.02754; formal analysis by Kumar, von Hippel, Manolios, Nita-Rotaru, arXiv 2212.05197. PDFs in this folder.*

## What it is
A topic-based publish/subscribe overlay, in two layers:
- **v1.0 — dissemination (no security):** per topic, each peer keeps a bidirectional **mesh** of ≈`D` (=8) peers it eager-pushes full messages to, plus **lazy-pull gossip** (`IHAVE`/`IWANT`) to non-mesh peers. A heartbeat maintains the mesh (`GRAFT` to add, `PRUNE` to drop).
- **v1.1 — hardening (all the security):** a per-peer **score function** plus five mitigations. A peer whose score goes **negative is pruned**.

## Topology — a per-topic push mesh + pull gossip
GossipSub is **hybrid push–pull**, with two peer relationships maintained *per topic*:
- **Full-message mesh (eager push).** Each subscribed peer keeps a **bidirectional mesh** of a target `D` peers for that topic (`D ≈ 6–8`, kept within `D_low`/`D_high`, e.g. 4–12), chosen ~randomly from its known topic peers subject to score. Full messages are **eager-pushed** to all mesh peers; `GRAFT`/`PRUNE` (mutual) add/drop mesh links each heartbeat. The result is a **degree-bounded, roughly-regular random graph per topic** (undirected).
- **Metadata gossip (lazy pull).** To peers it is *not* meshed with (a `D_lazy` / `GossipFactor ≈ 0.25` subset of topic peers), a peer periodically gossips `IHAVE` (message-ID announcements); a peer missing a message replies `IWANT` to **pull** the full payload. This is the recovery / redundancy path and helps repair the mesh.
- **Flood-publish.** The *originator* of a message sends it to **all** its (positively-scored) topic peers, not just its mesh — an anti-eclipse hardening for publishers.

So: **push the full message over a small bounded per-topic mesh; pull missed messages off-mesh via `IHAVE`/`IWANT`.** The mesh is the dissemination backbone; the gossip provides redundancy and mesh repair. (Contrast Murmur, which is pure push over one undirected `≈2G` graph, and Drum, which is push+pull with random ports for DoS-resistance.)

## The score function
Each peer **locally** scores every neighbour from its own first-hand observations (scores are never shared — deliberately *not* a reputation system): per-topic signals (mesh tenure, first-message deliveries, delivery deficit, invalid messages) plus global ones (IP colocation, behavioural penalty). The per-topic contributions are **summed across all topics**, then a single sign test decides pruning. **All security is a property of how this function is configured.**

## Properties (formally analysed in ACL2s)
The intent ("behave well ⇒ promoted, behave badly ⇒ demoted") was decomposed into four properties:
1. **Liveness / eventual demotion** — continuously non-positive behaviour ⇒ eventually pruned.
2. **Misbehaviour ↓ score** — more bad behaviour strictly lowers the score.
3. **Good behaviour monotone** — more good behaviour never lowers an established peer's score.
4. **Fairness / determinism** — identical behaviour ⇒ identical score.

- **Proved for all configs:** #3 and #4.
- **Filecoin config:** satisfies all four — but only by *disabling* the punitive parts (`TopicCap=0`, zeroed delivery-penalty weights) and deferring defence to the app layer.
- **Refuted (Ethereum config):** violates #1 and #2 via the **multi-topic blind spot** — because the score sums across *all* topics before one sign test, a peer can withhold in a few *target* topics while behaving in many *cover* topics, keep an aggregate **positive** score, and so be **never pruned** → **perpetual, topic-selective eclipse / partition**, independent of network size or topology. (Compounded by a `TopicCap` flat region where extra misbehaviour has zero score effect.)

## Key weakness — eclipse bottoms out on the discovery layer
Score-evasion is **deterministic** once an attacker surrounds a victim; the only probabilistic step is **positioning** (becoming all of a victim's mesh peers for the target topic), so `P(eclipse) = P(positioning) × 1`. But GossipSub does **no peer discovery** — it grafts from whatever a *separate* layer (Ethereum **discv5** / Kademlia DHT) supplies, and that layer is **not Byzantine-resistant** (free/grindable node IDs, no Sybil gate, demonstrated low-resource eclipses). So positioning — and hence eclipse-resistance — is **bounded nowhere**: it rests on an unstated, unmet assumption that the discovery view is non-adversarial.
