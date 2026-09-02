# fe2o3-hsa-runtime

> **Deprecated qualification surface.** Production fe2o3 applications use the
> direct-KFD runtime. HSA and HIP are not fallback execution paths.

The default crate is an inert compatibility marker. It exports
`HSA_RUNTIME_AVAILABLE = false`, does not expose an HSA adapter, does not depend
on `fe2o3-host` or `fe2o3-core`, and never probes the build host for ROCm.

Feature `qualification-legacy-hsa-runtime` restores the former reviewed HSA/HIP
API for compatibility testing during its deprecation window. That surface
contains a `RuntimeBackendV1` implementation and the profile-specific Worker V2
adapter. Both load already-finalized AMDHSA code objects and submit AQL directly.
They do not compile or link code and have no COMGR or command-line tool path.

## Deprecated Qualification Backend

The deterministic non-native qualification backend is selected explicitly with:

```text
FE2O3_HIP_SYS_DISABLE=1 cargo test -p fe2o3-hsa-runtime \
  --features qualification-legacy-hsa-runtime
```

Native HSA support must be selected explicitly with Cargo feature `native-hsa`.
That feature implies `qualification-legacy-hsa-runtime`:

```text
cargo build -p fe2o3-hsa-runtime --features native-hsa
```

When `native-hsa` is selected, the build requires matching HSA and HIP headers plus
`libhsa-runtime64` and `libamdhip64`. The ROCm root is taken from `ROCM_PATH`,
then `HIP_PATH`, and otherwise the unversioned `/opt/rocm` location. Both `lib`
and `lib64` layouts are accepted. An incomplete explicitly selected backend is
a build error that identifies the missing prerequisites; it never silently
falls back to the stub. Feature `hardware-test-hooks` implies `native-hsa`.

`ReviewedHsaRuntimeBackendV1` implements the backend SPI from `fe2o3-runtime`
for the one HIP-correlated reviewed HSA device. It exposes multiple persistent
streams, host-visible allocations, sealed module and kernel identities, typed
address-free pointer patching, nonblocking submission and polling, deadline
waits, and cross-stream events. Device-local allocation, peer access,
multi-device operation, and backend-owned atomics or collectives are reported
as unsupported rather than emulated. Queue admission is nonblocking: a full
ring or competing producer is rejected before packet reservation.

Constructing this direct in-process backend is `unsafe`. The caller must ensure
for its complete lifetime that every subsequently loaded code object is trusted
for execution in the application GPU VM, every typed signature matches the
exact kernarg ABI, and declared binding regions conservatively cover all memory
effects. Worker supervision contains backend aborts and timeouts, but its public
handshake does not authenticate an executable or make untrusted code safe;
callers still own worker selection, artifact authority, and OS isolation.
Completion signals and kernarg allocations remain owned by sealed submission
records until explicit submission release. Event dependencies retain their
source signal; cross-stream conflicting regions are rejected unless ordered by
a transitive event dependency. Host reads and writes are rejected while they
conflict with pending device access.

The feature-gated native packet tests exercise the exact production reservation
and AQL materialization helpers against an aligned fake ring:

```text
cargo test -p fe2o3-hsa-runtime --features hardware-test-hooks --lib native_packet
```

This lane requires the ROCm HSA/HIP development headers and shared libraries
described above, but it does not require a GPU or initialize HSA. It verifies
barrier-AND fan-in above five signals, ring wrap, bounded capacity rejection,
competing-producer rejection, and pre-publication no-mutation behavior. It is
not kernel-execution evidence.

Feature `hardware-qualification` adds the repository-owned, source/policy/object
digest-pinned gfx942 vecadd fixture. Its ignored hardware lane revalidates the
COV6 target, kernel closure, explicit ABI, and metadata-declared
read/read/write effects before retaining that admission across an unsafe
`ReviewedHsaRuntimeBackendV1` and the complete `RuntimeContextV1` lifecycle. The lane verifies exact results,
explicit teardown, a real six-event cross-stream fan-in, and more than 64
sequential submissions through the reviewed 64-slot queue. It also reports
host `host_visible_submit_wait_readback` and `synchronized_launch_wait` timing
with 10 warmups, 30 samples, and 10 launches per sample by default. Both use
persistent initialized host-visible allocations and exclude the exact output
reset before each launch. The first timer includes submit, deadline wait,
submission release, and full output readback; the second includes only submit,
deadline wait, and submission release, with readback and exact validation after
each sample. These boundaries differ from backends that stage inputs on every
submit. The corresponding `FE2O3_RUNTIME_WARMUPS`, `FE2O3_RUNTIME_SAMPLES`, and
`FE2O3_RUNTIME_LAUNCHES_PER_SAMPLE` variables accept positive overrides while
still requiring enough launches to cover ring wrap.

```text
HIP_VISIBLE_DEVICES=0 \
ROCR_VISIBLE_DEVICES=<physical-gpu-index> \
FE2O3_RUN_GFX942_RUNTIME_HSA_QUALIFICATION=1 \
cargo test --release --locked -p fe2o3-hsa-runtime \
  --features hardware-qualification \
  --test gfx942_runtime_context_hardware \
  qualification::gfx942_runtime_context_exact_fixture_executes_dependencies_wraps_and_times \
  -- --ignored --exact --nocapture --test-threads=1
```

`ROCR_VISIBLE_DEVICES` selects one decimal physical-device index;
`HIP_VISIBLE_DEVICES=0` selects that device's post-isolation HIP ordinal. The
process then opens visible ordinal zero and requires exactly one correlated
runtime device. The lane rejects debug builds so its host timings remain
comparable to the release-mode KFD and HIP runners. This bounded lane does not
promote the fixture into general launch authority. Live capacity
exhaustion/retry is not asserted because the qualified kernel cannot safely be
held pending; deterministic capacity and no-mutation behavior remain covered
by the native packet tests.

Facade waits use nonblocking acquire polls with a monotonic caller deadline:
32 short spins, 8 scheduler yields, then exponential sleeps starting at 50
microseconds and capped at 1 millisecond. Deadline expiry returns `Pending` and
retains all submission authority. A live queue fault is terminal, while a fault
observed only after exact-zero completion is reported as a failed quiescent
submission and seals that stream against further publication.

Release events retaining a completed submission before releasing its submission
record, then destroy streams and explicitly shut down the owning runtime
context. Dropping this direct backend with live or ambiguous custody aborts.

## Legacy Worker V2 Adapter

Construction locates the HSA and HIP runtime images through `/proc/self/maps`,
opens those paths, verifies that each opened file has the mapped device and
inode, checks stable metadata around hashing, and binds both file digests into
the runtime identity. The descriptors are closed after measurement. This is
path/device/inode evidence for the runtime files at observation time, not
authentication of the executable pages already mapped into the process. In
particular, a same-UID actor that can mutate a runtime file in place after the
observation is outside this model and may invalidate the evidence. This is also
not an operating-system attestation or rollback anchor.

The generic reviewed COV6 initializer accepts a bounded profile-supplied
explicit prefix followed immediately by the exact 256-byte implicit-argument
layout and requires zero dynamic LDS. The workgroup-sync specialization instead
requires exactly 256 dynamic LDS bytes for the LDS reduction profile and zero
for the scoped-atomic profile. The reviewed layout initializes block counts,
group sizes, zero remainders, zero global offsets, grid rank, and the exact
`hsa_queue_t *` 200 bytes into the implicit suffix. Queue creation therefore
happens during hidden-argument initialization; the adapter privately retains
that queue and binds it to the executable, kernel, geometry, and a digest of
the complete kernarg bytes until the immediately following launch consumes it.

The reviewed COV6 metadata profile has no hidden dispatch-pointer kernarg. The AQL
packet supplies dispatch identity through the AMDHSA dispatch mechanism. Its
hostcall, multigrid synchronization, heap, default device queue, and completion
action fields are null because the current reviewed profiles use none of those
services. A profile that requires any of them must add authenticated runtime
observations and a separate reviewed initializer; this adapter fails closed on
every other hidden-argument layout, unsupported dynamic LDS value, missing
queue binding, or binding substitution.

One loaded executable may resolve a fixed, const-generic set of distinct kernel
symbols. The non-clone set owns each kernel token and borrows the executable,
so safe Rust must release the whole set before unloading that executable. It
rejects duplicate requested names and native symbol, kernel-object, or derived
kernel-identity aliases. This is lifecycle and identity authority only: it does
not claim that arbitrary resolved kernels have a typed ABI. Safe dispatch
remains profile-specific: host-side typed lifecycles validate their own ABI and
resources before using this adapter. Resource-observation implementations exist
for the exact workgroup-sync, row-softmax, FlashAttention, MoE top-2,
Wave64-collectives, and LDS-GEMM profiles. Their configured hardware tests
remain ignored under their explicit worker, artifact, wrapper, receipt, target,
or MI300X prerequisites; those lanes do not establish general gfx942 support or
GPU execution for every profile. Each prepared queue is bound to one executable
identity, one kernel identity, one geometry, and one digest of the complete
kernarg bytes.

Dispatch has explicit pre-submit, submitted, and quiesced states. Packet
publication atomically releases the combined 32-bit AQL header/setup word.
After publication, completion is polled with nonblocking acquire signal loads,
asynchronous queue status checks, and a five-second host monotonic deadline.
No HSA wait call can extend that deadline. Only an exact zero completion value
establishes quiescence. A queue fault or expired deadline terminates the process
with `SIGABRT`, because returning would release caller authority while the GPU
may still reference its allocations. Errors return only on definite
pre-publication paths or after quiescence, where native queue, signal, and
kernarg resources can be cleaned in reverse order.

The adapter is `Send` so a unique owner may move it between threads, but it is
intentionally not `Sync`. Native ABI record sizes and alignments are asserted in
both C and Rust. HSA queue ID zero is accepted as valid; queue validity depends
on the returned queue pointer, size, and callback state. If a malformed queue is
returned after successful creation and its destruction fails, the callback and
queue record remain allocated and adapter construction terminates rather than
freeing callback state that a live queue could still invoke.
