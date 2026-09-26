# Directed Cooperative Router

Date: 2026-09-26. Development CPU/scripted evidence only. A1/A2, issue #182,
native qualification, formal refinement and HIP/HSA parity remain incomplete.

Parent: `0751fb296a747d3cd48601bc29966a3971bce292`.
Signed implementation: `62347b9d6181d179dafcbbf13267cf90185810d2`.

## Implementation

`KfdMultiDeviceRuntimeBackendV1` now implements the directed scalar peer-copy
SPI using its existing cooperative copy owner and child async SDMA ledger.
Immutable route, extents, original ordered event/producer roster, FIFO predecessor
and completed-ancestor depth remain retained through guarded submission release.
Admission authenticates event kind/child/producer identity and caps selected
allocation custody, including legacy copies joining directed-owned allocations.
Read/Write require all dependencies to have been consumed successfully.

Targeted progress selects one dependency or exact private resource owner, then
executes one step. Shared-source D2H and generation-reconciliation contention
can advance the actual blocker without a fabricated dependency or recursive
poll. Read and Write requesters can advance another allocation's exact read- or
write-side lane pin. Already-owned observations, readback, retirement and cleanup
do not wait on newer siblings. Private IDs are qualified by child identity.
Terminal ownership mismatch seals before progress; cleanup errors never report
another copy's terminal result as the requested copy's result.

Ordinary flush/cancel/release share retained-identity checks. The common phase
step also seals clean HostVisible read/profiling unwind paths, preserving custody.
The [contract](../../runtime-directed-cooperative-peer-v1.md) distinguishes this
host-staged backend from native XGMI and describes finite work versus latency.

## Qualification

| Check | Result |
| --- | --- |
| Directed focused groups | 19 passed |
| Unfiltered all-feature runtime library | 1,580 passed, 3 failed, 28 ignored |
| All-feature doctests | 52 passed (8 + 44) |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-features runtime check | Passed |
| Formatting, whitespace, source continuity and SSH signature | Passed |

The broad failures are the same three telemetry failures at
`authorized_execution.rs:1317`, `InspectSocket(PermissionDenied)`: bounded
cooperative debug telemetry, explicit failed session end, and pre-native
telemetry failure poisoning. None were skipped or counted as passes. Test
counts overlap and must not be added together as unique coverage.

Coverage includes actual router/Context pending chains with and without the
version journal, true reversed two-parent ordering, event and parent retirement,
completed depth 256/257, read fanout, 256/257 owner capacity including a legacy
join, phase and consumed-prefix corruption, event routing/orphan tails, selected
ancestor cleanup failure, native-scripted partial DMA, source contention,
independent readback, same-source and cross-allocation lane pins, orphan/private
owner corruption, panic retention and successful disposal/account refund.
Terminal scripted tests inspect retained custody before disarming synthetic
teardown. They own no real native queue or admitted device.

Development history is retained: the first build lacked two Context journal
capacity arguments; the next run had two tests incorrectly destroying retained
streams; subsequent 14- and 17-group runs passed before the final 19-group run.
Read-only reviewers found and checked the integrity, capacity, liveness,
child-local identity and error-attribution fixes. No separate private-ID-collision
integration fixture was added; that guard has code-review evidence only.

`receipts.tar.xz` contains the implementation patch/commit, source hashes,
toolchain versions, development failures, final command/status logs, signature,
review scope and access results. Comparison against the complete original
receipt directory passed. Archive SHA-256:
`01f80a62d6c24d5b42b1b4a64e3aa7c8dc7aa8476956351adb6e68b8663b04e6`.

## Remaining Gates

Pending peer-to-compute, all-admitted-device composition, native XGMI/compute,
positive native composed-account execution, mixed-kind formal refinement,
protected Worker/compiler execution and matched HIP/HSA performance remain
open. This is not a hardware timing or speedup claim. Existing accepted Native
R125, Admission R118B C1-C3 and Resources R116/V3 checkpoints are unchanged.

MI300X SSH failed hostname resolution before any remote execution. No remote
artifact was created, so no shared-machine cleanup was needed. API access was
intermittent; a successful direct read confirmed #182 remained open. No
permission request, escalation or sandbox bypass was used.
