# Graph Version Ledger Transactions

This slice extracts the actual `VersionLedger::begin` and `commit` bodies into
`async_engine/graph/versions/transition_bodies.rs`. Native code and Verus expand
the same two macros. Proof-only annotations are empty in the native invocation.
The previous iterator preflight becomes an equivalent indexed scan; signatures,
check order, refusal values, alias behavior, and write order remain unchanged.
`fail` is unchanged and has no new claim.

## Proven Boundary

The concrete proof retains all nine ledger fields, all five `Use` fields, all
four `Version` fields, the exact nominal node representation, and the actual
per-segment guard bodies and arguments. Allocation and report payloads are opaque
framed values, not supplied answers. Under an explicit index-bounds premise:

- Begin succeeds exactly when every original input/output guard accepts and the
  node has not started. Every refusal leaves the entire modeled ledger unchanged.
- Commit succeeds exactly when every original output guard accepts. Every
  refusal leaves the entire modeled ledger unchanged.
- Success performs the exact ordered prefix of writes, including duplicate
  segment/output aliases. No uniqueness premise silently replaces that behavior.
- Opaque allocation/report data and other unchanged fields remain unchanged.

An executable inhabitance case covers distinct segments, duplicate segments with
different outputs, and repeated identical outputs. CPU differential tests compare
the independently retained predecessor bodies over 2,304 phase/pointer/alias/read
combinations, plus explicit duplicate and empty-use cases. Capacity retention is
a CPU regression property, not an allocator theorem.

## Qualification

`check-graph-version-ledger-v1.py` binds thirteen exact source files and their active
includes, field/argument schemas, predecessor bodies and call-order checks.
`test-graph-version-ledger-v1.py` is CPU-only: it tests hostile source/diagnostic
inputs and constructs eighteen mutations without executing them.

`qualify-graph-version-ledger-v1.py --verus /absolute/pinned/verus --output /new/private/path`
requires clean signed source and the existing historical signer/tool closure.
It runs three complete-root positives and eighteen actual executable mutations,
using the original 120-second limit, default SMT resources and four threads.
Exact whole-root diagnostics, source continuity and process closure are mandatory;
timeouts, frontend errors, foreign spans and partial-root verification refuse.
The production pipeline invokes this as its eighth independent campaign.

Development discovery observed 13 obligations and eighteen logical failures with
positive before/after checks. That unsigned discovery is not a signed maintained
qualification. A fresh signed replay is required for each admitted input closure.

## Exclusions

The proof does not establish `prepare` partitioning/index bounds, native writes,
decoder correctness, Context journal/epoch behavior, allocation, destructors,
completion-authority transitions or whole HostStaging composition. Source checks
bind the existing host-write/commit/ready call order but are not executable proofs
of those adapters. No graph version becomes native GPU settlement authority.
