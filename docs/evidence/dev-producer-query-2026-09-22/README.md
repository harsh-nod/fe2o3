# Actual Producer-Read Queries

Development evidence for actual-type producer queries and their writer lookup leaf.
This packet does not authorize native pending consumers or establish HIP/HSA parity.

Production and proof share writer lookup and producer status, validation, retained
inspection, lookup and public status. Contracts are unconditional on raw contents:
there is no custody, count/storage shape, capacity or desired-success premise.
Four paired harnesses independently execute actual and historical queries.
Successful retained status now uses one traversal, not the historical two.

Literal mirrored raw witnesses cover Pending, Unknown, Success and NoEffect with
irrelevant malformed free/count/incarnation storage. Resolved queries ignore both
an out-of-bounds old writer slot and a same-slot unrelated replacement. Full
reservation identity is checked before stored-request errors. These synthetic
states do not prove acquisition or settlement reachability.

`check.py` brackets twenty-nine scoped negative controls with two whole-root runs,
authenticates the pinned Verus closure, and records exact diagnostics and terminal
process receipts. `cargo-checks.py` records formatting, full model tests, Clippy,
release compilation and the instrumented frozen-baseline benchmark.
`performance.py` authenticates original producer and writer methods, unchanged
dependencies, reviewed receiver/helper adapters and exact historical declaration
extraction. Its parser checks 1,456 ordered rows across 104 cases, including the
variant-specific access counts. `audit.py` reconstructs inputs from Git objects
and checks receipts, diagnostics, serialization, hashes and tamper controls.

Production's typed immutable journal receiver uses Deref; proof uses the explicit
journal field. Both unchanged Deref implementations and the reviewed arguments
are source-authenticated, not a new proof of the trait implementation itself.
Physical storage, allocation/unwind, constructor and lifecycle reachability,
producer admission/release and remaining wrappers are separate open obligations.

CPU comparisons use one immutable owner, retain unrelated custody, and compare
exact results, full snapshots, journal access counts and storage identities.
Timing includes test-only counters, per-call timer, result mapping and dispatch.
Resets and assertions are outside timing. These non-exclusive CPU observations
are not uninstrumented production latency, GPU performance or a parity claim.

Replay from a checkout containing the signed source commit in `SOURCE`:

```sh
python3 docs/evidence/dev-producer-query-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-producer-query-2026-09-22 --selftest
```

For a fresh qualification, run `check.py --repo ... --verus ... --output ...`, then
`cargo-checks.py --repo ... --output ... --target ...` into new owned directories.
Probe mode is development-only and is rejected by the offline auditor.
`RESULTS.md` retains every benchmark case, including regressions.

