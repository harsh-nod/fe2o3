# gfx942 Runtime Qualification and Timing

This harness executes the repository-owned VecAdd qualification fixture through
the public fe2o3 runtime context over the direct-KFD backend, then compares it
with deprecated HSA and HIP qualification oracles. All three paths load the
same SHA-pinned COV6
HSACO, use 1,048,576 `f32` elements, grid `[1048576, 1, 1]`, workgroup
`[256, 1, 1]`, and the exact input images defined by
`trusted-gfx942-vecadd-v1/policy-v1.txt`. The size and geometry are not
configurable because the qualification-only constructor admits only that
invocation.

Run on an MI300X host with ROCm 7.2.4 and Rust 1.94 or newer:

```sh
benchmarks/runtime_gfx942/run-mi300x.sh
```

`FE2O3_RUNTIME_GPU_INDEX` selects the deprecated HIP oracle device and the
script resolves the same device's KFD unique ID. The defaults are 10 warmups,
30 samples, and 10
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
| KFD `qualified_persistent_submit_wait_readback` | Exact output reset into retained coherent storage, typed `RuntimeContextV1` launch admission, persistent dispatch submit/wait/recycle, and generation-checked direct facade readback | Direct-KFD end-to-end qualification cost; compare cautiously with the deprecated HIP staged oracle |
| KFD `host_output_reset` | Repeated complete host-image validation/reuse and attached coherent output update before launch | KFD buffer-update policy only |
| KFD `synchronized_launch_wait` | Typed launch preparation, retained native binding, AQL publication, completion wait, and recycle; lazy facade readback is excluded | Host launch/wait comparison with persistent-buffer runtimes |
| KFD `facade_readback` | Generation- and effect-checked coherent copy directly into the caller-owned output buffer | KFD lazy readback policy only |
| KFD `phase_*` | Internal preparation, snapshot, authority, native-binding, publication, publication-to-completion, eager-readback, and recycle scopes | Diagnostic attribution; not a cross-backend parity metric |
| HSA `host_visible_submit_wait_readback` | Exact output reset outside timing, typed `RuntimeContextV1` launch, persistent host-visible buffers, host wait, submission release, and facade readback | Deprecated qualification oracle; not equivalent to KFD or HIP staging |
| HSA `synchronized_launch_wait` | Persistent host-visible buffers and facade launch/wait/release; exact output reset occurs before timing | Deprecated qualification measurement; HSA creates and releases a kernarg allocation and completion signal per submission |
| HIP `staged_submit_wait_readback` | Exact output reset, three host-to-device copies, module launch, output device-to-host copy, and stream synchronization | Deprecated staged oracle; not equivalent to KFD allocation policy |
| HIP `synchronized_launch_wait` | Persistent initialized buffers and host launch-to-stream-synchronize latency; exact output reset occurs before timing | Deprecated host launch/wait oracle |
| HIP `device_event_interval` | One exact module launch between HIP device events; reset and validation occur outside the event interval | Deprecated GPU-interval oracle only |

One untimed KFD launch records, waits, and releases a runtime event to cover the
typed event lifecycle without charging event management to every timed launch.
The deprecated HSA lane additionally runs real six-event cross-stream fan-in
and enough sequential submissions to wrap its 64-slot queue.
The KFD path then checks every output byte after warmup and every sample. The
deprecated HSA and HIP paths check every output element after every sample; HIP
also checks the device-event launch.

One-time fixture admission, module resolution, allocation, deterministic host
input construction, compilation of the host harness, and final cleanup are not
timed. Normal KFD exit explicitly releases all submissions, allocations,
module and stream handles, consumes `RuntimeContextV1::shutdown`, and tears down
the native KFD queue.

Record GPU load and competing processes with the results. Measurements taken
under unrelated device load are diagnostic observations, not release numbers.
Only the KFD/HSA/HIP qualification `synchronized_launch_wait` rows have similar
timing boundaries, and their currentness, allocation, signal, and lazy-readback
policies still differ. Do not compute ratios across the other metric names.
These rows do not establish general HIP/HSA parity: they cover one artifact,
one geometry, one device, and the exact software stack printed by the runner.
For copy and XGMI logs, apply release thresholds to every matched row with the
fail-closed checker. Thresholds are mandatory arguments so a report cannot
silently inherit a more permissive policy:

```sh
python3 benchmarks/runtime_gfx942/check-parity.py result.txt \
  --schema fe2o3.async-copy-benchmark.v1 \
  --max-latency-ratio 1.10 \
  --min-bandwidth-ratio 0.90
```

The example values are illustrative, not an adopted release policy. Ratios are
always KFD divided by the named reference: lower latency is better, while
higher bandwidth is better. Thus a 10x speedup policy uses
`--max-latency-ratio 0.10` and/or `--min-bandwidth-ratio 10.0`; it is not
silently weakened to a parity-only bound. The checker rejects missing or
duplicate backend rows, nonpositive metrics or thresholds, mismatched device
IDs, warmup/sample counts, and any KFD p50/p95 latency or p50 bandwidth ratio
outside the supplied bounds. Multi-device copy and XGMI logs use their
corresponding schema names and require the same ordered pair of device IDs.

A passing threshold is evidence only for the rows, devices, timing boundary,
software stack, and workload encoded in that input. It does not establish a
runtime-wide speedup or permit extrapolation to unmeasured HIP/HSA APIs.

## Asynchronous Copy Qualification

Run the matched KFD, HSA, and HIP copy-engine harness on two idle MI300X GPUs:

```sh
benchmarks/runtime_gfx942/run-async-copy-mi300x.sh
```

The retained release measurement is
[`results/async-copy-mi300x-2026-09-02.md`](results/async-copy-mi300x-2026-09-02.md),
with the prior R7 baseline retained at
[`results/async-copy-mi300x-2026-09-01.md`](results/async-copy-mi300x-2026-09-01.md).
Each record links its exact raw runner output.

The default profile transfers 1 MiB at depths 1 and 16, with 10 warmups and 30
samples. `FE2O3_ASYNC_COPY_GPU_INDEX` and
`FE2O3_ASYNC_COPY_SECOND_GPU_INDEX` select the physical pair. The runner refuses
to publish a result when either relevant GPU exceeds
`FE2O3_ASYNC_COPY_MAX_BUSY_PERCENT`, which defaults to 5. Every backend phase
also has an outer foreground timeout, controlled by
`FE2O3_ASYNC_COPY_PHASE_TIMEOUT_SECONDS` and defaulting to 120 seconds.
`FE2O3_ASYNC_COPY_KFD_PROFILE` selects the single-device KFD lane from
`directional` (the default), `generic`, `engine0`, or `engine1`; the
multi-device KFD lane remains directional.

Single-device H2D and D2H rows include submission of the complete depth, host
waiting for every completion, and no allocation. KFD defaults to two targeted
queues, index 1 for H2D and index 0 for D2H, but serializes the measured
directions and does not claim directional overlap. Its split metrics report
submit and wait phases separately; `combined_*` reports the checked
submit-through-observed-completion API with one currentness envelope. The KFD
runner can select `generic`, `engine0`, or `engine1` for engine-policy
ablations. HSA uses `hsa_amd_memory_async_copy`, and HIP uses one nonblocking
stream per depth entry. The two-device rows publish both devices' work before either
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
condition. This harness does not benchmark peer copy: the KFD facade peer
operation is host staged, while HIP/HSA native peer operations would be a
different mechanism.

## Native XGMI Peer Qualification

Run the matched public-runtime-facade KFD, HSA, and HIP native peer-copy harness
on two idle MI300X GPUs:

```sh
benchmarks/runtime_gfx942/run-xgmi-peer-mi300x.sh
```

The defaults use physical GPU indices 0 and 1, 1 MiB transfers, depths 1 and
16, 10 warmups, and 30 samples. Override them with `FE2O3_XGMI_GPU_INDEX`,
`FE2O3_XGMI_PEER_GPU_INDEX`, `FE2O3_XGMI_BYTES`, `FE2O3_XGMI_DEPTHS`,
`FE2O3_XGMI_WARMUPS`, and `FE2O3_XGMI_SAMPLES`. The release load ceiling is 5%
per selected GPU and may be lowered or raised with
`FE2O3_XGMI_MAX_BUSY_PERCENT`; the per-phase outer timeout defaults to 120
seconds and is controlled by `FE2O3_XGMI_PHASE_TIMEOUT_SECONDS`.

All lanes use the same exact physical unique IDs and require
`gfx942:xnack-`. The KFD parity row uses only the public
`RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>` surface. The backend admits a
retained directional type-11 topology link and creates a BY_ENG_ID queue on its
one-bit recommended XGMI engine. HSA uses `hsa_amd_memory_async_copy`; HIP uses
`hipMemcpyPeerAsync` after exact peer access checks. Both directions are timed
separately. Allocation, access setup, changing per-round source patterns,
destination poisoning, readback, canary checks, and teardown are outside
timing. The KFD interval intentionally includes facade admission, per-copy peer
mapping, publication, observed completion, and peer unmapping. Every timing row
covers all depth submissions through observation of all completions and reports
p50/p95 nanoseconds plus p50 aggregate GB/s.

The runner requires a clean checkout, prints the Git commit and complete
software/device context, checks both GPUs immediately before and after every
backend phase, and removes its unique temporary build directory on every exit
path. A passing result measures only that exact pair, transfer profile, commit,
and software stack. It exercises facade integration but does not establish
general topology support or system-coherent atomic behavior.

The lower-level prepared-batch harness remains available as a mechanics
diagnostic and is deliberately excluded from the parity row:

```sh
cargo run --locked --release -p fe2o3-kfd --features live-validation \
  --example kfd-sdma-xgmi-peer-benchmark -- \
  UNIQUE_ID_0 UNIQUE_ID_1 BYTES DEPTH WARMUPS SAMPLES
```

That diagnostic holds peer mappings across a round and submits the complete
depth with one doorbell. Comparing it with the facade row isolates the current
facade's per-copy mapping and publication overhead; it is not a substitute for
the public API measurement.

This peer-copy harness does not exercise the separate structure-required
Worker V3 dispatch wrapper and makes no atomic or collective
performance claim. That wrapper admits only its finite integer-atomic,
LDS-primitive, and workgroup-barrier structure roster; all `_DPP` spellings
currently fail closed.

HSA/HIP results remain qualification/oracle evidence and do not identify
production backend alternatives.
