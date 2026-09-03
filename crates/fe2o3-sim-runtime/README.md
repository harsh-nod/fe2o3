# fe2o3-sim-runtime

`fe2o3-sim-runtime` is an explicitly selected, no-GPU implementation of
`fe2o3_runtime::RuntimeBackendV1`. It executes exact admitted `.fe2sim` V3,
V4, or V5 Kernel IR through `fe2o3-virtual-runtime`; it never probes for or
falls back to GPU execution. V5 revalidates an exact production V8/V9 to
same-module V10 bridge and admits the V10 execution bytes directly.

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
correspondences. V4 and V5 content-bind an independently versioned one-to-many
semantic component map, including explicit physical kernarg size, alignment,
and slots; V5 binds that map directly to its exact KIR V10 body. The production
exporter derives those components from rustc layout, ABI, and the sole
semantic-MIR-to-KIR lowering correspondence. The consumer independently
rederives canonical KIR packing and admits bounded, pointer-free by-value
arrays, tuples, and structs as an exact recursive scalar-leaf roster.
The retained source ABI must be zero-sized `Ignore`, scalar `Direct`, `Pair`,
GPU `Unadjusted` `Direct(Memory)`, a simple Rust integer `Cast`, or sized,
non-stack, non-metadata `Indirect` with exact carrier attributes. `Cast` and
`Indirect` are evidence about source transport, not simulator
materialization: callers supply each logical scalar leaf separately, and the
runtime never reads a raw aggregate, its padding, or an indirect carrier
pointer. It validates nested projections, scalar validity, direct and niche
discriminants, inactive-payload poison, region metadata, and physical slot
bounds. This grants semantic CPU simulation only, not compiler-execution, KFD,
or GPU launch authority.

An owned RegionSlice wrapper is admitted only when retained compiler facts show
the exact ordinary three-field pointer/usize/ZST layout, initialized pointer and
integer scalar-pair ABI, raw mutable pointer evidence, whole-value component
correspondence, ownership/access, and canonical pointer/extent slots. Structural
lookalikes, reordered fields or slots, and ownership/access substitutions fail
typed admission. Enums without exact discriminant/variant materialization,
niches, embedded pointers, adjusted values, complex or foreign casts,
metadata-bearing or stack indirect values, ambiguous storage, unsupported
wrapper regions, and layouts without exact slots also fail typed admission.
For a kernel with exactly one reachable canonical
dynamic LDS declaration, the normal launch geometry's `dynamic_shared_bytes`
is propagated as the explicit simulator byte extent, including zero. A nonzero
extent supplied to a kernel without such a declaration fails typed preflight
instead of being ignored. Multiple bases and `DynamicAtLeast` remain
unavailable. Peer copy, multiple devices, and host runtime collectives are not
advertised. KIR wave operations remain simulated kernel semantics; they do not
constitute a host collective primitive.
