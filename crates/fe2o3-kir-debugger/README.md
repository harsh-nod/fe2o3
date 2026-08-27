# fe2o3 KIR debugger

`fe2o3-kir-debugger` records bounded, execution-derived snapshots from the
deterministic CPU KIR simulator. It supports thread, logical-wave, workgroup,
and dispatch scopes; operation breakpoints; committed-memory watchpoints;
forward and reverse transcript navigation; and typed stack, SSA, and memory
inspection.

Reverse navigation moves over an immutable deterministic transcript. It does
not invert writes or claim physical GPU scheduling. Wave32 and Wave64 are
explicit visualization profiles over the canonical local-work-item order.

Source locations are optional sidecar claims bound to the exact canonical KIR
digest and byte length. The debugger rejects a source catalog whose identity
does not exactly match the simulated module.
