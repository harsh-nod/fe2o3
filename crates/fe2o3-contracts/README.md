# fe2o3-contracts

This crate defines a target-neutral vocabulary shared by ordinary Rust, future
device lowering, and verification tooling. Its v1 specification layer models:

- logical one-dimensional launch domains and checked physical geometry;
- in-domain thread witnesses;
- bounded indices and identity-mapped, per-thread write indices;
- bounded symbolic allocation provenance, address spaces, and byte regions;
- shared-read and exclusive-write permissions with initialization state;
- branded one-dimensional launch domains and affine injective write mappings;
- self-contained executable proof obligations with precise failure reasons;
- bounded independent-thread checks for initialized reads, disjoint writes, and
  initialization after writes;
- bounded required and maximum 3D workgroup dimensions plus an optional
  minimum-resident-workgroups constraint;
- a gfx942 V1 unsafe-assembly declaration with explicit operand, option, and
  memory/control-flow effect sets;
- finite source loop bounds, fixed-width integer-switch cases, and structured
  break/continue vocabulary for the separately versioned frontend sidecar;
- kernel, executable, contract, and proof artifact identities; and
- `Unverified`, `Checked`, and `Verified` proof states.

The safe API can only create `Unverified` proof records. A future verifier/build
integration must validate a proof manifest and gain a private construction path
before it can issue `Checked` or `Verified` records. This prevents application
code from promoting an artifact by assertion.

This crate is `no_std`, contains no target or runtime dependencies, and does not
claim to model SIMT scheduling, barriers, atomics, arbitrary index mappings, or
compiler correctness. `AffineWriteMappingV1` covers identity and strided
independent writes; reductions, scatter with collisions, and synchronized
sharing need different contracts.

## Trust boundary

There is no trusted `external_body` in this crate. `ArtifactDigest` stores an
identity supplied by build tooling; this spike does not calculate hashes or
validate tool output. The adjacent `verus_vecadd` harness documents its hardware
thread-ID boundary separately.

All memory and launch identities in this crate are symbolic proof inputs. Safe
construction does not authenticate them. `IndependentThreadContractV1::evaluate`
checks internal consistency only: success is neither a Verus result nor runtime
authorization. The sealed `SpecificationFactV1` marker makes that type domain
explicit, and the crate intentionally exposes no conversion to host loading,
allocation, or launch tokens.

`KernelFrontendContractV1` is also descriptive. Its assembly declaration is an
allowlist that a compiler may validate against reachable assembly; constructing
it neither permits an instruction nor proves that generated code matches the
declaration. Target admission must recheck launch bounds and occupancy against
the exact device and executable.

The control-flow vocabulary is descriptive as well. Consumers must
authenticate source spans, validate the structured graph, and prove declared
loop bounds against MIR and the final executable before relying on it.
