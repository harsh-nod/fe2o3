# gfx942 Memory-Safety and Provenance V2 Foundation

Status: inert foundation. This work does not change parity status or production
admission.

## Existing Boundaries Audited

- `fe2o3-kernel-ir::formal_memory_obligations` extracts bounded affine accesses
  and caller obligations from verified IR. Its launch extent and index width are
  unauthenticated, and it does not model allocation generations, initialization,
  typed validity, borrow epochs, or dynamic lifetime transitions.
- `fe2o3-kernel-ir::region_effects` compares allocation-relative regions and
  synchronization epochs. It is useful for conflict reporting but is not an
  ownership, provenance, or typed-memory semantics.
- `fe2o3-kernel-analysis` explicitly reports conservative facts without granting
  checked, verified, or launch authority. Its machine-effect records do not turn
  caller-authored effects into observed GPU behavior.
- `fe2o3-verifier::static_view_proof` canonically binds caller-authored view,
  lifetime, target, and proof-request identities. Its API explicitly grants no
  proof, runtime, allocation, or lease authority.
- `examples/verus_vecadd/verus/permission_core.rs` proves allocation-relative
  range and simple shared/exclusive permission lemmas. It has no canonical
  executable model, allocation generation, nested lifetime, stale-loan, target
  layout, or raw-pointer capability semantics.

## Foundation

`crates/fe2o3-kernel-ir/src/memory_safety_v2.rs` is intentionally not exported.
The integration tests include it by path, matching the other untrusted V2
foundations. It supplies:

- an exact `gfx942:xnack-`, little-endian address-space profile, including
  64-bit flat/global/constant pointers and 32-bit workgroup/private pointers;
- deterministic type layouts with scalar bit-validity, aggregates, arrays,
  checked projections, alignment, and by-value cycle rejection;
- allocation identities plus generations, owners, half-open byte ranges,
  initialization, exact typed-write facts, and disjoint live numeric storage
  ranges within each address space;
- nested lifetime regions, shared/exclusive loans, monotonically issued borrow
  epochs, and stale-loan rejection;
- explicit range-, scope-, lifetime-, generation-, and access-bound capabilities
  for every raw access and every address-space cast;
- sequential typed/raw reads and writes, pointer-distance obligations, and
  nonoverlapping copy obligations;
- a pure, deterministic transition result containing descriptive obligations;
  and
- a canonical, versioned, bounded decoder that preflights collection counts and
  re-encodes to reject noncanonical input.

The executable model rejects out-of-bounds or misaligned places, stale or dead
provenance, incompatible aliases, uninitialized reads, invalid scalar bit
patterns, typed reads after validity-destroying byte writes, unauthorized raw
access, invalid address-space casts, nonintegral pointer distances, and
overlapping copies. A 32-bit allocation's exclusive arithmetic end may equal
`2^32`, but its base and every materialized access, zero-length, or
pointer-distance endpoint must be at most `u32::MAX`. Zero-sized allocations
claim no storage. Allocation IDs are single-use within one program, even after
deallocation; a new ID may reuse dead numeric storage with a new generation.

Scalar validity ranges are strictly ordered with a gap between neighbors.
Overlapping or adjacent ranges and range encodings equivalent to `Any`,
`Bool`, `Char`, or `NonZero` are rejected, so accepted value sets have one
canonical representation.

`examples/verus_vecadd/verus/memory_safety_v2.rs` is a Verus-friendly pure
specification of selected executable predicates. Its target predicate includes
all five exact gfx942 pointer widths and alignments. It proves nested bounds,
stale generation rejection, lifetime nesting, disjoint exclusive loans,
write-initialization, integral same-allocation element distance, the distinction
between a 32-bit exclusive range bound and a materialized pointer, zero-sized
non-overlap, physical-range disjointness, and the repaired validity-range
canonicality rules. Mutation-negative fixtures cover each of those boundaries.

## Resource and Trust Boundary

All externally sized collections have caller-selected hard budgets. One
cumulative validation-work counter charges target entries, type sorting and map
construction, type and edge traversals, every validity range, cycle traversal,
actions, and projections. Decoder collection counts are charged before vector
allocation. Arithmetic uses checked executable operations; the Verus layer uses
mathematical naturals and therefore does not stand in for executable overflow
checks. Canonical bytes bind the exact target profile, type table, and ordered
transition trace, but are content identity only.

This foundation does **not** establish:

- that rustc or LLVM lowers Rust/MIR/Kernel IR to this model correctly;
- that a caller-authored trace matches a runtime allocation, launch, or HSACO;
- that Verus or its solver executed in an authenticated production boundary;
- concurrent inter-invocation or inter-workgroup race freedom;
- allocation-ID reuse within one trace, reborrows, or a parent/child lifetime
  graph (loans are deliberately flat owner-issued regions, so reborrow cycles
  are not representable);
- enum/niche inhabitation or a complete aggregate bit-validity proof (aggregate
  opaque writes remain rejected whenever a recursively contained scalar is
  constrained);
- GPU memory-model, cache, volatile-MMIO, atomic, or barrier behavior;
- compiler-to-machine refinement or dynamic illegal-access detection; or
- Complete status for parity rows 04, 05, 06, or 50.

Those require source extraction and refinement, authenticated proof execution,
runtime binding, concurrency semantics, backend preservation, and gfx942
hardware evidence in later milestones.
