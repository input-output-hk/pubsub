# 016 — /speckit-plan input (verbatim)

The technical-direction text passed to `/speckit-plan`, byte-for-byte, per the
project convention that phase inputs are recorded verbatim.

```
Technical direction for the 016-experiments-framework plan.

Dependencies. One new dependency: serde_json, declared optional and tied to
the `experiments` cargo feature (compiled only when the feature is enabled;
JSONL/aggregates encoding; deterministic float formatting). No rayon:
parallel execution is a
std::thread::scope worker pool over a run-index work queue, collecting into
a pre-sized results vector — canonical order by construction, and the
parallelism-degree knob is the worker count (which also bounds in-flight
populations, the peak-memory requirement). The uniform sampler and seed
derivation reuse the existing rand/rand_chacha and sha2 dependencies (seed
derivation: SHA-256 over (master_seed, run_index), truncated — the documented
rule recorded in the manifest). Wilson 95% is a closed-form function, no
statistics dependency. SCC is a hand-rolled, iterative (explicit-stack —
recursion overflows at experiment scale) Kosaraju over the extracted
digraph, in its own module behind a small interface (replaceable): small,
dependency-free, and deliberately the same algorithm family the formal
folder's validator cross-checks with.

Module layout (all under src/experiments/, gated by the `experiments` cargo
feature; nothing re-exported from the library's public surface):
- population: participant storage (class + NodeState + strategy bundle),
  seeded population build (keys via the seeded mock crypto scheme, class
  assignment, registry pre-population or faithful-fold event scripts).
- driver: the wavefront scheduler (event queues per wave, Send routing,
  Misbehaved consumption, per-phase drains), phase orchestration
  (registration -> dial -> churn draw -> SCC passes -> publish -> measure).
- graph: propagation-digraph extraction (the dissemination-model dispatch —
  an enum with the M2 variant only), Kosaraju SCC + condensation, goodness
  + min-publisher-coverage, degree/sink statistics.
- metrics: drain observation (first-receipt waves, miss-cause
  classification, sends split by recipient class, suppressed accounting,
  the per-run identity assertion), run-record assembly.
- statistics: histograms (sparse integer maps; fixed-width bins for
  coverage fractions), means/percentiles, Wilson interval, aggregates
  assembly (a pure fold over run records in run-index order).
- sweep: manifest construction, run-seed derivation, the worker pool,
  canonical-order JSONL streaming, aggregates emission.
- config: parsed sweep description types; validation (eligible receivers
  nonempty, up-honest publisher exists, single topic).
- scripted: declarative scripted-topology builders for validation (line,
  star, full mesh) via the direct pre-population path.
The front-end is a second [[bin]] target named `experiments`
(required-features = ["experiments"]): clap flags for config path, output
directory, parallelism, detail flags; TOML sweep-description file parsed at
the edge; the experiments API takes already-parsed values.

Core changes: none beyond crate-internal access — the core is touched only
by the pub(crate) visibility the experiments module needs (reading
NodeState link records for extraction; constructing states for
pre-population); no strategy-seam or public-API changes. The
ordered-collection conversion of upstream/downstream is delegated to the
in-flight connection-link strategies PR (coordination note there);
determinism inside 016 never depends on core iteration order — the driver
canonicalises each wave's collected sends before routing (a stable sort on
a canonical send key) and builds all extraction/tally structures in sorted
or index-keyed form.

Implementation phasing (critical-path first, per project convention):
(1) driver + population + scripted topologies + the determinism/
known-topology validation tests — the instrument skeleton (wave
canonicalisation in from the start); (2) graph analytics + drain metrics +
the two-instrument cross-check and identity assertions; (3)
records/statistics/sweep runner + output contract + parallel determinism
tests; (4) front-end binary + config validation; (5) the two shipped
M2-comparison configs + the smoke test + quickstart documenting the manual
comparison procedure (including the uncertainty-methodology note).

Testing strategy: unit tests per module; integration tests feature-gated
behind `experiments` (scripted-topology exactness, determinism across
repeated executions and worker counts, cross-checks, smoke variant — smoke
budget: seconds); the green-checkpoint sweep runs the suite both without
and with --features experiments. Determinism testing is layered: the
workhorse is pure in-memory value equality (RunRecord/Aggregates derive
Eq; run twice and compare; workers 1 vs K and compare) — byte-identity
then follows because encoding is deterministic by construction (record and
aggregate types contain only order-stable containers: Vec/BTreeMap, never
a HashMap field; one focused serialization test pins it), and the
file-level byte comparison shrinks to one or two integration tests (tiny
sweep written twice to temp dirs, plus a differing worker count, files
diffed) that anchor SC-001's artifact-level claim. Logs remain operator
UX: no test asserts on log output; all measurement flows through
driver-owned state.

Risks the plan should carry: float-serialization determinism (mitigated by
serde_json + asserting byte-identical artifacts in tests); memory at
N = 20k x worker count (mitigated by the worker-count knob; the operating
point is a manual run where workers can be few); HashMap iteration order
reaching an observable path — core collections stay hash-based in this
feature, so the wave-canonicalisation step and sorted extraction structures
are load-bearing (a determinism test across process restarts guards them),
and any new driver-side collection must be ordered or index-keyed by
construction.
```
