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
  ranges within fixed target alias domains;
- explicit target mutability and alias semantics: flat, global, and constant
  addresses conservatively share one physical alias domain, while constant
  storage is read-only;
- nested lifetime regions, shared/exclusive loans, monotonically issued borrow
  epochs, and stale-loan rejection;
- explicit range-, scope-, lifetime-, generation-, and access-bound capabilities
  for every raw access and every address-space cast;
- sequential typed/raw reads and writes, pointer-distance obligations, and
  nonoverlapping copy obligations;
- a pure, deterministic transition result containing descriptive obligations;
  and
- a canonical V2 codec whose decoder preflights each collection against the
  remaining minimum encoded bytes, cumulatively admits aggregate type edges and
  validity ranges before reservation, uses fallible reservation, and re-encodes
  to reject noncanonical input.

The executable model rejects out-of-bounds or misaligned places, stale or dead
provenance, incompatible aliases, uninitialized reads, invalid scalar bit
patterns, typed reads after validity-destroying byte writes, unauthorized raw
access, invalid address-space casts, nonintegral pointer distances, and
overlapping copies. A 32-bit allocation's exclusive arithmetic end may equal
`2^32`, but its base and every materialized access, zero-length, or
pointer-distance endpoint must be at most `u32::MAX`. Zero-sized allocations
claim no storage. A 64-bit exclusive end must fit in `u64`, matching executable
`checked_add`; storage starting at `u64::MAX` is therefore only valid at length
zero. Allocation IDs are single-use within one program, even after deallocation;
a new ID may reuse dead numeric storage with a new generation. Deallocation and
final live counts both require the allocation lifetime to contain the current
epoch.

Scalar validity ranges are strictly ordered with a gap between neighbors.
Overlapping or adjacent ranges and range encodings equivalent to `Any`,
`Bool`, `Char`, or `NonZero` are rejected, so accepted value sets have one
canonical representation.

`examples/verus_vecadd/verus/memory_safety_v2.rs` is a Verus-friendly pure
specification of selected executable predicates. Its target predicate includes
all five exact gfx942 pointer widths and alignments. It proves nested bounds,
stale generation rejection, `dead_at` and expiry liveness, deallocation making
all subsequent current-state observations dead, lifetime nesting, disjoint
exclusive loans, write-initialization, integral same-allocation element
distance, the distinction between a 32-bit exclusive range bound and a
materialized pointer, zero-sized non-overlap, the executable 64-bit
exclusive-end limit, conservative flat/global/constant aliasing, constant
read-only semantics, physical-range disjointness, and repaired validity-range
canonicality.

The Verus file defines a concrete field-for-field refinement relation between
`ExecutableAllocationFacts`/`ExecutableReadFacts` and the modeled allocation,
provenance, access, initialization, and epoch values. A mechanically checked
theorem proves equivalence between the executable and modeled predicates for
exactly provenance equality, current-state liveness, bounds, and initialized
read coverage. This is not a refinement proof for the Rust implementation or
for borrow, capability, raw-cast, identity, or full transition behavior.
Mutation-negative fixtures include stale generation and deallocated-liveness
counterexamples as well as the other listed boundaries.

## Resource and Trust Boundary

All externally sized collections have caller-selected budgets that may only
narrow immutable hard caps. One meter is carried through decode, structural
validation, canonical re-encoding, byte comparison, and program-identity
hashing; no phase may reset it. A decoded or constructed program privately
retains its admission usage. Execution and report verification continue from
that retained value through repeated structural validation and identity
hashing, and the report identity binds the final cumulative validation work.
The retained value has no public setter and cloning cannot reduce it. Decode
charges every input byte once, each
collection element before reservation, and every byte compared after
re-encoding. Validation charges target-name bytes and target entries, every
type/action/edge/range/projection traversal, cycle scratch initialization and
stack visits, and canonical output bytes. An in-place sort of `n` items costs
`n * ceil(log2(n))` (`n` for zero or one item); a sorted-slice lookup costs
`ceil(log2(n)) + 1`. All additions and products are checked before charging.
The 1,000-range regression therefore rejects both the formerly accepted
1,010-work construction budget and a decode-then-execute budget that only fits
one phase. Tests exercise exact success and one-less failure thresholds for
construction, canonical validation, decode, cumulative decode-through-identity,
execution, and report verification.

A separate hard-capped execution meter charges initial type/action traversal,
the three pre-reserved state-map capacities, every action, conservative sorted
state-map lookup and insertion work, insertion shifts, linear
allocation/loan/capability scans, recursive validity scratch and visits, both
preflight and mutation retain passes, each state sort using the sort formula
above, initialized/typed-state lookup, final liveness, and every obligation's
allocation lookup. Canonical action encoding charges emitted bytes. Every
identity charges each hashed input byte plus 64 units per SHA-256 compression
block, including padding. Report sizing is a checked fixed-width formula plus
one visit per transition; the report digest binds both the cumulative validation
work and the execution-meter value after its own work has been charged.

Decoder counts are checked and byte-preflighted before allocation. The aggregate
type-edge and validity-range counters use checked addition and are enforced
before each nested vector reservation, so multiple individually legal
collections cannot allocate beyond one global ceiling. Target strings (32
bytes), target entries (16), types (4,096), aggregate edges (16,384), validity
ranges (16,384), actions (65,536), and per-place projections (64) all reserve
fallibly under the 16 MiB canonical-byte ceiling. Cycle/validity scratch,
canonical writers, and transition records also reserve fallibly before growth.

Execution fallibly pre-reserves sorted state maps for at most 4,096 allocations,
16,384 loans, and 16,384 capabilities before the first action. The record vector
reserves the admitted action count. Each initialized and typed range vector is
limited to 65,536 entries; growth computes its post-retain size with checked
arithmetic, enforces the ceiling, and reserves before mutation. These vectors use
nonallocating unstable sorts. Per-action obligation vectors reserve each growth
before mutation and enforce the global 262,144-obligation ceiling before the
reservation. Failed map/range growth leaves that collection unchanged, as the
boundary regressions assert. Caller-owned vectors supplied to `new`, explicit
clones performed by callers or tests, and process setup remain outside this
admission boundary; the model does not claim global OOM recovery. All internal
input-sized codec and execution growth described above reports
`AllocationFailed` on failed fallible reservation.

Domain-separated SHA-256 identities bind codec semantics, the exact target,
type table, ordered actions including allocation generations, and every policy
field. Each obligation has its own identity over every obligation field, its
canonical ordinal, and its program/action enclosure. Obligation verification
first verifies the complete unchanged transition and then requires exact value
membership at that ordinal. Each transition has a distinct identity over every
transition field, ordered obligation identity, and repeated obligation field.
The report identity directly repeats every transition and obligation field and
also binds final epoch, live count, and final execution work. Verification APIs
recompute obligation-in-transition, transition-for-action, and full
program-to-report enclosure; detached mutation or substitution is rejected.
These remain unauthenticated content identities and grant no proof authority.

This foundation does **not** establish:

- that rustc or LLVM lowers Rust/MIR/Kernel IR to this model correctly;
- that a caller-authored trace matches a runtime allocation, launch, or HSACO;
- authenticated evidence that two target alias domains are physically disjoint
  on a particular runtime allocation (the fixed model instead fails
  conservatively for flat/global/constant overlap);
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
