# Generation-Pinned Host Reconciliation

Date: 2026-09-26. Development CPU evidence only. Issue #182, A1/A2, native
qualification, formal refinement and HIP/HSA parity remain incomplete.

Signed implementation: `1588ebde876f04ba4866075e7c9d6812f18cd326`.
Parent: `32b932ea4025dfc7fe284b357ee1edce1b336890`.

## Authority Correction

Review found that writable DeviceLocal materialization is rejected by
`snapshot_bound_data_v1`, and the native recycled read-into API accepts only
HostVisible authority. Persistent DeviceLocal compute instead makes its SDMA
shadow dirty. The initial DeviceLocal native-dirty upload proposal was removed,
not qualified with a fabricated positive fixture. The final implementation
addresses reachable HostVisible materialized writebacks; authoritative
DeviceLocal transfers retain the existing asynchronous SDMA path.

## Implementation

Each child retains a move-only reconciliation root containing the exact logical
and native lane, recycled generation, descriptor, dirty extent, scratch and
cursor. Allocation and lane pins survive between chunks. Conflicting pending
compute is deferred before staging can turn Busy into Failed. Replay, cache
detach, host access, ordinary copy and teardown cannot invalidate pinned data.

Each progress step reads at most the private scratch window, up to 64 KiB, then
writes only that exact HostVisible backing range. It does not submit or wait for
GPU DMA. Generation queries authenticate the captured value; they never replace
it. Only a fully reconciled extent is removed and counted down. Partial
cancellation preserves the complete extent for retry. Identity mismatch,
mapped-write failure and panic retain authority and scratch while sealing retries.

HostVisible destination writes bypass cache detachment and full shadow refresh.
They preserve unchanged native descriptors and invalidate endpoint content/write
provenance, requiring later ordinary launch preparation to refresh and overwrite
stale native data. Allocation custody excludes active/ordered-successor reuse.
Shadow-only allocations receive exact CPU range updates.

Synchronous reconciliation also uploads exact extents instead of a full stale
shadow. It preserves the previous shadow-dirty state rather than forcing a new
whole-allocation download. Scratch is reserved for every native endpoint at
admission, including HostVisible producer effects that appear before Read begins.

## Qualification

| Check | Result |
| --- | --- |
| Final focused cooperative groups | 18 passed |
| Unfiltered all-feature runtime library | 1,561 passed, 3 failed, 28 ignored |
| All-feature doctests | 52 passed |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-features runtime check | Passed |
| Formatting, whitespace, source hashes, SSH signature | Passed |

The three broad failures are unchanged telemetry environment failures at
`authorized_execution.rs:1317`, `InspectSocket(PermissionDenied)`. They were
neither skipped nor counted as passes. Counts overlap.

Six new focused groups cover late dirty authority, nonzero descriptor/data and
allocation offsets, three chunks including a one-byte tail, untouched backing
sentinels, observational polling, lane and allocation guards, exact-once dirty
count removal, generation/descriptor changes, cancellation followed by successful
retry, host destination writes without full reads, later partial mapped-write
error/panic, and pinned target/cached-binding compute remaining Pending. Existing
cooperative SDMA tests continue to pass. Scripted owners are disposed on success;
terminal tests inspect retained custody before disarming synthetic teardown.

Two read-only agent reviews identified the authority mismatch, stale-shadow
overwrite, scheduler failure-classification and whole-shadow destination paths.
Final review reported no concrete correctness blockers. `receipts.tar.xz`
contains development failures, final logs, command/status records, source patch
and hashes, tool versions, signature, review limitations and access failures.

Archive SHA-256:
`114c10d8ca267e9047f2f54684d5f0db0b3b6e87447bb18077e5296c663fd6e3`.
Comparison against the complete original receipt directory passed.

## Remaining Work

The router still lacks the directed-copy SPI and pending peer-to-compute
admission. Native XGMI/compute composition, mixed-kind formal refinement, positive
composed-account execution and matched HIP/HSA measurements remain open. No
Verus campaign or hardware execution ran. MI300X DNS failed before any remote
artifact was created; initial pushes to both remotes also failed DNS.

Tests do not establish native adapter correctness, multiple native-lane/extent
execution, the complete mixed resumable/synchronous path, positive shadow-only
reconciliation, or physical disjoint compute overlap. Allocation, copy-on-write
and driver costs also prevent a hard per-call latency guarantee. See the
[composition work order](../../runtime-peer-producer-composition-v1.md).
