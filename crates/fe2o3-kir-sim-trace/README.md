# fe2o3-kir-sim-trace

This crate adapts the deterministic cooperative CPU KIR simulator's ephemeral
event stream to the collector-neutral semantic trace model. Exact canonical KIR
V7 uses frozen trace envelope V1. Exact canonical KIR V9 and V10 use additive
trace envelope V2 with the same bounded event grammar. It has no HIP, HSA, KFD,
ROCgdb, or rocprof dependency and grants no compiler, proof, artifact, load,
launch, GPU-equivalence, or performance authority.

## Truth boundary

- The KIR version, digest, identity policy, and length come from the exact
  admitted canonical owner, but are copied into an inert trace claim. A
  consumer must bind the complete claim back to an independently owned
  exact-version canonical module before resolving site ordinals. The V1 adapter
  rejects V9/V10 and the V2 adapter rejects V7 rather than relabeling an owner.
- Site claims are vector ordinals in a catalog built from that admitted module.
  Function names, block IDs, and source names never become occurrence identity.
- All emitted facts are Observed by CpuKirSimulator during CpuKirSimulation.
- Static LDS create/release events have workgroup scope. Completed LDS reads and
  writes, integer atomics, and barrier arrivals have exact lane scope. Atomic
  accesses retain the trace schema's atomic kind and allocation-relative range;
  their exact operation, scope, ordering, and compare-exchange metadata remain
  bound through the canonical KIR site. Fence order points retain their
  operation begin/end and exact KIR site because trace V1 has no separate fence
  payload; the adapter does not mislabel them as barriers. One release event has
  workgroup scope for each compatible phase. Its participant count remains on
  the ephemeral simulator event; semantic trace V1 consumers recover the active
  logical participants from invocation scopes rather than from an invented wave
  history.
- Raw simulator allocation numbers are remapped to nonzero, generation-aware,
  trace-local IDs. Exact simulator preexisting/create/release observations bind
  layout and lifetime; the adapter invents neither. Empty argument buffers and
  zero-count private allocations retain their exact zero-byte lifecycles.
- The caller must provide a nonzero dispatch occurrence identity that is unique
  in the caller's trace namespace and must not be a raw runtime address or
  handle. Wave visualization never changes it. The separate configuration
  identity hashes KIR, target, launch, argument metadata, values, initialization
  state, and shared buffers. Treat that deterministic digest as sensitive:
  low-entropy inputs can be guessed offline. It grants no authority.
- Wave32 or Wave64 is an explicit visualization profile. Cooperative workgroup
  execution still does not simulate physical waves. Tail masks use the exact
  logical grid and canonical x-fastest D1-D3 linearization.
- Event and byte budgets use the selected envelope's exact canonical codec
  sizes. Dispatch closure is
  reserved before execution. If a cooperative workgroup would cross a limit,
  every record from that active workgroup is removed, including partial lane,
  barrier, and LDS lifecycles. The rejected callback is reported to the
  simulator as not retained, callbacks stop nonfatally, the execution result is
  unchanged, and the retained trace ends with the observed dispatch outcome.
- The trace resident limit is shared by the retained site catalog and all
  collector vectors. Catalog strings are copied fallibly, function and block
  resolution uses sorted indexes, allocation lookup is logarithmic, and frame,
  per-invocation frame and operation stacks plus allocation and event vectors
  grow lazily and geometrically within that one ledger. Multiple live lanes are
  kept separate and one workgroup checkpoint makes truncation deterministic.
  Large simulator hard limits therefore do not cause trace-side preallocation
  or per-event reallocation.

The simulator's schedule identity and bounded memory-conflict assessment remain
on SimulationExecutionV1. Semantic trace V1 has no typed field for those facts;
this adapter does not disguise them as diagnostics. Block entry and the chosen
branch target are exact simulator observations. Failed memory attempts remain
unavailable because the current stream reports only completed accesses. When
the capture is not already truncated and its budgets permit the record, a
dynamic execution failure produces a generic fault diagnostic at its resolved
site. The typed failure remains on the simulation result even when that trace
diagnostic cannot be retained.

Every operation visit receives a nonzero trace-local frame and occurrence pair
derived from the explicitly nested simulator lifecycle. This distinguishes
loops and direct or mutual recursive visits to the same site. Future KFD,
ROCgdb, and rocprof adapters should emit the same schema while preserving their
distinct producer and execution identities.
