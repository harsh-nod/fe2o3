# fe2o3-kir-sim-trace

This crate adapts the serial CPU KIR simulator's ephemeral event stream to the
collector-neutral semantic trace V1 model. It has no HIP, HSA, KFD, ROCgdb, or
rocprof dependency and grants no compiler, proof, artifact, load, launch, or
GPU-equivalence authority.

## Truth boundary

- The KIR digest and length come from the admitted canonical V7 owner, but are
  copied into an inert trace claim. A consumer must bind the claim back to an
  independently owned canonical V7 module before resolving site ordinals.
- Site claims are vector ordinals in a catalog built from that admitted module.
  Function names, block IDs, and source names never become occurrence identity.
- All emitted facts are Observed by CpuKirSimulator during CpuKirSimulation.
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
- Wave32 or Wave64 is an explicit visualization profile. The simulator remains
  serial. Tail masks use the exact logical grid and canonical x-fastest D1-D3
  linearization.
- Event and byte budgets use exact canonical codec sizes. Dispatch closure is
  reserved before execution. If an invocation would cross a limit, its partial
  records are removed, the rejected callback is reported to the simulator as
  not retained, callbacks stop nonfatally, the execution result is unchanged,
  and the retained trace ends with the observed dispatch outcome.
- The trace resident limit is shared by the retained site catalog and all
  collector vectors. Catalog strings are copied fallibly, function and block
  resolution uses sorted indexes, allocation lookup is logarithmic, and frame,
  operation, allocation, and event vectors grow lazily and geometrically within
  that one ledger. Large simulator hard limits therefore do not cause
  trace-side preallocation or per-event reallocation.

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
