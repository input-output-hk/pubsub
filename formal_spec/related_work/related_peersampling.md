# Byzantine Peer-Sampling / Membership Protocols

Peer sampling (membership) is the layer that continuously supplies each node with a random sample of peers to gossip with. Since every node communicates only with the peers this layer hands it, the layer determines whether an adversary can *eclipse* a node — surround it with malicious peers and cut it off from the honest network. Eclipse-resistance of the system therefore bottoms out here.

Two design branches:

- **Full membership** — track ~all peers, sample locally. Stronger, simpler eclipse-resistance; `O(N × churn)` per-node cost. → Fireflies.
- **Partial view** — keep a small, continuously refreshed random sample. Internet-scale; weaker per-node guarantees, open to targeting attacks. → Brahms, BASALT.


Inclusion criteria (mirror the pub/sub set): **Byzantine-resilient · no centralized runtime trust · peer-reviewed.**

## Candidates

| Protocol | Byzantine-resilient? | View | Peer-reviewed? | Verdict |
|---|---|---|---|---|
| **Fireflies** (Johansen, Allavena, van Renesse) | Yes — correct nodes cannot be eclipsed (probabilistic; TOCS Thm 3.2, `p_corrupt < ½`) | **Full** + pseudo-random mesh | EuroSys 2006; TOCS 2015 | **INCLUDED — analyzed.** Unbiasable hash-ring positions → no eclipse, with only a setup-time CA (= our blockchain). `O(N×churn)` cost; not rational-tolerant. |
| **Brahms** (Bortnikov, Gurevich, Keidar, Kliot, Shraer) | Yes — proven uniform sampling + connectivity (< 1/3 transient) | Partial | PODC 2008 | **CANDIDATE** — the classic Byzantine partial-view baseline. |
| **BASALT** (Auvolat, Bromberg, Frey, Mvondo, Taïani) | Yes — bounds Byzantine fraction in views via "stubborn chaotic search" (per-slot ranking functions, rotating seeds) + hierarchical IP-prefix sampling for Sybil resistance | Partial | Middleware 2023 | **CANDIDATE** — strongest recent partial-view design; built as the substrate for epidemic (Avalanche-style) consensus. Its IP-scarcity Sybil defense is partly moot given our chain-gated membership; the bounded view-corruption mechanism is the relevant part. |
| **LIFT** (LIP6) — *2026 review* | Yes — crypto-PRNG hub selection; extends Elevator | Partial (hub-sampling) | arXiv 2026 (status TBC) | **CANDIDATE** — recent Byzantine-resistant sampling building block. |
| **Honeybee** (Zhang & Bojja Venkatakrishnan) — *2026 review* | Yes — verifiable random walks | Partial (sampling for DAS) | ACM 2024 | **CANDIDATE** — uniform node sampling for Data-Availability Sampling; sampling-only. |
| **CYCLON / SCAMP / Newscast** | **No** — fragile under Byzantine faults | Partial | Yes | **EXCLUDED** — non-Byzantine ancestors of the partial-view branch (SecureCyclon is the hardened CYCLON descendant). |
| **SecureCyclon** (Antonov & Voulgaris) | Claimed — "dependable" peer sampling | Partial | ICDCS 2023 | **REJECTED — targeted eclipsing attack**: our analysis shows a reliable targeted eclipse of a chosen victim at low adversarial fraction μ. |

