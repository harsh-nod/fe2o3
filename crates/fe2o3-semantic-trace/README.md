# fe2o3 semantic trace

This crate defines the collector-neutral V1 semantic execution-trace model and
its canonical binary encoding. A CPU Kernel-IR simulator is the first intended
producer. Direct-KFD execution, debugger sessions, and profiler imports can add
adapters without changing the trace envelope or truth model.

## Truth rules

- The Kernel-IR digest and length in a trace are untrusted producer claims. This
  crate never upgrades them into verified-canonical identity. An external
  adapter must compare the claim with an independently owned and validated V7
  artifact.
- Function, block, and operation ordinals are unresolved site claims. This
  crate checks only their syntactic event role. An external catalog/CFG adapter
  must establish existence and source/IR correspondence after authenticating
  the exact KIR claim. Names and raw addresses are never occurrence identities.
- `Observed` has no second collector field: the observer is exactly the header
  producer, whose kind must match the execution kind. CPU simulation therefore
  observes abstract Kernel-IR execution, never GPU behavior or performance.
- `Declared`, `Proved`, `Observed`, `Inferred`, and `Unavailable` are distinct.
  Consumers must not promote one class into another.
- Proof claims refer to exact external evidence. This crate does not validate a
  proof, authenticate an artifact, or grant compile, load, dispatch, or device
  authority.
- Provenance evidence has one representation in the bounded event evidence set.
  `Declared`, `Proved`, and `Inferred` each require exactly one reference of the
  corresponding kind; it counts toward the same per-event limit.
- Allocation and dispatch identities are opaque, nonzero identities in a
  declared domain. Native pointers, GPU virtual addresses, KFD handles, and
  queue identifiers are observations and must not be encoded as identities.
- Workgroup and local coordinates use canonical D1-D3 linearization with D1
  varying fastest. The exact logical grid is distinct from padded workgroup
  coverage. Wave/lane/logical coordinates must correlate, every active mask
  must exactly equal its D2/D3/multiwave tail mask, and a lane-scoped event must
  select an active lane.
- Allocations must be introduced as created, preexisting, or explicitly unknown
  before use. The trace validator checks generation, region, address space, and
  release transitions against memory outcomes. For each ordinal, generation
  zero is first, at most one generation is live, and reuse advances exactly by
  one after release. Zero-byte allocations are valid objects, while every
  memory access must still have a nonzero byte length.
- Complete traces cover sequence `0..n-1` and full dispatch boundaries. Gaps or
  missing lifecycle boundaries require explicit truncation/loss metadata;
  known loss cannot be zero. Dispatch, invocation, and operation begin/end
  records are validated against the declared capture boundaries.
- Every operation lifecycle carries a nonzero frame/occurrence pair. The pair
  disambiguates loops, recursion, and overlapping visits to the same lane/site;
  begin and end must bind the same pair, coordinate, and site.
- Event count, encoded bytes, resident bytes, and evidence references are
  bounded. Resident accounting uses actual retained vector and string
  capacities and includes the largest validation scratch phase or encoded
  output. Public constructors reject unaccounted over-capacity buffers.
  Encoding first performs an allocation-free exact-size pass, then reserves the
  output once and materializes without further growth. Encoder, decoder, and
  validator growth is fallible and typed. Lifecycle
  validation uses sorted indexes and logarithmic lookups rather than repeated
  linear scans. Decoding checks that the remaining input could contain the
  claimed event count before fallible reservation. Loss can never be inferred
  from a missing event.
- V1 intentionally carries no register or memory-value payload. Later value
  capture must be bounded, typed, and explicitly captured, redacted, or
  unavailable.

The binary codec is little-endian and rejects unknown tags, invalid UTF-8,
noncanonical evidence ordering, trailing bytes, impossible event counts,
noncanonical sequences, and any declared or global bound violation.
