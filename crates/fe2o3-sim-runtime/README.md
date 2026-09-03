# fe2o3-sim-runtime

`fe2o3-sim-runtime` is an explicitly selected, no-GPU implementation of
`fe2o3_runtime::RuntimeBackendV1`. It executes exact admitted `.fe2sim` V3
Kernel IR through `fe2o3-virtual-runtime`; it never probes for or falls back to
GPU execution.

The backend reports `hardware = false` and `performance_prediction = false`.
Its results are deterministic semantic-simulation evidence within the admitted
KIR subset, not hardware or performance evidence.

The ordinary `RuntimeContextV1` lifecycle is supported for one logical device:
typed allocation and copy, module load, exact-signature kernel resolution,
stream submission, event dependency, poll/wait, release, and shutdown. The
backend owns a dedicated CPU worker and a 64-command bounded channel.
Synchronous commands wait for a response; asynchronous submission uses
nonblocking backpressure and rejects before custody when the channel is full.
Shutdown closes the sender before joining, so a full or disconnected channel
cannot strand Drop, and every admitted dispatch is independently bounded by
the simulator limits. Worker loss makes the backend terminal and retains
outstanding resources.

V3 materializes exact scalar, thin global pointer, and global slice storage
correspondences. V4 content-binds an independently versioned one-to-many
semantic component map, including explicit physical kernarg size, alignment,
and slots. Those facts are available to typed debugger inspection, but V4 does
not grant compiler authority. Ordinary by-value aggregate execution therefore
fails with a typed unsupported error until the production compiler exports an
authenticated component and host packing plan. The consumer already validates
nested Rust projections, scalar validity, direct and niche discriminants,
inactive-payload poison, region metadata, and physical slot bounds without
reading padding or host pointer bytes. Embedded pointers, cast or adjusted
ABIs, indirect arguments, ambiguous storage, and layouts without exact slots
also fail typed admission. Dynamic shared memory, peer copy, multiple devices,
and host runtime collectives are not advertised. KIR wave operations remain
simulated kernel semantics; they do not constitute a host collective primitive.
