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
`FE2O3_RUNTIME_MAX_BUSY_PERCENT` sets the fail-closed preexisting-load ceiling
and defaults to 5. The runner checks that ceiling before and after every KFD,
HSA, and HIP phase; exceeding it invalidates the run instead of emitting a
qualification record.
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
IDs, warmup/sample counts, incomplete pre/post phase-load observations, and any
KFD p50/p95 latency or p50 bandwidth ratio outside the supplied bounds. It
also requires one canonical run context binding the rows to an exact commit,
target/XNACK mode, device roster, ROCm and Rust versions, queue profile or XGMI
timing scope, and load ceiling. Decimal arithmetic prevents binary-float
overflow or underflow from satisfying a bound. Multi-device copy and XGMI logs
use their corresponding schema names and require the same ordered pair of
device IDs. XGMI qualification requires both emitted KFD rows: the
`remap-per-round` row is a completeness diagnostic, while only the exact
`persistent-hot` row is ratioed against the persistent HSA and HIP peers.

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
`directional` (the default), `generic`, `engine0`, `engine1`, or one of the
balanced `striped2`, `striped4`, ..., `striped16` profiles; the
multi-device KFD lane remains directional. New schema-V1 records state both
facts independently as `kfd_profile` and `kfd_multi_profile=directional`.
Retained schema-V1 records made before the latter field was added remain
admissible because that runner contract already fixed the multi-device lane to
the directional profile.

Single-device H2D and D2H rows include submission of the complete depth, host
waiting for every completion, and no allocation. KFD defaults to two targeted
queues, index 1 for H2D and index 0 for D2H, but serializes the measured
directions and does not claim directional overlap. Its split metrics report
submit and wait phases separately; `combined_*` reports the checked
submit-through-observed-completion API with one currentness envelope. The KFD
runner can select `generic`, `engine0`, or `engine1` for engine-policy
ablations. A striped profile partitions each direction's depth into balanced
contiguous shards, publishes every shard to a distinct round-robin native
queue before waiting, and reports the exact configured queue count, active
concurrency, per-queue depth ceiling, and doorbell count. Striped rows omit
`combined_*`, because there is no single currentness envelope spanning those
multiple queues. HSA uses `hsa_amd_memory_async_copy`, and HIP uses one
nonblocking stream per depth entry. The two-device rows publish both devices'
work before either wait and report aggregate bytes over the shared wall-clock
interval. These are aligned host submit-plus-wait boundaries and byte counts,
not identical native mechanisms or allocation/currentness policies.
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

### R40 Aggregate Striped Copy Qualification

R40 measures the aggregate striped submission surface against matched HIP and
HSA copy workloads on the repository's fixed MI300X qualification device:

```sh
mkdir -p /tmp/fe2o3-r40-evidence
FE2O3_R40_OUTPUT_DIR=/tmp/fe2o3-r40-evidence \
  benchmarks/runtime_gfx942/run-r40-striped-mi300x.sh
```

The runner admits only physical GPU index 2 with unique ID
`0xd2e26fef80cf5c33`. It runs depth 112 with 10 warmups and 30 samples for
4 KiB and 1 MiB transfers. Each size covers combined directional-plus-striped
profiles with 2, 4, 8, and 14 striped queues, plus the standalone 16-queue
profile. Three slots rotate backend order as KFD/HSA/HIP, HSA/HIP/KFD, and
HIP/KFD/HSA. Workloads run forward, reverse, and rotated by five positions in
those slots.

The KFD binary contract is:

```text
kfd-sdma-copy-benchmark <unique-id> <bytes> <depth> <warmups> <samples> \
  <combined-striped2|combined-striped4|combined-striped8|combined-striped14|striped16> \
  <aggregate>
```

The HIP and HSA comparators accept logical device index, exact unique ID, the
same four statistical/shape values, logical queue count, and profile. HIP uses
that many nonblocking streams. HSA uses that many logical dependency lanes;
`physical_engine_count=not-observed` prevents the logical width from being
presented as a hardware-engine count. Every path assigns request `i` to
`(submission_ordinal + i) % q`, publishes in rotating queue-major order, and
validates every byte after every H2D-then-D2H round. Allocation, queue/stream
creation, request construction, pattern initialization, validation, and
teardown are outside the measured intervals.

Rows use `fe2o3.async-copy-striped-benchmark.v2`. Each direction retains 30
raw submit, wait, and E2E nanosecond samples plus p50/p95 summaries and E2E p50
GB/s. The checker recomputes every summary and throughput value and requires
`e2e[i] == submit[i] + wait[i]`. KFD rows retain canonical `role:queue-id`
entries in `queue_ids` and matching `role:queue-id:engine-index` entries in
`engine_placement`. Their digests are SHA-256 over the exact ASCII field value.
The checker recomputes both digests, requires distinct u32 queue IDs, and
validates exact role order and alternating gfx942 engine placement. Combined
profiles order H2D engine 1, D2H engine 0, then striped queues 0 through q-1 on
alternating engines; standalone `striped16` retains only its 16 alternating
striped queues. KFD rows also require passing directional, aggregate-poll, and
destruction sentinels.

Each of the 90 backend/workload/slot phases runs under the R26 2 ms process-tree
queue monitor with a 10 ms maximum observation gap. Start/end telemetry,
topology, exact target output, process reaping, zero foreign/terminal selected
queues, and stable system identity are retained. KFD phases additionally
require zero GPU busy at the post boundary. A successful run atomically
publishes the three logs, validation report, exact Git source archive, file
manifest, and a separately SHA-256-sealed archive. Temporary and verification
trees use unique `fe2o3-r40-striped-*` paths and are removed by traps; the
runner never resets the GPU or signals foreign processes.

The pre-registered bounded comparison uses the median of the three paired
slotwise KFD/reference ratios. It reports parity only when median latency is at
most 1.10, median bandwidth is at least 0.90, and every slot latency ratio is
at most 1.20 for every workload, direction, and reference. Missing a threshold
does not discard structurally valid evidence; it reports
`bounded_parity_status=not-demonstrated`. Use checker option `--require-parity`
only when a gating job should reject that result. A 10x result is reported only
when every matched slot/workload/direction latency ratio is at most 0.10. This
is reported as `ten_x_status`; it is not described as orders of magnitude.

This scope is one host, one GPU, host-observed H2D/D2H, depth 112, and transfer
sizes no larger than one SDMA linear packet. Directional queues are idle during
the measured combined aggregate copy. The harness does not cover bidirectional
overlap, compute/copy overlap, larger windowing, other devices, or runtime-wide
HIP/HSA parity, and its output cannot support a generic speedup claim.

### Large Directional Window Qualification

Run the R22 public-facade large-copy comparison on an idle MI300X system:

```sh
benchmarks/runtime_gfx942/run-directional-window-mi300x.sh
```

The default is a 256 MiB H2D-then-D2H transfer with three warmups and ten
samples. `FE2O3_DIRECTIONAL_WINDOW_GPU_INDEX`,
`FE2O3_DIRECTIONAL_WINDOW_BYTES`, `FE2O3_DIRECTIONAL_WINDOW_WARMUPS`, and
`FE2O3_DIRECTIONAL_WINDOW_SAMPLES` select the measured device and controls.
The size is intentionally restricted to 264,239,137 through 268,435,456 bytes,
so every run crosses the 63-packet R22 window boundary. At 256 MiB, the facade
publishes 65 packets as windows of 63 and two packets; the last packet is 2,048
bytes. Each window uses one write-pointer publication and one doorbell.

The KFD interval starts before `copy_async` and ends only after facade wait plus
any explicit continuation flush has observed the complete transfer. Allocation,
pattern writes, readback validation, submission release, and teardown stay
outside the interval. The HSA and HIP comparators use the existing schema-V1
copy programs with the same device identity, byte length, direction ordering,
warmups, samples, and host-observed completion boundary. The runner requires a
clean checkout, tests the frozen R22 manifest, records the exact commit and
software stack, applies a default 5% load ceiling before and after each phase,
and deletes its unique temporary build directory on exit.

This is a depth-one large-transfer diagnostic. It does not establish
concurrency, peer-copy, H2H, D2D, hardware-refinement, or runtime-wide
performance parity. No result is admissible while the shared GPU is busy, and
no speedup claim may be made until raw output from the exact commit passes the
same qualification checks as the retained asynchronous-copy results.

### Same-Device D2D Window Qualification

Run the R23 public-facade D2D comparison on an idle MI300X system:

```sh
benchmarks/runtime_gfx942/run-d2d-window-mi300x.sh > d2d-window.txt
python3 benchmarks/runtime_gfx942/check-parity.py d2d-window.txt \
  --schema fe2o3.d2d-copy-benchmark.v1 \
  --max-latency-ratio 1.10 \
  --min-bandwidth-ratio 0.90
```

The thresholds above are illustrative and must be chosen as an explicit
release policy. A tenfold speedup policy would instead require a latency ratio
of at most `0.10` and/or a bandwidth ratio of at least `10.0`; a parity result
cannot be described as an order-of-magnitude result.

The default transfers 256 MiB at depth one with three warmups and ten samples.
`FE2O3_D2D_WINDOW_GPU_INDEX`, `FE2O3_D2D_WINDOW_BYTES`,
`FE2O3_D2D_WINDOW_WARMUPS`, and `FE2O3_D2D_WINDOW_SAMPLES` select the measured
device and controls. The size is restricted to 264,239,137 through 268,435,456
bytes so the KFD path crosses its 63-packet window boundary. At 256 MiB this is
65 packets in two windows, hence two write-pointer publications and two
doorbells per measured copy. `FE2O3_D2D_WINDOW_SECOND_GPU_INDEX` selects an
unused system-load sentinel; the runner requires a distinct second physical
identity and checks it again after all phases without presenting it as a D2D
participant.

All three lanes retain distinct source and destination device allocations.
Each round initializes the source and poisons the destination outside timing.
The measured interval starts before the D2D enqueue and ends after host-observed
completion: KFD uses `RuntimeContextV1::copy_async`, explicit `flush_stream`,
and facade wait; HSA uses `hsa_amd_memory_async_copy`; HIP uses
`hipMemcpyAsync` on one nonblocking stream. After the interval, every lane
copies both allocations back to host and verifies that the destination exactly
matches the source pattern and that the source was not modified.

The runner requires a clean checkout, `gfx942:xnack-`, the exact KFD device
identity, both frozen SDMA manifest digests, and a default 5% load ceiling at
each measured phase boundary. It deletes its unique temporary build tree on
every exit path. The checker requires the exact depth, packet/window/doorbell
accounting, validation and timing labels, phase observations, and matching
KFD/HSA/HIP device and statistical fields. A passing report applies only to
that commit, device, software stack, size, depth, and timing boundary. No R23
hardware result is currently retained, so this harness makes no correctness,
parity, or speedup claim.

Boundary load samples cannot detect unrelated work that starts and stops
wholly inside one phase. Continuous machine exclusivity remains an external
condition for release evidence.

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
publication, explicit batch flush, and observed completion. Its
`remap-per-round` diagnostic performs host writes before and readback after each
timed interval; those host accesses retire retained peer mappings, so the next
interval includes peer-map establishment. Mapping retirement and host
validation remain outside timing. The `persistent-hot` parity row primes one
mapped batch before timing, then performs no host access between timed rounds;
its intervals reuse those mappings and exclude both map establishment and
retirement. Final readback and canary validation occur after all timed hot
rounds. Both rows cover all depth submissions through observation of every
completion and report p50/p95 nanoseconds plus p50 aggregate GB/s. The checker
requires both rows for completeness but ratios only `persistent-hot` against
HSA and HIP, whose peer access and device allocations also persist across timed
rounds. Their per-round data preparation policies remain different and outside
the measured interval.

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
depth with one doorbell. Comparing it with the public facade's
`persistent-hot` row isolates remaining facade admission, completion, and
release overhead without conflating remapping; it is not a substitute for the
public API measurement.

This peer-copy harness does not exercise the separate structure-required
Worker V3 dispatch wrapper and makes no atomic or collective
performance claim. That wrapper admits only its finite integer-atomic,
LDS-primitive, and workgroup-barrier structure roster; all `_DPP` spellings
currently fail closed.

HSA/HIP results remain qualification/oracle evidence and do not identify
production backend alternatives.

## Persistent In-place Compute Qualification

R26 compares one exact persistent-buffer workload across direct KFD, raw HSA
AQL, and HIP module launch on one idle MI300X GPU:

```sh
mkdir -p /tmp/fe2o3-r26-evidence
FE2O3_R26_GPU_INDEX=6 \
FE2O3_R26_OUTPUT_DIR=/tmp/fe2o3-r26-evidence \
  benchmarks/runtime_gfx942/run-r26-inplace-mi300x.sh
```

Choose a physical GPU that has no existing KFD queues; the example index is
not a portable default. The runner uses one 1 MiB `u32` allocation, a
262,144-work-item grid, workgroups of 256, 10 warmups, and 30 untrimmed samples
of 10 iterations. Every iteration alternates one of two complete input images,
runs the SHA-pinned `inplace_transform` artifact, copies the result back, and
validates every element. H2D, compute, D2H, and enclosing E2E samples use the
same host-monotonic phase boundaries in all three implementations.

The KFD lane promotes the complete H2D result into the runtime's authenticated
persistent device allocation and launches compute against that same HBM
storage without a second user-data materialization. Its nested promotion
sample measures the full-H2D-to-compute-ready transition. The HSA and HIP
comparators retain one device allocation but report promotion as unavailable.
Setup, final validation, and release occur outside the enclosing E2E interval
for every backend.

The V4 KFD row records `control_path=persistent-control-replayed` only after
every measured launch confirms reuse of the retained dispatch control. It also
retains host-monotonic launch timings; the HSA and HIP comparators report the
KFD-specific control path as `n/a`. `preparation` is an
inclusive interval that encloses the `bound_snapshot` and `authority`
subintervals; their sum must not exceed it. The remaining persistent-launch
critical path is exclusive: `native_binding` stops before `publication`, which
is followed by `publish_to_completion` and `recycle_inclusive`.
`recycle_inclusive` must equal the overflow-checked sum of
`completion_signal_recycle` and `completion_detach_restore` for every sample;
the second component begins at the exact signal-recycle timing boundary, so it
also includes the handoff into detach. Both components must be nonzero on this
persistent path. The checker requires
the sum of `preparation` and the four exclusive critical-path intervals to fit
inside the inclusive compute sample. Component durations use the declared
integer average; `recycle_inclusive` is then derived from those two averaged
components so independent integer rounding cannot invalidate the equality.
`completed_readback` is exactly zero
because this persistent device path does no completed host readback. These
observations are host timings for diagnosis, not device-clock kernel durations.

One run contains three separate cyclic Latin-square slots. Each slot retains
30 samples per backend without cross-slot aggregation. The set checker emits
slot-qualified KFD/HSA and KFD/HIP p50 ratios for every phase, the KFD promotion
share, and `kfd-host-launch-timing` p50 observations, followed by a manifest
over all three exact slot hashes. These are descriptive, non-gating
comparisons; R26 rejects parity thresholds and makes no runtime-wide parity or
speedup claim.

Qualification runs in a minimal declared environment, builds Rust from a
private snapshot of the exact Git commit, seals its fixture, helpers, sources,
and binaries, and records start/end kernel, driver, ROCm, loader, GPU, PCI, KFD,
and NUMA identities. Each child first restores the topology-approved GPU-local
CPU mask with `taskset`, then `numactl` binds that same mask and the GPU-local
memory node. A separate CPU takes an immediate post-launch census, then samples
`/sys/class/kfd/kfd/proc` on absolute `CLOCK_MONOTONIC_RAW` deadlines every 2
ms. It rejects foreign selected-GPU queues or a gap above 10 ms and must observe
a target-owned queue before a row can be released. After the target is reaped,
its process group must be empty and a terminal census must find zero
selected-GPU queues. This is bounded sampled interference detection, not proof
that no queue existed between censuses or that a process cannot escape the
monitored process group.

The runner writes nothing durable on an incomplete or failed run. On success it
revalidates staged copies of all three logs, byte-compares the regenerated set
report, and atomically publishes one external evidence directory. The output
directory must be outside the checkout.
