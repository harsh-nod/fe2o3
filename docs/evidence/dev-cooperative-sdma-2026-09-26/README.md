# Resumable Cooperative SDMA Leaves

Date: 2026-09-26. Development CPU evidence only. A1/A2, accepted milestones,
native qualification, formal refinement and HIP/HSA parity remain incomplete.

## Implementation

Signed implementation: `93ef769c0a15b7dc64c477b93b122fbbeb6040c3`.
Parent: `06a1b95246bf96a2bd2eeacf067b03282cdbd6b0`.

The multi-device router now uses real child async SDMA submissions for
cooperative DeviceLocal transfers whose DMA backing is authoritative. Each
Read or Write phase owns a private HostVisible allocation and logical stream,
reuses at most 64 KiB of scratch across chunks, and retains exact child
submission custody through observation and retirement. The full source is
captured before any destination write. Public poll/wait do not drive progress.
One flush may return Pending; subsequent phases require drain or more flushes.

Scratch is reserved in the router's aggregate staging budget. Required request
mode authenticates and charges the exact child's composed account before
backing effects, distinguishes rejection from settled-empty disposal, and
quarantines uncertain custody. Private handles never enter public routing maps.
Cooperative records are fallibly boxed before admission, avoiding enlargement
of every native submission slot.

Ordered successors remain admissible during a predecessor's private DMA, but
every conflicting native owner must match that exact allowed predecessor.
Published D2H/H2D cannot be cancelled or lose their buffers. Pending observations
and unchanged Ready heads do not advance the progress generation.

Recovered scratch-disposal failure freezes the copy as Failed. A selected
dependent chain is settled completely, including intermediate nodes. Quiescent
private scratch and its charge remain on the completed record; submission
release retries disposal without resuming writes. Terminal failure or unwind
seals the router, retaining potentially live custody and the original panic.

## Qualification

| Check | Result |
| --- | --- |
| Final focused transfer groups | 12 passed |
| Final unfiltered all-feature runtime library | 1,555 passed, 3 failed, 28 ignored |
| All-feature runtime doctests | 52 passed |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-features runtime check | Passed |
| Formatting, whitespace, source continuity and SSH signature | Passed |

The three broad failures are the unchanged `authorized_execution` telemetry
tests in `qualification.json`, all failing at socket inspection with
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`. They were
not skipped or counted as passes. Counts overlap.

The focused groups exercise partial ranges, stale shadows, exact sentinels,
full-page compute-ready hashes, multi-chunk scratch reuse, read-all-before-write,
observation-only polling/waiting, active-DMA cancellation, event release,
same-stream and explicit cross-stream ordering, and unordered rejection.
They also cover staging-budget admission, warm/cold settled allocation failures,
prepublication cancellation, recovered disposal through public Context drain and
cancel, returned-token release retries, and failure propagation at depths 3 and
256. Terminal polling and retirement unwind retain custody and seal retries.

The scripted driver consumes actual runtime allocation/submission/retirement
owners. No successful script supplies a GPU Wait step. This establishes tested
CPU control flow, not physical DMA behavior, timing or concurrency. Positive
composed request-account adapter execution was not available in this CPU fixture;
the charge path was compiled and independently reviewed, not positively qualified.

Two read-only reviews identified and checked fixes for cleanup-error settlement,
intermediate-chain liveness, exact ownership, accounting and panic handling.
`receipts.tar.xz` retains development attempts, final checks, command/status
manifests, source patch/hashes, tool identities, signature, review notes and access
failures. Earlier compile errors, a fixture-teardown abort and Clippy findings
are preserved rather than omitted.

Archive SHA-256:
`dcc4e1013620b935de3feb19f041200aa9add9f0fe2247ac53d5630cc5704321`.
Comparison against the complete original receipt directory passed.

## Remaining Work

`native_dirty` preparation still falls back to synchronous host-transfer APIs.
It needs generation-pinned, extent-specific readback/upload ownership before
the router can implement the directed no-wait SPI. Allocation, CPU work and
driver operations also prevent a hard latency guarantee for the new clean path.
The [composition work order](../../runtime-peer-producer-composition-v1.md)
records the next steps.

Pending peer-to-compute admission, positive composed/native qualification,
mixed-kind formal coverage, native XGMI/compute composition and matched HIP/HSA
measurements remain open. No Verus campaign was changed or run. MI300X DNS
resolution failed before any remote process or artifact was created. This
checkpoint makes no speedup, native correctness, full parity or milestone-closure
claim.
