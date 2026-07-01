# Murmur — Probabilistic Broadcast

*Bottom layer of Scalable BRB (Guerraoui, Kuznetsov, Monti, Pavlović, Seredinschi, Vonlanthen — DISC 2019; arXiv 1908.01738v3, in this folder).*

## What it is
A **probabilistic broadcast** protocol: a designated sender `σ` broadcasts one message, and — with high probability — **every correct process eventually delivers it**. Gossip-based, with a single parameter: the gossip-sample size `G`.

## Properties (each may fail with probability ≤ `ε`)
- **No duplication** — a correct process delivers at most one message. *(deterministic)*
- **Integrity** — if a correct process delivers `m` and `σ` is correct, then `σ` actually broadcast `m`. *(deterministic, via signatures)*
- **Validity** — if `σ` is correct, it delivers its own message. *(deterministic, ε = 0)*
- **Totality** — if any correct process delivers, then every correct process eventually delivers, with probability ≥ `1 − ε`.

So for a correct sender, Validity + Totality give the headline: **all correct processes deliver, w.p. ≥ `1 − ε`.**

## Algorithm
- **Setup.** Each process picks `Ḡ ~ Poisson(G)` peers uniformly at random (via a sampling oracle `Ω`) as its gossip sample, and sends each a `Subscribe`. Subscriptions are **reciprocated**, so the gossip graph is undirected — a node's actual neighbor set is its `G` chosen peers **plus the ~`G` peers that chose it**, i.e. **`≈ 2G` (= `Θ(log N)`) in expectation**. This is a small *partial view*, not full membership.
- **Broadcast.** `σ` **signs** the message and sends it to its sample.
- **Relay.** On first receipt of a correctly-signed message, a process **delivers** it and **forwards** it to its whole sample (a `delivered` flag stops re-sending). Pure sign-and-flood.

## Why totality holds
The graph over the correct processes is an **Erdős–Rényi** random graph `G(C, p)` with `C ≈ (1−f)·N` correct nodes and edge probability `p ≈ 2G/N`. By the connectivity threshold it becomes connected — and a connected correct graph ⇒ totality — once expected degree exceeds `ln C`, i.e. **`G = Θ(log N)`**. The paper proves **`ε` decays exponentially in `G`** (and grows polynomially in the Byzantine fraction `f`), so a modest `G ≈ c·log N` makes `ε` negligible.

## Cost
- **Per process:** `O(G) = O(log N)` messages.
- **Latency:** graph diameter `O(log N / log log N)`.

## Assumptions
- A **uniform random sampling oracle `Ω`** over the membership. Totality holds only while `Ω` is (near-)uniform.
- **Unforgeable signatures** for authenticity.
