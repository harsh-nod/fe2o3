# HSA Pool/Engine Diagnostic CPU Qualification

CPU qualification completed on 2026-09-17, from 20:31:47 to 20:41:33 UTC.
All twenty command records are closed: the preserved initial benchmark-suite
record exited 1; the other nineteen exited 0. The corrected full suite passed.
`SHA256SUMS` seals the archive. No native execution or performance result is claimed.

Base: `5ed58ea896e492f5bbc135e9f8449eabd51165bd`. Seven changed source/documentation
files and the unchanged shared argument parser are captured. A supplemental
two-test-file patch fixes the fixture races exposed by the initial suite. The implementation
and its limitations are described in
[HSA-COPY-DIAGNOSTIC.md](../../../benchmarks/runtime_gfx942/HSA-COPY-DIAGNOSTIC.md).

This adds a separate diagnostic, not a runtime feature or optimization. The
existing KFD, HSA and HIP benchmarks, all runtime/proof sources and accepted
checkpoints remain unchanged. A1/A2, #182 and the copy-performance gap remain open.

CPU mocks exercise actual adapter control flow but do not execute DMA or prove
native coherence, physical engine identity, memory attributes or performance.
The real ROCr executable is compiled/linked, inspected and removed without being
run. No remote build or benchmark artifacts are created by this qualification.
The final MI300X command is a read-only occupancy observation, not a reservation
or native test. Tool/header/library hashes identify selected inputs, not a full
compiler, loader or operating-system closure.

## Source Identity

- Eight-file diagnostic/input manifest:
  `e76f7e1783b2baf50a5bcbfe0ffbf0daac6e97cfe376220b3bd91c011f64fcf4`.
- Seven-file diagnostic patch:
  `ae7c39b737353b00fc358efa3e8c687af7ea60229f3a2e8ecd52e22e3c8a2aa4`.
- Two-file supplemental fixture manifest:
  `37e9e212dabb870be31b99fdd16022dd27ed6b41cc51bfeaa20337f1ea6410a6`.
- Supplemental fixture patch:
  `2b48533bd5bf15dfda821bb95e45c3cf6f1081f6285027d7229d0af197ff9a73`.

The initial eight source hashes remained unchanged throughout qualification.
The two fixture hashes matched before and after the successor run. The seal
checks both exact patches, all nine changed paths, staged/disk agreement, ordered
records and selected tool/header/library identities. `qualify.sh` is the original
aborted run; `finish.sh` is the explicit successor, not an overwritten rerun.

## Results

- Ten focused diagnostic test groups passed without skips. They include eight
  CPU/grain/engine cells, pure policy cases, real-adapter mock failures and exact
  buffer/pool/access/release/cleanup/output assertions.
- The changed host-guard module passed all 75 tests, and the pipeline-runner
  module passed all 16 tests.
- The complete corrected benchmark suite passed all 305 tests without skips.
- The actual adapter compiled with strict C++ warnings against real ROCm headers
  and linked to `libhsa-runtime64.so.1`. Its dynamic dependencies and binary hash
  are recorded. It was never run; the owned temporary executable/directory were
  removed by the build helper's successful exit cleanup.
- Python lint/format and C++ format checks passed. Runtime/proof sources,
  comparator sources, the directional runner and both production process guards
  remained byte-unchanged. No new formal runtime claim is made or required by
  these diagnostic/test-only changes.
- Final source, fixture and selected tool/header/library identity checks passed.

## Preserved Failure And Correction

The initial 305-test run had one failure and two errors. One gate-order test used
wall time and exceeded the unchanged production 10 ms census bound by 0.341 ms.
The other two fixtures assumed child readiness within fixed 0.2/2-second delays,
and observed empty PID data. Its subprocess/resource warnings remain in the raw
receipt; this unsuccessful run is not relabeled as qualification success.

Only two test files changed afterward. The gate-order test now uses the same
injected clock as existing deterministic fixtures; dedicated cadence-rejection
tests retain the exact production bound. The interruption fixture waits for a
complete PID line before injecting the same interruption. The termination fixture
installs SIGTERM ignore before atomically publishing readiness, waits on readiness
or monitor exit with a bounded watchdog, and always reaps the monitor/closes pipes.
It records the cleanup oracle before test-only fallback cleanup, so the fallback
cannot mask a production cleanup failure. Production guard/runner behavior and
acceptance thresholds were not changed.

## Shared Host

The final read-only observation reports approximately 56.6-91.5 GB of allocated
VRAM on every MI300X GPU. GPU 0's zero utilization was not treated as availability.
No GPU workload was started, no remote build directory was created, and no
foreign process or file was changed. Hardware matrix execution remains pending
an actually free device and the external UID/BDF/utilization/VRAM/PID guard.
