# gfx942 Runtime Qualification and Timing

This harness executes the repository-owned VecAdd qualification fixture through
the public fe2o3 runtime context over the direct KFD and reviewed HSA backends,
then through HIP's module API. All three paths load the same SHA-pinned COV6
HSACO, use 1,048,576 `f32` elements, grid `[1048576, 1, 1]`, workgroup
`[256, 1, 1]`, and the exact input images defined by
`trusted-gfx942-vecadd-v1/policy-v1.txt`. The size and geometry are not
configurable because the qualification-only constructor admits only that
invocation.

Run on an MI300X host with ROCm 7.2.4 and Rust 1.94 or newer:

```sh
benchmarks/runtime_gfx942/run-mi300x.sh
```

`FE2O3_RUNTIME_GPU_INDEX` selects the HIP device and the script resolves the
same device's KFD unique ID. The defaults are 10 warmups, 30 samples, and 10
launches per sample. `FE2O3_RUNTIME_WARMUPS`, `FE2O3_RUNTIME_SAMPLES`, and
`FE2O3_RUNTIME_LAUNCHES_PER_SAMPLE` may change only those statistical controls.
Every temporary build artifact is placed under one unique directory and
removed by an exit and signal trap. The runner requires a clean checkout so the
printed Git commit identifies every host source and checked fixture byte.

## Measurement Scopes

The output is a line-oriented `key=value` stream. Percentiles are computed over
per-sample average microseconds.

| Metric | Included work | Comparable use |
| --- | --- | --- |
| KFD `qualified_persistent_submit_wait_readback` | Exact output reset into retained coherent storage, typed `RuntimeContextV1` launch admission, persistent dispatch submit/wait/recycle, and generation-checked direct facade readback | Direct-KFD end-to-end qualification cost; compare cautiously with HIP staged end to end |
| KFD `host_output_reset` | Repeated complete host-image validation/reuse and attached coherent output update before launch | KFD buffer-update policy only |
| KFD `synchronized_launch_wait` | Typed launch preparation, retained native binding, AQL publication, completion wait, and recycle; lazy facade readback is excluded | Host launch/wait comparison with persistent-buffer runtimes |
| KFD `facade_readback` | Generation- and effect-checked coherent copy directly into the caller-owned output buffer | KFD lazy readback policy only |
| KFD `phase_*` | Internal preparation, snapshot, authority, native-binding, publication, publication-to-completion, eager-readback, and recycle scopes | Diagnostic attribution; not a cross-backend parity metric |
| HSA `host_visible_submit_wait_readback` | Exact output reset outside timing, typed `RuntimeContextV1` launch, persistent host-visible buffers, host wait, submission release, and facade readback | Reviewed-HSA end-to-end host-visible path; not equivalent to KFD or HIP staging |
| HSA `synchronized_launch_wait` | Persistent host-visible buffers and facade launch/wait/release; exact output reset occurs before timing | Compare cautiously with the HIP row: HSA creates and releases a kernarg allocation and completion signal per submission |
| HIP `staged_submit_wait_readback` | Exact output reset, three host-to-device copies, module launch, output device-to-host copy, and stream synchronization | Staged end-to-end oracle; not equivalent to KFD allocation policy |
| HIP `synchronized_launch_wait` | Persistent initialized buffers and host launch-to-stream-synchronize latency; exact output reset occurs before timing | Host launch/wait comparison with persistent-buffer runtimes |
| HIP `device_event_interval` | One exact module launch between HIP device events; reset and validation occur outside the event interval | GPU execution interval only |

One untimed KFD launch records, waits, and releases a runtime event to cover the
typed event lifecycle without charging event management to every timed launch.
The HSA lane additionally runs real six-event cross-stream fan-in and enough
sequential submissions to wrap its 64-slot queue.
The KFD path then checks every output byte after warmup and every sample. The
HSA and HIP paths check every output element after every sample; HIP also checks
the device-event launch.

One-time fixture admission, module resolution, allocation, deterministic host
input construction, compilation of the host harness, and final cleanup are not
timed. Normal KFD exit explicitly releases all submissions, allocations,
module and stream handles, consumes `RuntimeContextV1::shutdown`, and tears down
the native KFD queue.

Record GPU load and competing processes with the results. Measurements taken
under unrelated device load are diagnostic observations, not release numbers.
Only the KFD/HSA/HIP `synchronized_launch_wait` rows have similar timing
boundaries, and their currentness, allocation, signal, and lazy-readback
policies still differ. Do not compute ratios across the other metric names.
These rows do not establish general HIP/HSA parity: they cover one artifact,
one geometry, one device, and the exact software stack printed by the runner.

## Asynchronous Copy Qualification

Run the matched KFD, HSA, and HIP copy-engine harness on two idle MI300X GPUs:

```sh
benchmarks/runtime_gfx942/run-async-copy-mi300x.sh
```

The default profile transfers 1 MiB at depths 1 and 16, with 10 warmups and 30
samples. `FE2O3_ASYNC_COPY_GPU_INDEX` and
`FE2O3_ASYNC_COPY_SECOND_GPU_INDEX` select the physical pair. The runner refuses
to publish a result when either relevant GPU exceeds
`FE2O3_ASYNC_COPY_MAX_BUSY_PERCENT`, which defaults to 5. Every backend phase
also has an outer foreground timeout, controlled by
`FE2O3_ASYNC_COPY_PHASE_TIMEOUT_SECONDS` and defaulting to 120 seconds.

Single-device H2D and D2H rows include submission of the complete depth, host
waiting for every completion, and no allocation. KFD uses one classic SDMA
queue, HSA uses `hsa_amd_memory_async_copy`, and HIP uses one nonblocking stream
per depth entry. The two-device rows publish both devices' work before either
wait and report aggregate bytes over the shared wall-clock interval. These are
aligned host submit-plus-wait boundaries and byte counts, not identical native
mechanisms or allocation/currentness policies.
The two-device HIP metric includes single-threaded `hipSetDevice` transitions;
the result row records this as `host_context=single-thread-device-switching`.

The allocation metrics have deliberately different names. KFD times a pooled
host-plus-device checkout/recycle pair, HSA times one device-pool
allocate/free pair, and HIP times one stream-ordered device
`hipMallocAsync`/`hipFreeAsync` pair. They expose each API's supported scope and
must not be treated as identical allocator operations.

Every warmup and measured round assigns a new pattern to every slot and device,
poisons each download buffer, and validates every returned byte. The runner
passes each KFD unique ID to the HSA and HIP lanes: HSA requires the exact
`GPU-%016llx` agent UUID, a `gfx942` target, and a disabled-XNACK system query.
HIP requires the exact 16-byte ASCII UUID and a `gfx942` architecture name with
the `xnack-` feature. The runner requests `HSA_XNACK=0` for both comparators. It
records Git, ROCm, Rust, physical identities, load-boundary samples, and the
frozen SDMA manifest digest. Percentiles use the nearest-rank definition over
complete rounds.

The load gate samples each selected GPU immediately before and after every
phase and refuses the result when either boundary exceeds the configured
threshold. It cannot detect an unrelated workload that starts and stops wholly
inside a phase; continuous machine exclusivity remains an external benchmark
condition. The harness does not benchmark peer copy: the implemented KFD
facade peer operation is host staged, while HIP/HSA native peer operations
would be a different mechanism.
