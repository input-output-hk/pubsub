# M2 model — high-level overview

## What M2 is

**M2** is a hybrid dissemination model derived from RingCast under an adaptive grinding adversary. It combines two layers — one for regular nodes, one for a small privileged tier — and analyses the per-target eclipse probability of an honest node.

The model is designed to characterise eclipse resistance in a setting where the deterministic Harary backbone of RingCast is no longer trustworthy because adversaries can grind their identifiers to choose where they land on the ring.

## Three classes of nodes

The network has N nodes, split into three disjoint classes:

- **Golden** (G of them) — small, trusted, never failing, never corrupted. They are part of the protocol's privileged tier and provide a structural guarantee.
- **Regular honest** (H of them) — ordinary participants, behaving correctly but not privileged.
- **Adversarial** (k of them) — Sybils or compromised nodes. Worst-case modelled as silent (contribute nothing useful); placement may be adaptive.

## Two forwarding layers

Unlike RingCast (where every honest node forwards along the ring + RF random push targets), M2 keeps two distinct mechanisms:

1. **Golden push.** Each golden node picks F_g random targets per round and forwards the rumour to them. F_g can be much larger than the regular fanout — it is the design lever for the privileged tier.
2. **Regular pull (the partitioning mitigation).** Each regular honest node *requests* RF other nodes to act as forwarders for itself, instead of waiting passively to be selected. This is the "ask, don't wait" inversion of RingCast's r-links.

Importantly, **the deterministic ring (d-links) is dropped** in M2. The justification: an adaptive adversary can grind its identifiers cheaply until it lands next to a chosen target on the ring, defeating the d-link contribution. Treating the ring as compromised is the conservative, threat-model-honest choice.

## What eclipse means

A regular honest node j is *eclipsed* in a round if it receives no useful in-edge — equivalently, **both** of the following happen:
1. No golden node selected j as one of its F_g push targets.
2. All RF forwarders that j requested happen to be adversarial.

This is the conjunction of two failures in two independent random layers.

## The headline result

Because the two layers' randomness is independent, the eclipse probability **factors**:

```
P(j eclipsed)  ≈   exp(− G·F_g / N)   ·   (k/N)^RF
                   └────── push ──────┘   └── pull ──┘
```

The push factor depends only on the golden tier; the pull factor depends only on the adversary fraction and RF. Adversary tolerance follows by inverting:

```
k_max(ε)  ≈  N · ε^(1/RF) · exp(G·F_g / (N·RF)).
```

Three structural facts fall out of this:

- **Polynomial in ε^(1/RF)** — tightening ε is much harder at small RF. Doubling RF takes the square root of the previous tolerance.
- **No feasibility floor** — at k = 0, P(eclipse) is exactly zero (j actively avoids the adversarial pool by choosing its own forwarders). Any ε > 0 is achievable with a small enough adversary.
- **Golden tier is a multiplicative bonus** of factor exp(λ_push / RF), where λ_push = G·F_g / N. It does not depend on ε.

## Where M2 sits relative to RandCast

At equal fanout (F = RF, so equal per-node initiated traffic), M2 is **strictly better** than RandCast for every adversary fraction μ = k/N ∈ [0, 1). The pointwise ratio is

```
P_M2(eclipse) / P_RandCast(eclipse)  ≈  (μ · e^(1−μ))^F  ≤  1,
```

with equality only at μ = 1. The structural reason is that pull eliminates the random-graph "unlucky j has no in-edge" failure mode of push: in M2, j's honest in-degree is *deterministically* RF, whereas in RandCast it is random with mean ~F·(1−μ) and a non-trivial Poisson tail at zero.

## What M2 does not do

- **M2 assumes a grinding-resistant peer-sampling layer.** Regular j's RF picks must be uniform from an honestly populated peer-sampling cache (e.g., SecureCyclon-style). Cache poisoning would defeat the whole construction.
- **M2 is single-round, single-target.** Multi-round dynamics, fresh-node bootstrap, message loss, and active interference are out of scope.
- **M2 deliberately ignores d-links.** If the system can defeat grinding (proof-of-work or stake-bound IDs), d-links return as a real coverage mechanism that M2 throws away.

## Where to read more

- [`m2_eclipse_report.md`](m2_eclipse_report.md) — full derivation, validity regime of the approximations, running-example tables, comparison with RandCast.
- [`m2_eclipse_check.py`](m2_eclipse_check.py) — numerical verification.
- [`golden_tier_eclipse_calculator.html`](golden_tier_eclipse_calculator.html) — interactive calculator (toggles between RandCast and M2).
- [`golden_tier_eclipse_report.md`](golden_tier_eclipse_report.md) — the parallel RandCast / golden-tier analysis that M2 is compared against.
