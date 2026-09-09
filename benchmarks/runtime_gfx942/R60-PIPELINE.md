# R60 Ordered Pipeline Benchmark

This benchmark compares a bounded 64-launch ordinary vecadd batch on one
gfx942:xnack- device. It measures the public KFD runtime, HIP module launch,
and raw HSA AQL submission separately. It does not establish general HIP/HSA
parity, machine-code refinement, concurrent execution, or device-kernel speedup.

All three backends use the retained `trusted-gfx942-vecadd-v1/vecadd.hsaco`
(SHA-256 `3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab`),
1,048,576 `f32` elements, grid `[1048576,1,1]`, and workgroup `[256,1,1]`.
The same two read-only input handles and one write-only output handle select
coherent host-visible allocations for all 40 batches. Each input
element is initialized as `a[i] = f32(i % 1024) / 2` and
`b[i] = f32(i % 256) / 4`. The output is reset to `0x7fc00000` before every batch.
Each batch submits the same 64 ordered dispatches, then observes completion.
All output bytes and both unchanged inputs are checked after every batch.

The output SHA-256 is
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
Since all launches overwrite the same output, this hash verifies the final
result; execution counts additionally rely on accepted ordered submissions and
backend completion semantics. HSA checks all 64 distinct completion signals.
HIP checks completion of the nonblocking stream after 64 accepted module calls.

## Timing

Each backend preallocates application-owned buffers, launch arguments, queue
or stream, and timing storage. HSA also preallocates its completion signals;
KFD's per-dispatch resource setup remains inside issue timing. Explicit allocation API
calls, output reset, and validation are outside timing. Runtime-internal work
performed by a launch API is included in its issue cost. In particular, a KFD
host output write evicts the compute cache, so the next timed flush allocates
and materializes native bindings again. This benchmark does not assume steady
native residency across batches. Its `explicit_allocation_api_timed: false`
field does not exclude that implicit runtime work. There are 10 warmup batches and 30 measured
batches. A single monotonic host clock gives three timestamps:

- `issue_batch_ns`: first timestamp through publication of all 64 launches.
- `tail_wait_batch_ns`: return from issue through observed batch completion.
- `total_batch_ns`: complete interval, exactly the sum of the two components.

There is no explicit completion wait between issue calls. HSA uses a queue with
capacity for the entire batch, 64 preallocated signals, one timed signal rearm and packet publication
and one doorbell store per launch, system acquire/release fences, and the AQL
barrier bit. It checks queue capacity before timing without spinning for space.
HIP uses one `hipStreamNonBlocking` stream, the exact module bytes already
hashed in memory, and `hipHostMallocMapped | hipHostMallocCoherent` buffers.
KFD performs 64 public `launch` calls and 64 `flush_stream` calls inside issue
timing, because launch alone enqueues host work. Its
`issue_api_calls_per_batch` is therefore 128. HSA reports 192: each
publication calls `hsa_signal_store_screlease` to rearm its completion signal,
`hsa_queue_add_write_index_relaxed` to reserve its slot, and
`hsa_signal_store_screlease` for the doorbell. HIP reports its 64
`hipModuleLaunchKernel` calls.
The checker requires those backend-specific values and exposes them in the
comparison output. All backends execute the same 64 kernel launches.

HIP tail waiting polls `hipStreamQuery`; HSA polls the final completion signal
with acquire loads. The timed endpoint is that tail observation; independent
confirmation of all 64 HSA completion signals occurs afterward. Both check the monotonic
10-second batch deadline on unsuccessful polls and yield every 4096 polls.
The per-poll API cost differs and remains included in the tail measurement.
A backend API call itself cannot be preempted by this host deadline; the runner
must also enforce an external process timeout. Teardown finishes before any
JSONL evidence is emitted, so a failing cleanup cannot leave an accepted log.

## Building the Baselines

Both sources require C++17 and OpenSSL libcrypto development files. HIP and HSA
must come from the same installed ROCm release used by the final runner.

```sh
hipcc -O3 -std=c++17 -Wall -Wextra -Werror \
  benchmarks/runtime_gfx942/r60_pipeline_hip.cpp -lcrypto -o r60-pipeline-hip
c++ -O3 -std=c++17 -Wall -Wextra -Werror -I/opt/rocm/include \
  benchmarks/runtime_gfx942/r60_pipeline_hsa.cpp \
  -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 -lcrypto \
  -o r60-pipeline-hsa
```

Both binaries take:

```text
EXACT_HSACO VISIBLE_DEVICE_INDEX EXPECTED_UNIQUE_ID SOURCE_COMMIT RUN_ID
```

`SOURCE_COMMIT` is 40 lowercase hexadecimal digits, and `RUN_ID` is 64 lowercase
hexadecimal digits selected by the runner. The expected GPU unique ID is a
nonzero decimal or `0x`-prefixed integer. The selected visible GPU index must
match that device identity. Device visibility environment variables must be
controlled consistently by the runner. Host NUMA/CPU placement, interference
checks, source/build provenance, backend ordering, and remote cleanup are also
runner responsibilities. The log checker verifies matching record claims; it
does not authenticate a source commit or prove a machine was idle.

## Evidence Contract

Each backend emits exactly 42 newline-terminated JSON objects to stdout. Every
field is mandatory, and unknown or duplicate fields are rejected. The complete
fixed configuration is declared in `check-r60-pipeline.py::FIXED_CONFIG`.
The first record extends that configuration with `backend` (`kfd`, `hip`, or
`hsa`), `run_id`, `source_commit`, and the actual checked `unique_id` formatted
as 16 lowercase hexadecimal digits without `0x`.

The next 40 records contain `record: "batch"`, `phase: "warmup"` for indices
0 through 9, then `phase: "sample"` for indices 0 through 29. Every record also
contains `issued_launches: 64`, `completed_launches: 64`, positive integer
`issue_batch_ns`, `tail_wait_batch_ns`, and `total_batch_ns` no greater than
10,000,000,000, and the exact `output_sha256` given above. Output validation
must succeed before accepting a record. The last record is exactly
`{"record":"complete","validated_batches":40}`.

```sh
python3 benchmarks/runtime_gfx942/test_check_r60_pipeline.py
python3 benchmarks/runtime_gfx942/check-r60-pipeline.py \
  kfd.jsonl hip.jsonl hsa.jsonl
```

The checker rejects mismatched source, run, device, artifact, shape, memory,
ordering, sample count, output, or completion count. Warmups are validated but
excluded from statistics. It emits nearest-rank p50 and p95 for each metric
and `baseline_over_kfd` ratios separately for HIP and HSA. A ratio greater than
one means KFD has the shorter interval for that metric in this matched run.
Host issue ratios describe host submission cost; only total ratios describe
end-to-end batch latency. Tail ratios also depend on how much work completed
during issue and cannot by themselves support a kernel-performance claim.

## Guarded MI300X Runner

`run-r60-pipeline-mi300x.py` binds ROCm GPU index 1 to unique ID
`0xab83d2ffef0d3cdf`. It obtains the PCI BDF from the retained system identity
collector and verifies that identity against PCI sysfs and KFD topology. DRM
card numbering is not used to select a device.

The source must be a clean Git checkout containing an SSH-signed commit. On a
remote machine, a private checkout reconstructed from a Git bundle retains
the objects needed to verify that signature. A source tar alone is not an
accepted input. Supply an independently trusted SSH allowed-signers file;
the runner invokes `/usr/bin/ssh-keygen` through Git verification and never
reads a private signing key.

```sh
python3 benchmarks/runtime_gfx942/test_run_r60_pipeline.py
python3 benchmarks/runtime_gfx942/run-r60-pipeline-mi300x.py \
  --repo /path/to/clean-signed-checkout \
  --allowed-signers /path/to/trusted-allowed-signers \
  --output-dir /path/to/existing-external-evidence-directory
```

The runner creates a mode-700 temporary directory outside the checkout. It
archives the verified commit, authenticates the invoked runner against that
archive, removes write permission from the extracted source, and hashes every
source file. Rust builds use `--offline --locked --release`, four build jobs,
and a private target and temporary directory. Required Rust dependencies and
the pinned toolchain must already be available in the selected build home's
cache. User-level Cargo configuration is rejected. Both baselines use strict
C++17 release builds with the selected ROCm installation and libcrypto.

Two guarded KFD qualification runs precede measurement. Each must emit the
exact PASS marker and a complete native runtime profile with all 64 dispatches
published before the first completion, completions in publication order, and
zero native binding cost for successors. This is evidence of ordered host
publication; it does not assert simultaneous device-kernel execution.

Three measured triples use the orders KFD/HSA/HIP, HSA/HIP/KFD, and
HIP/KFD/HSA. Each triple has a distinct run ID shared across its three backend
logs. Source, workload, device, and output records must match before any
comparison is accepted. The existing host guard observes selected-GPU queue
ownership throughout every qualifier and benchmark process, with a 2 ms
cadence and 10 ms maximum observation gap. An interference or census failure
aborts the entire set without publishing partial results. Work on other GPUs
is allowed; it is not terminated. CPU/NUMA placement is selected from local
allowed resources, with a separate observer CPU. Idle load, clock, and power
telemetry and unchanged topology are checked at every phase boundary.

Execution uses a cleared environment with explicit locale, path, and backend
visibility variables. Every GPU process has a 180-second external timeout;
the guard verifies target reaping, process-group absence, and no remaining
selected-GPU queue after completion. The runner supervises its own command
groups, adopts orphan descendants for reaping, and allows 15 seconds for
graceful guard cleanup before escalating its own group. Build and helper
commands also have explicit time and output bounds.

Only after all three comparisons, both profiles, unchanged start/end system
identity, and source/binary rechecks pass does the runner copy evidence into
a new private directory under the caller's output directory. That directory
contains the signed commit, source tar, trusted signer file, source and binary
hashes, binaries, command/environment records, raw profiles, all benchmark
logs, comparisons, telemetry, topology, guard records, and a hash manifest.
The copied manifest is verified before atomic publication. Temporary build
and publication directories are removed on success or failure. The caller
owns the requested final evidence directory and any remote checkout used to
invoke the runner.
