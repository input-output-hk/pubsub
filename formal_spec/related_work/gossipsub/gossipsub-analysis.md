# GossipSub — Protocol, Guarantees, and the Discovery Layer


## Summary

- GossipSub is a topic-based pub/sub overlay combining an **eager-push mesh** with **lazy-pull gossip**. All attack-resilience comes from a **per-peer score function** (introduced in v1.1) plus five mitigation strategies; the base v1.0 dissemination engine has no security.
- **Its security is entirely a property of how the score function is configured.** Correctness of that function was decomposed into **four formal properties**: two hold universally, two are configuration-dependent.
- **Proved:** monotonicity and fairness hold for *all* configurations; Filecoin's configuration satisfies all four — but only because it *disables* the punitive parts of the score function and defers defense to the application layer.
- **Refuted:** Ethereum's (ETH2.0) configuration **violates** the liveness and misbehavior-detection properties. A peer can misbehave in targeted topics *forever* while keeping a positive aggregate score, so it is **never pruned** — yielding perpetual topic-selective **eclipse** and **partition** attacks, independent of network size or topology.
- **The exploit is deterministic; only *positioning* is probabilistic.** Once an attacker is grafted around a victim, evading detection is a *certainty* — the score function is a deterministic, proven-fair function. The sole stochastic step is *getting positioned* (becoming all of the victim's neighbours for the target topic), and it is **not provably hard**: cheap for low-degree / sparse-subnet / freshly-joined targets, and GossipSub's own peer-promotion logic *assists* it. Net: `P(eclipse) = P(positioning) × 1`.
- The **discovery layer** GossipSub depends on (discv5 / Kademlia DHT) is **not Byzantine-resistant**: no Sybil gate, no proven eclipse bound, and a **critical, still-unresolved** eclipse exposure (Least Authority's discv5 audit rated it CRITICAL; the 2025 EGN study isolated nodes using **<0.3 % of the network**; older discv4 eclipses needed just **2 IP addresses**). GossipSub's eclipse-resistance silently assumes a non-adversarial discovery view that the deployed substrate does not provide.

---

## 1. What GossipSub is

A topic-based publish/subscribe overlay for permissionless networks, deployed as the messaging layer of **Filecoin** and the **Ethereum consensus layer**. It is built in two layers:

**v1.0 — dissemination engine (no security):**
- **Mesh / eager push.** Per topic, each peer maintains a bidirectional mesh of ≈ `D` peers and forwards full messages only to them. Degree bounds `D=8, D_low=6, D_high=12`.
- **Gossip / lazy pull.** Metadata (`IHAVE`/`IWANT`) is gossiped to a `GossipFactor` (0.25) fraction of non-mesh peers, letting off-mesh nodes pull what they missed.
- **Heartbeat (≈1 s; 0.7 s in ETH2.0).** Maintains the mesh: `GRAFT` to add, `PRUNE` to drop, with a 1-min prune backoff.

**v1.1 — hardening layer (all the security):** a per-peer **score function** plus score-consuming parameters (`D_score=6` retains best-scoring peers on over-subscription; `D_out=2` reserves outbound slots) and **five mitigations**: controlled mesh maintenance, opportunistic grafting, flood publishing, adaptive gossip dissemination, and prune backoff.

**The score function.** Each peer *locally* scores every neighbor `q` (scores are never shared — deliberately *not* a reputation system). A neighbor with **negative** score is pruned.

```
score(q) = TC( Σ_{t∈topics} tw(t) · Σ_{i∈{1,2,3,3b,4}} w_i(t)·P_i(t) )  +  Σ_{i∈{5,6,7}} w_i·P_i
           └──────────── per-topic, capped by TopicCap ────────────┘     └──── global ────┘
```
`TC(x)=min(x,TopicCap)` if `TopicCap≠0`, else `x`. Counters decay each interval. The per-topic sum ranges over **all known topics**, subscribed or not.

| Indicator | Scope | Sign | Captures | What A must observe about B |
|---|---|:--:|---|---|
| `P1` Time in mesh | topic | + | mesh tenure (anti-flash) | how long B has been in A's mesh — *A's own state* |
| `P2` First-message deliveries | topic | + | fast honest relaying | which msgs B delivered to A *first* — *B's message stream* |
| `P3` Mesh delivery-rate deficit (squared) | topic | − | silent under-delivery | B's delivery count vs expected threshold — *B's message stream* |
| `P3b` Mesh delivery failures (sticky) | topic | − | persistent under-delivery | B's no-show after `IHAVE`, + deficit captured at prune |
| `P4` Invalid messages | topic | − | invalid/garbage traffic | B's messages that fail validation |
| `P5` Application-specific | global | ± | app reward/penalty signal | external app hook (*not* the gossip stream) |
| `P6` IP colocation | global | − | IP-co-located Sybils | peers sharing B's IP — *connection metadata* |
| `P7` Behavioral penalty | global | − | graft/backoff abuse | B's re-`GRAFT` timing vs backoff — *control messages* |

**How it's computed:** every counter is A's own **first-hand** observation of B over their direct link (never shared, never third-party); the message stream B sends A (`P2`–`P4`) is the core signal — *is B a useful, honest relay on this topic?* Since the per-topic counters are kept per (neighbour, topic), A *does* observe B's under-delivery on a given topic — the score merely **sums it across topics** before the prune test (§4.1).

---

## 2. Properties

**Informal design goals.** Fast propagation (under the Filecoin 6 s / ETH2.0 3 s block deadlines), low bandwidth versus flooding, and resilience to Sybil/eclipse/censorship attacks.

**The "fundamental property" of the defense layer** (the security intent): *peers that behave poorly are demoted by their neighbors; peers that behave better-than-average are promoted; promotion/demotion is based entirely on behavior.* This is too informal to verify, so it was decomposed into four precise properties of the score function:

| # | Property | Statement (informal) |
|---|---|---|
| 1 | **Liveness / eventual demotion** | If a peer's performance in *any* topic is continuously non-positive, its overall score eventually becomes non-positive (→ pruned). `G(score_t ≤ 0) ⟹ F(score ≤ 0)` |
| 2 | **Misbehavior decreases score** | Increasing a bad-behavior counter strictly lowers the score. |
| 3 | **Good behavior is monotone** | Increasing a good-behavior counter never lowers an *established* mesh peer's score. |
| 4 | **Fairness / determinism** | Two peers behaving identically receive identical scores. |

These properties are **independent of network topology, size, and fraction of malicious peers** — they constrain only the score function. Consequently a counterexample is an attack against *every* network using that configuration, and a proof covers *all* of them.

---

## 3. What was proved

- **Properties 3 and 4 hold for all configurations.** Monotonicity follows because positive contributors are monotone; fairness follows from the score function being a pure (referentially transparent) function. (Proved in ACL2s.)
- **Filecoin's configuration satisfies all four properties** — *but with a caveat that matters:* it does so by setting `TopicCap=0` and zeroing the mesh-delivery penalty weights (`w3=w3b=0`) and the app component. I.e. it "passes" by **disabling the punitive parts of the score function** and relying on application-layer defenses. In isolation, Filecoin's GossipSub layer is therefore *less* able to punish under-delivery, not more.
- **The ACL2s model is faithful and executable** (~6,800 LOC, 203 definitions, 177 theorems), conformance-tested against the Go implementation, and is the **officially endorsed formal spec** of GossipSub.
- **Empirically (not a proof):** Protocol Labs' Testground emulations (1k honest + 4k Sybils, 20:1–40:1 connection ratios) found gs-v1.1 **delivered 100 % of messages under every tested attack** (eclipse, censor, degradation, cold-boot, covert-flash, attack-at-dawn, IP-colocation), with an estimated attack cost ≈ $40k/month. This establishes practical robustness against *coordinated-takeover* attacks, but is evidence, not a theorem, and did not test the stealthy single-peer scenario below.

---

## 4. What was refuted

- **Ethereum's (ETH2.0) configuration violates Properties 1 and 2.** The violations are *structural*s:
  - **Property 1 (liveness) fails — the multi-topic blind spot.** The score aggregates contributions across *all* topics before the single sign test. A peer can misbehave (never forward) in a few *target* topics while behaving well in many other subnet topics; the positive contributions outweigh the negatives, the aggregate stays **positive**, and the peer is **never pruned** — even though the per-topic under-delivery signal exists in the victim's own counters. ETH2.0 has 70+ topics, so the offset is easy.
  - **Property 2 (misbehavior↓) fails — the TopicCap flat region.** With `TopicCap=37.72` but an uncapped topic sum above it, additional misbehavior leaves the capped output unchanged: misbehavior has *zero* score effect.
- **Consequence — perpetual, position-based attacks that evade detection.** From these violations the authors synthesize **throttle/block, topic-selective eclipse, and partition** attacks. The key move: an attacker *behaves well in non-target topics to keep a positive aggregate score (never pruned)*, while withholding the target topic. To eclipse a victim on a topic, control **all of the victim's neighbors for that topic**; to partition a victim set, control a **vertex cut**. Validated on real testnet topologies (Ropsten/Goerli/Rinkeby) — e.g. a 6-victim partition with a 2-node cut — and shown to be **perpetual** (scores converge to a positive fixed point). These hold for *any* ETH2.0 network regardless of size or topology.
- **Scope caveat.** The ACL2s *theorems* concern the **score function** (a misbehaving peer evades pruning). The step to working eclipse/partition is by **construction + simulation on concrete topologies**, not a separate network-level theorem.
- **What is proved *nowhere* (in either the Protocol Labs or the formal work):**
  - an end-to-end **dissemination guarantee** ("every honest peer receives every message"),
  - a **partition/connectivity** invariant over topology classes,
  - a **detection-time** bound ("a misbehaving peer is pruned within Δ heartbeats").

  Protocol Labs shows delivery *empirically*; the formal work proves *score-function* properties and *simulates* attacks. The end-to-end question is open.

### 4.1 Topic-selective eclipse: why it works

Classic *all-topic* eclipse (silence the victim entirely) is defeated by gs-v1.1 — blocking everything earns a negative score everywhere → pruned. The refuted attack is **topic-selective** (block one topic; let the rest flow) and survives a structural mismatch:

- GossipSub keeps a **separate mesh per topic**, so an attacker can sit in the victim's mesh for the *target* topic `X` **and** for *cover* topics `Y, Z, …` at once.
- But the prune decision uses a **single aggregate score per neighbour**, summed across all topics. Withholding on `X` gives a negative `X`-contribution; relaying well on the cover topics gives positive ones; the sum stays **positive**, so the attacker is **never pruned from any mesh — including `X`'s**.

The signal needed to evict the attacker from `X` (its negative per-topic `X`-score) exists in the victim's own counters but is **averaged away before the prune decision** — precisely Property 1's failure. Had `X`-mesh membership been gated on the *per-topic* `X`-score, the attack would self-correct (prune → re-graft honest peers → eclipse breaks); the cross-topic aggregate is what defeats that natural defence. Feasibility (ETH2.0 weights): the attacker needs enough cover topics to offset the attacked ones — roughly `7.2 + 3.2·(t/T) > 24.7·(i/T)` with `t + i ≤ T`, for `i` attacked and `t` cover topics out of `T` — solvable precisely because ETH2.0 has many topics. (Filecoin's 2 topics leave no room for cover topics.)

### 4.2 Deterministic exploit, probabilistic prerequisite

The attack has two layers of opposite character:

- **Score-evasion (the proven result) is deterministic.** Given the config and the attacker's behaviour, the aggregate score is a deterministic computation (Property 4) and positive *by construction*; the paper exhibits a concrete witness and shows scores converge to a **fixed positive value repeated every heartbeat**. "Never pruned" is a *certainty* — robust even to the randomised over-subscription pruning, since `D_score` deterministically retains high-scorers.
- **Positioning (surrounding the victim) is probabilistic** and is *abstracted away* by the formal model (which instantiates all of the victim's neighbours as attackers). So the formal result is **conditional**: *given* the position, the eclipse is deterministic and perpetual. All uncertainty lives in achieving the position — a peer-discovery-layer question (§5). Hence `P(eclipse) = P(positioning) × 1`.

*Disclosure:* both Protocol Labs and the Ethereum Foundation acknowledged the findings; the EF was preparing a patch.

---

## 5. The underlying peer-discovery layer

**GossipSub does no discovery and no peer sampling.** It forms meshes from whatever peers a *separate* subsystem places in the libp2p peerstore; the spec states discovery is "pushed outside the core functionality." Its only discovery-adjacent feature, **Peer Exchange (PX)**, is opt-in, **off by default**, and recommended only for trusted bootstrap nodes (it is not used by the ETH2.0 spec).

**What actually does discovery in deployment:**
- **Ethereum consensus layer → discv5**, a Kademlia-style DHT over UDP. Peers carry signed **ENRs** advertising attestation-subnet bitfields (`attnets`/`syncnets`) and a fork id. A deterministic node-id→subnet **"backbone"** (`SUBNETS_PER_NODE=2`, 256-epoch subscriptions) gives subnets discoverable stability.
- **Filecoin → libp2p Kademlia DHT** (`/fil/<net>/kad/1.0.0`) plus a bootstrap list.

**Kademlia in one line.** A structured DHT using an **XOR distance metric** and **k-buckets** (k≈16–20) for O(log n) iterative lookups. Designed for efficiency and churn-tolerance, **not** for Byzantine adversaries.

**Security of the deployed substrate — the key finding: no Sybil gate, no proven eclipse bound.** Node IDs are free to generate/grind, and the XOR structure lets an adversary place IDs next to a target. Demonstrated, low-resource eclipses:

| Target | Attacker resources | Status |
|---|---|---|
| discv4 (geth <1.8) — Marcus et al. 2018 | **2 machines / 2 IPs** + grindable IDs | partially patched (geth v1.8) |
| discv4 (geth ≥1.8) — Henningsen et al. 2019 ("False Friends") | **2 IPs in distinct /24**, no reboot, rides churn | partially patched (geth v1.9) |
| discv5 — Least Authority audit 2019 | free id-grinding; 384 nodes/24 buckets in 7 h on 1 CPU core | **CRITICAL, unresolved** |
| discv5 / "Ethereum Global Network" — 2025 | **~300 nodes ≈ <0.3 % of network** → <1 % connection success | open |

discv5 fixed discv4's *transport-level* flaws (authenticated/encrypted sessions, signed ENRs) but **not** the root cause (free, grindable IDs + a target to grind toward). It is exactly the "predictable overlay" that theory shows is isolable with `O(log N)` chosen IDs.

**What secure designs achieve — and what they require.** Robustness costs machinery the deployments lack (certified or cost-bound IDs, quorums or continuous re-randomization, redundant routing); Sybil resistance is *always* a separate precondition (a cost-to-join).

| Design | Layer | Tolerated adversary | Guarantee class |
|---|---|---|---|
| **discv5** (deployed) | structured DHT | none | assumed |
| **S/Kademlia** (2007) | structured DHT | ~99 % lookups at 20 % adversarial | empirical |
| **Castro et al.** (OSDI'02) | structured DHT | ≤ 25 % | measured / probabilistic |
| **Young–Kate–Goldberg–Karsten** (ICDCS'10) | quorum DHT | < 1/3 per quorum | **proven** |
| **Awerbuch–Scheideler cuckoo rule** (SPAA'06) | DHT maintenance | ε < 1−1/k (≈ <1/4 w/ churn) | **proven** |
| **Brahms** (PODC'08) | unstructured peer sampling | < 1/3 (transient); any f eventually | **proven** (uniform sampling + connectivity) |
| **SecureCyclon** (ICDCS'23, IOG) | unstructured peer sampling | ~40–50 % (low swap length) | empirical (**no closed-form theorem**) |

Two recurring ceilings: **~1/3** (majority/voting-based: Brahms, Young–Kate) and **~1/4** (adaptive churn + DoS: Castro, cuckoo&flip).

**Is positioning hard? Not provably.** Because the exploit reduces to positioning (§4.2), the system's resistance to topic-selective eclipse *is* the difficulty of positioning — and no theorem bounds it. The evidence is unreassuring:
- *Cheap at the discovery layer.* Free, grindable discv5 IDs let an attacker dominate a victim's candidate set (Least Authority: CRITICAL, unresolved); full eclipses were shown with 2 IPs (discv4) and <0.3 % of nodes (discv5).
- *The score function assists positioning.* Graft eligibility is judged on the **aggregate** score, so a peer with a high *cover-topic* score is a prime graft candidate for the target topic *before delivering anything on it* — the same blind spot promotes the attacker *into* the mesh, not just keeps it there.
- *Cheapest for weak targets.* Low-degree nodes, sparsely-subscribed subnets (small meshes), and freshly-joined / rebooted nodes (empty tables) are easiest; an attacker can also ride natural churn into position over days, no reboot.

Real friction exists — `D_out` (victim-initiated outbound the attacker can't directly occupy), flood-publishing (a publisher reaching the victim directly), the eth2 subnet backbone (guaranteed-discoverable honest subscribers), and the need for *total, sustained* coverage — and it bites hardest for well-connected victims, widely-published topics, and *specifically chosen* targets. But none is a proof: positioning is a heuristic obstacle, not a guaranteed one.

**Verdict on the discovery layer.** The deployed substrate is **not Byzantine-resistant**: no Sybil gate, no proven eclipse bound, and a **critical, unresolved** eclipse exposure — the **Least Authority audit of discv5 rated it CRITICAL** (free ID-grinding; *"no known strategies that completely eliminate"* it), and it remains live (the **2025 EGN study** isolated nodes using **<0.3 % of the network**; older discv4 eclipses needed only **2 IPs**). discv5's transport-level hardening (authenticated sessions, signed ENRs) and the IP-diversity / disjoint-path / subnet-backbone heuristics raise the bar but **do not close the root cause** (free, grindable identities).

**The composition gap.** GossipSub's mesh-layer score function is provably hardened (in some configs), but it grafts only from the candidate pool this substrate supplies. So its eclipse-resistance rests on an **unstated, unmet assumption — that the discovery view is not adversarial — which the deployed substrate (discv5) does not satisfy.** Discharging it is *not a GossipSub fix*: it requires either a hardened DHT (S/Kademlia / Castro-style) or a Byzantine peer-sampling service (Brahms / SecureCyclon).

---

## 6. Conclusion

GossipSub's resilience is a property of a *configuration* of a *score function* layered on an *assumed-honest discovery view*. The chain of this analysis:

> GossipSub's mesh grafts only from the candidate pool the discovery layer supplies → eclipsing a node requires controlling that pool (*positioning*) → positioning is the sole probabilistic factor, `P(eclipse) = P(positioning) × 1` → positioning is governed by the discovery DHT → **the deployed DHT (discv5 / kad-dht) is not Byzantine-resistant and carries a critical, still-unresolved eclipse exposure.**

So GossipSub's eclipse-resistance — *however well its score function is configured* — **bottoms out on an unguaranteed, demonstrably-attackable foundation**, and `P(positioning)` is bounded nowhere. The single missing result that would tie the stack together is quantitative: a verified bound on **positioning / eclipse probability and detection time**, as a function of configuration and adversary fraction. It cannot come from GossipSub — making `P(positioning)` *provably* small is the job of a **Byzantine-resistant peer-sampling layer** (Brahms / SecureCyclon) beneath it, which is exactly where this analysis meets the SecureCyclon detection-speed work.

---

## References

1. Vyzovitis et al. *GossipSub: Attack-Resilient Message Propagation…* PL-TechRep-2020-002 / arXiv:2007.02754 (2020).
2. Vyzovitis et al. *Gossipsub-v1.1 Evaluation Report.* PL-TechRep-2020-001 (2020).
3. Kumar, von Hippel, Manolios, Nita-Rotaru. *Formal Model-Driven Analysis of Resilience of GossipSub…* arXiv:2212.05197 (2023).
4. Ethereum `consensus-specs`, phase0 *p2p-interface*; `libp2p`/`go-libp2p-pubsub`; Ethereum `devp2p`/discv5.
5. Maymounkov & Mazières. *Kademlia.* IPTPS 2002. — Douceur. *The Sybil Attack.* IPTPS 2002.
6. Marcus, Heilman, Goldberg. *Low-Resource Eclipse Attacks on Ethereum's P2P Network.* IACR ePrint 2018/236.
7. Henningsen et al. *Eclipsing Ethereum Peers with False Friends.* arXiv:1908.10141 (EuroS&P-W 2019).
8. Least Authority. *Node Discovery Protocol Review* (Ethereum Foundation, 2019).
9. Baumgart & Mies. *S/Kademlia.* ICPADS 2007.
10. Castro et al. *Secure Routing for Structured P2P Overlay Networks.* OSDI 2002.
11. Young, Kate, Goldberg, Karsten. *Practical Robust Communication in DHTs Tolerating a Byzantine Adversary.* ICDCS 2010.
12. Awerbuch & Scheideler. *Towards a Scalable and Robust DHT.* SPAA 2006.
13. Bortnikov, Gurevich, Keidar, Kliot, Shraer. *Brahms: Byzantine Resilient Random Membership Sampling.* PODC 2008.
14. Antonov & Voulgaris. *SecureCyclon: Dependable Peer Sampling.* ICDCS 2023 / arXiv:2309.02952.
