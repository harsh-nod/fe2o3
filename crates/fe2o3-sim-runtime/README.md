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

Only exact scalar, thin global pointer, and global slice storage
correspondences are materialized. Cast, adjusted, indirect, aggregate,
ambiguous, reordered, and expanded ABI forms fail typed admission. Dynamic
shared memory, peer copy, multiple devices, and host runtime collectives are
not advertised. KIR wave operations remain simulated kernel semantics; they do
not constitute a host collective primitive.
