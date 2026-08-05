# fe2o3-contracts

This spike defines a small, target-neutral vocabulary shared by ordinary Rust,
future device lowering, and verification tooling. It deliberately models only:

- logical one-dimensional launch domains and checked physical geometry;
- in-domain thread witnesses;
- bounded indices and identity-mapped, per-thread write indices;
- kernel, executable, contract, and proof artifact identities; and
- `Unverified`, `Checked`, and `Verified` proof states.

The safe API can only create `Unverified` proof records. A future verifier/build
integration must validate a proof manifest and gain a private construction path
before it can issue `Checked` or `Verified` records. This prevents application
code from promoting an artifact by assertion.

This crate is `no_std`, contains no target or runtime dependencies, and does not
claim to model SIMT scheduling, barriers, atomics, arbitrary index mappings, or
compiler correctness.

## Trust boundary

There is no trusted `external_body` in this crate. `ArtifactDigest` stores an
identity supplied by build tooling; this spike does not calculate hashes or
validate tool output. The adjacent `verus_vecadd` harness documents its hardware
thread-ID boundary separately.
