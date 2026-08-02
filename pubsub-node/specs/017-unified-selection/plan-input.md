# Plan input — 017-unified-selection

Technical direction supplied with the `/speckit-plan` invocation, recorded
verbatim:

> Design record: notes/017-unified-selection-pre-spec.md (untracked) carries
> the full pre-spec rationale; consult it for intent, the spec is canonical.
>
> Two-commit mapping: the pinned constraint is the A→B ordering, not a
> two-commit total. Commit A = the unified selection implementation + merged
> acceptance + experiments wiring, reproducing the current sampler derivation
> byte-exactly (the `experiments/uniform-sampler/v1` domain string, no nonce
> or self-id in the preimage, same ChaCha20 + `rand::seq::index::sample` over
> the sorted self-excluded candidate order), validated by byte-diff against
> the recorded baselines. Commit B = the domain rename + preimage extension
> (epoch nonce, self-id key bytes) + the `--selection-seed` loader derivation,
> then re-baseline and the statistical m2-comparison re-run. Other work (CLI
> surface, docs) splits into green-checkpoint commits as usual.
>
> Dependency promotion: uniform sampling in the node moves `rand`/`rand_chacha`
> from the experiments feature gate to unconditional dependencies — verify how
> they're gated today and record the promotion per the dependency-ADR practice
> (0001/0037 lineage).
>
> Preimage encoding: the seed/topic-draw preimage uses the crate's one
> length-prefix primitive (`push_len_prefixed`), mirroring the edge
> predicate's conventions (variable-width components length-prefixed, nonce
> fixed-width).
>
> Module layout: per ADR 0029 — the unified implementation replaces the
> per-strategy files under `strategies/connection/` and
> `strategies/acceptance/`; the trait files and `strategies/test_support`
> fixtures are updated in place, not duplicated. ADR 0028's two-phase
> construction applies to the new param shapes; parse-at-the-edge holds (the
> loader validates knob domains and derives seed bytes; constructors take
> parsed values).
