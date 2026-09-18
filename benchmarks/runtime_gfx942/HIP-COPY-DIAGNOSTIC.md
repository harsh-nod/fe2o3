# HIP Copy-Only Comparator

`async_copy_hip.cpp` accepts the optional `diagnostic-copy-only` mode for
directional-copy comparisons. The original six-argument command and its single
`fe2o3.async-copy-benchmark.v1` summary remain available, including the separate
10,000-pair allocator benchmark. Neither path is part of the production direct-KFD
dependency closure.

```sh
hipcc -std=c++17 -O3 -Wall -Wextra -Werror \
  benchmarks/runtime_gfx942/async_copy_hip.cpp -o "$owned/async-copy-hip"
"$owned/async-copy-hip" 0 268435456 1 3 10 "$unique_id" diagnostic-copy-only
```

Select the visible device explicitly and bind its expected unique ID. Admission
requires gfx942 with an exact disabled-XNACK feature token. Unknown modes,
excess arguments, invalid arithmetic, zero samples, depth other than one, more
than 256 MiB, or more than 10,000 combined rounds reject before HIP device
selection. Run only with fresh identity/occupancy observations, an external
deadline, a core-size limit, CPU/NUMA binding and post-run checks. These are
shared-host precautions, not an exclusive GPU reservation.

## Measured Work

The diagnostic uses one nonblocking HIP stream, two default `hipHostMalloc`
buffers and one `hipMalloc` device buffer. Every round writes a changing upload
pattern and poisons the destination, then performs H2D and D2H in order. Each
host timer covers `hipMemcpyAsync` through `hipStreamSynchronize`, not physical
DMA time. Preparation, validation and resource release are outside the timers.
The diagnostic does not call `hipMallocAsync` or `hipFreeAsync` and does not
report allocator timing.

HIP chooses its copy route and engine. Host allocation flags, mapping choices,
stream synchronization and internal runtime policy are not assumed to match
KFD or an explicitly selected HSA pool/engine. Compare end-to-end host operation
intervals only, under a predeclared matched protocol. A runtime-chosen HIP
engine is not evidence of physical engine equivalence.

## Output Contract

Only after all rounds pass full-buffer validation, all three allocations are
explicitly freed, and the stream is destroyed does the executable emit:

- One config row in `fe2o3.hip-directional-copy-diagnostic.v1`, identifying the
  target, workload, host-allocation policy and runtime-selected engine.
- One indexed row for every warmup and sample, including the expected pattern,
  full checked byte count and positive H2D/D2H nanosecond intervals.
- One completion row with exact validated/measured counts and release counts.

Acceptance requires the complete row roster, zero executable status, and the
protocol's successful post-observations. An output flush failure is nonzero even
after successful resource release. A failed copy or synchronization exits
without reusing or explicitly freeing possibly live storage; external isolated
process teardown and guards remain required. No successful row is emitted for
allocation, submission, validation or cleanup failures.

## CPU Tests

```sh
python3 -B -m unittest discover -s benchmarks/runtime_gfx942 \
  -p test_hip_copy_diagnostic.py -v
```

These tests compile the actual comparator against a CPU-only HIP mock using
installed ROCm headers. The mock delays copying until synchronization, checks
resource roles, changing patterns and destination poisoning, and injects
selection, identity, allocation, submission, wait, corruption, cleanup and
output failures. They also check the maximum round count, the exact output
roster, and preservation of the legacy allocator exercise. The mock must never
be linked into hardware runs. Missing headers cause an explicit test skip;
CPU success does not establish native HIP behavior or performance.
