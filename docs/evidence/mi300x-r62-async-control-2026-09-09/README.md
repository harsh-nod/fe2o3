# R62 Async Control Qualification

## Scope and Result

Two guarded direct-KFD copy runs passed on MI300X GPU1. The profile validates
pre-submission cancellation, recovery of the exact timed-out operation, progress
after observer Drop, complete H2D/D2H input/output/padding bytes, and explicit
native cleanup. It does not qualify generated kernels, a wall-clock timer,
performance, multi-GPU placement, or distributed execution. Issue #182 remains open.

Hardware-qualified source: `d3e3343d9b0594d86caf31833d02e80955786588`.
The subsequent test-only correction requests supported device-local memory from
the mock backend; production, example, and runner bytes are unchanged. The first
GNU suite failure is retained as `raw/host/fe2o3-r62-all-tests.log` rather than
being presented as a passing run.

## Host and Custody

- Selected GPU1 UID `0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`, KFD ID `23018`.
- NUMA node 0; placement CPUs `0-47`; observer CPU `95`.
- GPU0's foreign 44% VRAM allocation was left untouched. Only idle GPU1 was used.
- The test-only bounded owner gate forces both cancellation and timeout before
  context submission. Cancellation is not tested after native publication.
- A cancelled full-buffer copy would corrupt padding. The actual body copy
  preserves all 128-byte prefix/suffix canaries; full readback checks 1,048,832
  bytes in each input/output allocation.
- Both queue censuses are clean: maximum gaps 5,844 and 5,289 microseconds against
  a 10,000-microsecond gate, zero foreign/terminal queues, targets reaped and
  process groups absent. These intervals are not latency measurements.
- Cleanup removed `/dev/shm/fe2o3-r62-owner.YwG8nLE0`. The independent final SSH
  check confirms that directory and target PIDs are absent, and GPU1 VRAM is 0%.

## Build and Integrity

The profile retains the existing static `x86_64-unknown-linux-musl` target,
repository-pinned nightly, explicit compiler/BFD link invocation, locked offline
two-job build, Cargo closure audit, full ELF/symbol audit, and rechecked tool and
target-library identities. Both shared R61 and outer R62 runner sources must
match the signed source archive. No compatibility closure gate was relaxed.

The admitted binary is 4,300,248 bytes with 4,815 full symbol names, no undefined
symbols, no interpreter or dynamic dependencies, and no GNU minstack lookup.
Its SHA256 is
`6ff30acd3c85c1616e8817a2a9a390f713f785627b049d5ea4af38d208061f31`.

The independent capture audit verifies all 73 original manifest payloads, all
32 command return codes, two exact PASS lines, the signature against the
separately retained trusted signer file, and exact byte equality with a fresh
local Git archive of all 6,330 source files. See `raw/host/capture-audit.json` and
its retained read-only audit script. The trusted signer fingerprint is
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`.

The full local capture is
`/home/harsh/.codex-tmp/fe2o3-r62-hardware-d3e3343d.tar`, SHA256
`005f7f86f2cc64585653855fc3de39dfd68abe1b1ed3d0d8b268f1371fc9d9d1`.
The repository retains compact raw evidence, omitting only `source.tar` and
`owner-binary` from the accepted capture. Its original `sha256.json` is unchanged;
it intentionally still lists those two omitted payloads. `raw/retained-files.sha256`
separately authenticates every repository-retained raw file.

## CPU and Proof Validation

- GNU: 1,846 passed, 5 ignored across 40 all-target harnesses; 60 doctests passed.
- Musl: 1,846 passed, 5 ignored across 40 all-target harnesses; 60 doctests passed.
- Strict all-feature/all-target Clippy and workspace formatting checks passed.
- 28 R60/R61/R62 runner tests passed, including cross-profile result rejection,
  imported-source authentication, static closure checks, and cleanup failures.
- Authenticated Verus: 51 positive sources, 1,276 obligations, and 600 required
  negative rejections. All source/runner/checker/transcript pins and the
  190-file Verus toolchain closure matched.

R62 adds 17 runtime tests and two finite Rust model tests, including actual
registry cancellation/retirement, 2,048 tracked operations across four streams,
out-of-order observation, dropped and timed-out observers, start/cancel races,
forced completion during timer polling, rejected poll retention, stop/panic,
opaque identity, and reentrant/panicking waker disposal.

Only the eight R62 abstract host-control properties are newly proved. Finite
Rust table checking and reviewed model correspondence do not establish
executable refinement; CAS/order remains contracted, actual thread/future and
native behavior remains tested. The limitations and full open acceptance gates
are in [the control contract](../../runtime-async-control-v1.md).
