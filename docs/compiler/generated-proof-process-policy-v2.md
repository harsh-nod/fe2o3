# Generated Proof Process Policy V2

`FunctionalRefinementVerusRuntimeLeaseV1::open` retains the legacy single-solver
controller, configuration digests, and execution-identity transcript unchanged.
`open_pinned_contexts_v2` explicitly selects a different, closed host policy for
the same authenticated runtime closure. Opening either lease is not proof or
signer authority. There is no fallback between policies.

## Pinned Lifecycle

The retained Verus revision is `b677dd5a766f25f56e9aa1e32621aa4e53304b47`.
Its `rust_verify/src/buckets.rs` groups functions by module; `verifier.rs` creates
an AIR context for each bucket. The bit-vector and nonlinear paths create a
synchronous spinoff context while the bucket's base context can remain alive.
`air/src/context.rs` lazily starts the SMT process. `air/src/smt_process.rs`
retains its `Child`, closes the input channel, and waits for that child on drop.
Thus legitimate stock behavior includes sequential solver processes and two
simultaneous direct solver children even with the fixed `--num-threads 1`.

The closed V2 policy admits at most 4,096 solver lifetimes and two live solver
groups. Pending births already occupy a slot. These are resource limits, not a
claim that every workload can complete within them. Larger lifecycles refuse.
The separately authenticated auxiliary verifier remains exactly one, and must
reach its successful terminal before a solver context is admitted.

## Custody

Only the authenticated verifier group may create a solver process. Each exec
must match the retained executable identity, exact descriptor closure, and
allowed executable maps before release. Solver, pending, and auxiliary process
parents cannot create another process. Existing authenticated threads and their
clone restrictions remain unchanged. Re-exec is refused.

A group releases its slot only after every tracked task has a genuine terminal
wait matching its authenticated exit checkpoint, including a successful leader
terminal. Queued terminal waits are validated before considering a new birth.
An earlier failure stays sticky. Numeric PID reuse is possible only after the
entire previous generation is retired; a PID alone never authenticates custody.
Existing global deadlines, 32-task limit, output bounds, filesystem/FD/mapping
checks, cleanup ownership, and quarantine behavior are unchanged.

## Identity And Authority

V2 includes its version, fixed bounds, and invocation policy in new
domain-separated verifier and solver configuration digests and in a new
execution-identity transcript. The physical runtime identity remains a hash of
the retained tool closure, not an implicit process-policy approval. Receipt
import still requires an independently configured exact toolchain identity.
A legacy accepted toolchain cannot import a V2 receipt merely because the tool
files and signer are the same.

The immutable runtime lease and each retained output carry the selected policy;
cross-policy substitution refuses. The V1 executor channel remains V1 and
refuses V2 owners and outputs. Its inert replies cannot become proof evidence.
Production V53 explicitly selects V2. No signer, trusted native backend, LLVM
trust selection, or proof-to-execution authority is supplied by this policy.

The census has fixed storage derived from the closed live limit, with no
per-lifetime allocation. Existing retained-runtime header charges include the
added policy field. Process limits are not a claim to meter solver RSS in the
canonical compiler resource ledger.

## Qualification

Ordinary tests cover exact/pending/live/total boundaries, sticky failures,
terminal ordering across group threads, generation reuse, legacy identities,
cross-policy associations, and actual controlled process lifecycles. Their
synthetic census rows and receipt fixture signatures are not execution evidence.
The separately ignored protected smoke submits real module and bit-vector
proofs through the pinned runtime. The original finite-domain child-module
proof and actual composed source proof remain separate runtime obligations;
none is replaced by flattening proof modules or skipping equations.
