# fe2o3-compiler-api

`fe2o3-compiler-api` defines the target-neutral boundary between a future Rust
frontend adapter and the fe2o3 compiler driver. It contains only versioned data
contracts and validation. The crate has no dependencies on rustc internals,
Pliron, LLVM, HSA, a target runtime, or another fe2o3 crate.

The V1 API provides:

- bounded compile requests with an explicit `PlironShadow` or `PlironV1`
  selector;
- domain-specific, fixed-width identity commitments;
- bounded opaque stage snapshots and deterministic stage receipts;
- stable numeric diagnostics without paths or process-local handles; and
- transactional compile outputs containing, at most, an opaque executable
  candidate.

All collection, message, snapshot, and candidate sizes are checked against
hard V1 limits and caller-selected limits. Receipt and diagnostic sequences
must be contiguous. Snapshot order must match receipt order, and both snapshot
and obligation-set commitments must chain exactly across transformations. A
rejection must carry an error and cannot carry a candidate. `PlironShadow` is
inspect-only and can never return a candidate. Successful artifact-producing
output must end at an HSACO snapshot and bind its candidate to that snapshot.
`PlironV1` is the only selector permitted to produce such a candidate.

## Authority boundary

Identity bytes and opaque payloads are supplied by callers. This crate checks
their shape and domain separation but does not hash, authenticate, parse, or
prove them. A `CompileOutputV1`, stage receipt, snapshot, diagnostic, or
`ExecutableCandidateV1` grants no artifact publication, proof promotion,
module loading, dispatch, or launch authority. Those decisions belong to
separately reviewed artifact and runtime layers.

This crate deliberately defines no wire format. A future canonical codec must
be versioned separately, enforce exact framing and bounds before allocation,
and receive golden compatibility tests before any bytes become durable.
The surviving selector tags remain `2` and `3`, preserving existing in-memory
request commitments that include the numeric selector.

## Integration

The workspace integration owner must add this crate to the root workspace and
central dependency table. Future frontend and driver crates should depend on
this API, while compiler implementation types remain behind their adapters.
