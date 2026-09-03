# fe2o3-sim-runtime

`fe2o3-sim-runtime` is an explicitly selected, no-GPU implementation of
`fe2o3_runtime::RuntimeBackendV1`. It executes exact admitted `.fe2sim` V3 or
V4 Kernel IR through `fe2o3-virtual-runtime`; it never probes for or falls back
to GPU execution.

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
and slots. The production exporter derives those components from rustc layout,
ABI, and the sole semantic-MIR-to-KIR lowering correspondence. The consumer
independently rederives canonical KIR packing and admits pointer-free by-value
aggregates only when their ABI is exact `Direct`, `Pair`, or zero-sized
`Ignore`. It validates nested projections, scalar validity, direct and niche
discriminants, inactive-payload poison, region metadata, and physical slot
bounds without reading padding or host pointer bytes. This grants semantic CPU
simulation only, not compiler-execution, KFD, or GPU launch authority.

Embedded pointers, adjusted, cast, or indirect ABIs, ambiguous storage,
unsupported wrapper regions, and layouts without exact slots fail typed
admission. For a kernel with exactly one reachable canonical
dynamic LDS declaration, the normal launch geometry's `dynamic_shared_bytes`
is propagated as the explicit simulator byte extent, including zero. A nonzero
extent supplied to a kernel without such a declaration fails typed preflight
instead of being ignored. Multiple bases and `DynamicAtLeast` remain
unavailable. Peer copy, multiple devices, and host runtime collectives are not
advertised. KIR wave operations remain simulated kernel semantics; they do not
constitute a host collective primitive.
