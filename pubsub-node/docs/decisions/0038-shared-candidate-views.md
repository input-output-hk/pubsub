# ADR 0038: Shared per-topic candidate views — full-membership sets behind a self-excluding read seam

**Status**: Accepted
**Date**: 2026-07-27
**Feature**: none (PR #77/#102 follow-up — instrument performance)
**Source**: `specs/IMPLEMENTATION_NOTES.md` N-033 (experiment population memory)

## Context

`NodeState.candidates` held one owned `BTreeSet<PeerId>` per topic, with the
node's own id excluded at **fold time** — every membership fold inserted all
members except self. For a real node this is immaterial, but the experiments
driver owns N node cores per run, and v1's "view = full candidate set" means
those N cores each own an (N−1)-element copy of what is semantically the same
membership: O(N²) `PeerId` allocations per run, measured ~30 GB peak RSS at
N = 20 000 (N-033). The worker count doubles as the memory knob, forcing the
shipped operating-point sweep to `--workers 1`, and the plan's ~10⁵-node
populations are unreachable (~750 GB).

The N sets cannot be shared as they stand: each excludes a *different* member
(its own node), so no two are equal. Sharing requires the stored sets to be
identical across subscribers — which means storing the **full membership,
including self**, and moving self-exclusion to read time.

## Decision

- `NodeState.candidates` becomes `BTreeMap<TopicId, Arc<BTreeSet<PeerId>>>`,
  each set holding a topic's **full membership including this node**. The
  outer map stays per-node owned: which topics a node knows about is node-
  local state; only the per-topic membership set is a shared fact, and the
  `Arc` sits exactly at that granularity.
- Self-exclusion moves to a **read seam** on `NodeView`: `candidates_for`
  (iterator, skips the node's own id), `candidates_len` (self-excluded count —
  the bucket derivation's input, unchanged bit-for-bit), and `is_candidate`
  (the node's own id is never a candidate). The raw field leaves the view's
  public surface (`pub(crate)`); every strategy reads through the accessors,
  so the exclusion lives once. `candidates_snapshot` (the `Node::candidates`
  getter's source) filters self the same way — external read behavior is
  unchanged.
- Folds mutate through `Arc::make_mut`, guarded by a `contains` check so a
  no-op insert/remove never clones a shared set. A real node's maps are never
  shared, so `make_mut` is move-free there; the membership folds now insert
  the node's **own** entry into the sets like any member's.
- The experiments fast path (`prepopulate_candidates`) takes the `Arc`
  directly, so the driver can hand every core the same shared set (the
  sharing itself lands as the follow-on change; this ADR's change makes it
  possible and keeps the faithful-fold mode's state content identical to the
  fast path's).

## Consequences

- One membership set per topic per run instead of N: the O(N²) memory term
  collapses to O(N), the operating-point sweep stops being worker-bound, and
  the fast path stops doing O(N²) insert work at registration.
- The change is result-neutral by contract: selection order, bucket
  derivation inputs, and membership decisions are byte-for-byte what the
  fold-time exclusion produced. Verified two ways — unit tests pin the razor
  edges (a stored self must not shift `UniformSampler`'s sampled indices;
  `candidates_len` subtracts self only when present), and the M2 baseline
  sweeps must byte-diff identical (`notes/experiments-baselines/`).
- The faithful-fold registration mode does not share (each core folds its own
  events into its own sets) — it remains the small-N fidelity check, and its
  equivalence test now also pins that fold-built and prepopulated state agree
  under the new invariant.
- "The candidate set never contains self" changes from a storage invariant to
  a **read-seam** invariant. Code touching the stored sets directly (folds,
  future state readers) must remember they contain self; everything reading
  through `NodeView` or the snapshots cannot observe the difference.

## Alternatives considered

- **`Arc` over the whole map** (`Arc<BTreeMap<TopicId, BTreeSet<PeerId>>>`):
  shares at the wrong granularity. The map's key set is per-node state (its
  own subscribed topics) — identical across nodes only in single-topic,
  uniform-subscription populations, so whole-map sharing breaks on the first
  heterogeneous experiment; and any fold under a shared outer `Arc` deep-
  clones every set for every topic unless the inner sets are `Arc`ed too, at
  which point the outer `Arc` is redundant. The outer map is a handful of
  keys; all the memory is in the sets.
- **Interning peer ids** (`Arc` inside `PeerId`): smaller diff, no invariant
  change, but only a constant-factor saving — the O(N²) set-entry term
  remains, registration stays O(N²) time, and ~10⁵-node populations stay out
  of reach.
- **Keeping fold-time exclusion, sharing only in experiments** (prepopulate
  installs a full set, folds keep excluding self): makes "does the stored set
  contain self?" mode-dependent — the faithful-fold and fast-path states
  would differ in content, the equivalence check could no longer compare
  them, and a mode-conditional `candidates_len` is exactly the off-by-one
  that silently shifts every sampled topology. One invariant, one read seam.
- **Freeing candidates after the dial drain**: does not reduce peak RSS
  (workers establish concurrently), and a hardcoded free-after-establish
  conflicts with any future multi-round mode (N-034).
