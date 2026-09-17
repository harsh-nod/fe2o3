# HSA Pool And Engine Diagnostic

`async_copy_hsa_pool_engine.cpp` is a development diagnostic for the unresolved
[256 MiB directional copy gap](../../docs/evidence/dev-directional-copy-mi300x-2026-09-17/README.md).
It does not change `async_copy_hsa.cpp`, the KFD/HIP comparators, their runners,
or any runtime implementation. Its distinct schema is not a parity acceptance
record. No native performance or coherence result follows from CPU tests.

## Build And Controls

Build in an owned temporary directory with ROCm development headers/libraries:

```sh
g++ -std=c++17 -O2 -Wall -Wextra -Werror -pedantic \
  -isystem /opt/rocm/include \
  benchmarks/runtime_gfx942/async_copy_hsa_pool_engine.cpp \
  -L/opt/rocm/lib -Wl,-rpath,/opt/rocm/lib -lhsa-runtime64 \
  -o "$work/hsa-copy-pool-engine"
```

Do not link `hsa_copy_diagnostic_mock.cpp` into a hardware executable. That file
implements a CPU test double and never establishes a native result.

The executable takes eight explicit arguments:

```text
hsa-copy-pool-engine <gpu-index> <cpu-index> <bytes> <warmups> <samples> <expected-unique-id> <fine|coarse> <engine0|engine1>
```

Depth is exactly one; size is 1 through 268,435,456 bytes, samples are nonzero,
and warmups plus samples cannot exceed 10,000. Invalid CLI input is rejected
before HSA initialization. GPU UUID, exact `gfx942` target and disabled XNACK
must match. The CPU index is an explicitly selected enumerated CPU agent, not
a Linux CPU or NUMA node number. The nearest-CPU handle, driver nodes and GPU
PCI identifiers are observations, not automatic placement changes.

The host pool must be the unique eligible CPU-location pool with exactly
fine-grained flags (2) or coarse-grained flags (4). Kernarg, mixed and extended
flags do not match. The device pool is the unique eligible coarse-grained
GPU-location pool. Allocation flags remain zero. Both agents must be eligible
for access and are explicitly granted access to all three buffers. Admission
checks granule-rounded aggregate capacity for two host allocations and one
device allocation; reported maximum capacity is not free-memory telemetry or
a reservation. Missing, ambiguous or unavailable choices fail without fallback.

Engine 0 and engine 1 mean HSA masks 1 and 2. Each must be advertised for both
H2D and D2H by separate `hsa_amd_memory_copy_engine_status` queries. All copies
use `hsa_amd_memory_async_copy_on_engine` serially, with no automatic-engine API
and `force_copy_on_sdma=false`. That flag's documented special case concerns
same-agent blits, which this diagnostic does not perform. HSA masks must **not**
be identified with KFD queue indices or treated as measured physical placement.
The [ROCr API contract](https://rocm.docs.amd.com/projects/ROCR-Runtime/en/latest/api-reference/api.html)
requires engine availability checks, direct access by both agents and system
release/acquire ordering around DMA buffers.

## Measurement

Each round initializes the retained upload buffer with a changing uniform
pattern and poisons the retained download buffer. An idle-signal `screlease`
after both writes establishes the host release boundary before timed H2D.
Each direction records host submission duration, blocked `scacquire` wait plus
signal reset duration, and their total. Only exact signal zero permits reset
and the next operation. Negative errors or pending/timeout observations fail
without resetting the signal or reusing/freeing the buffers in application code;
fatal native errors terminate the diagnostic process.

H2D completes before D2H. Every returned byte is checked after every warmup and
sample. Setup, host preparation, host release, validation and cleanup are outside
copy timing. The signal reset remains inside timing, as in the existing HSA
comparator. The additional intermediate clock observation is diagnostic overhead.
There is no separate allocation/free stress probe. These are host intervals, not
device timestamps, and they do not isolate sleeping or physical DMA duration.

`record=pool` describes all enumerated GLOBAL pools, including ineligible ones;
non-GLOBAL pools are skipped before reading GLOBAL-only attributes. `record=config`
records admitted choices and extents. Neither is a successful benchmark result.
`record=round` retains every warmup/sample's timings and validation count, followed
by `record=complete`, only after signal destruction, all frees and shutdown succeed.
The process must also exit zero; output errors are fatal. Truncated output, missing
completion, failed postflight or nonzero exit cannot be accepted as a measurement.

## Experiment And Limits

On an actually free device, compare fine/engine0, fine/engine1, coarse/engine0,
and coarse/engine1 at 256 MiB, three warmups and ten samples. Hold CPU agent,
device pool, NUMA policy, wait policy and preparation fixed. Use counterbalanced
cell order and retain per-process records rather than pooling process percentiles.
An unavailable cell is unavailable, not permission to change its policy.

Before and after every process, an external shared-host guard must check the
exact UID/BDF, utilization, VRAM and mapped PIDs. This executable does not reserve
a GPU or implement that guard. Do not run it merely because utilization is zero
when another process holds allocations. Stop on admission/postflight failure and
remove only owned artifacts after all owned processes have exited.

The matrix can establish effects of HSA pool policy and requested engine within
this diagnostic. It cannot establish physical NUMA residency/cache attributes,
equivalence to KFD GTT mappings, physical engine identity, or the cause of the
facade gap. The KFD facade also repeats short currentness-checked waits and performs
substantial outside-timer shadow preparation; those are separate control variables.
Automatic HSA engine policy and device profiling are not exercised here.

## CPU Qualification

```sh
python3 -I benchmarks/runtime_gfx942/test_hsa_copy_diagnostic.py -v
```

The pure policy test has no HSA dependency. Adapter tests compile the actual
translation unit against the separately named CPU mock and real ROCm headers,
without linking ROCr or opening a GPU. They cover all explicit cells and CPU
choices, exact selected/allocated pools, aggregate bounds, access grants, host
release placement, directional availability, serial completion/reset, full-buffer
failures, cleanup failures and early/late output errors. Adapter tests explicitly
skip when headers are missing; a skip is not adapter qualification. Set `ROCM_PATH`
to select another header installation. Mocks verify control flow, not hardware
coherence, memory attributes, DMA execution or performance.

The [CPU qualification receipt](../../docs/evidence/dev-hsa-pool-engine-cpu-2026-09-17/README.md)
records the actual source identity and check outcomes. Hardware execution remains
a separate guarded measurement, not an implication of that receipt.
