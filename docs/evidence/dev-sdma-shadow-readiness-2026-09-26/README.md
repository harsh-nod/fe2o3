# Authoritative SDMA Backing Readiness

Date: 2026-09-26. Development CPU evidence only. Accepted milestones, A1/A2,
formal correspondence, native qualification and HIP/HSA parity are unchanged.

## Source And Behavior

Signed implementation: `39537c9fbade603a731bfcb8315aa20ccdf7bc9d`.
Parent: `9a6ecd58abdeec93e0bb27bdfa6cdb69e5eaf5f7`.

`copy_async_v1` previously deferred initial H2D/D2H publication when either
allocation had a dirty CPU shadow, even if its retained DMA backing was already
authoritative and every dependency had succeeded. Polling observes deferred work
without publishing it, so such copies required an unnecessary explicit flush.

Immediate admission now checks `native_dirty` rather than also rejecting
`sdma_shadow_dirty`. Separately materialized compute data still requires
reconciliation. Dependency gating, owner extraction, native publication,
retirement, failure custody and H2D compute-ready authentication are unchanged.
The change adds no allocation, native copy or host shadow synchronization.

The regression performs H2D, D2H, then H2D using real runtime SDMA ownership with
a scripted lower driver. The old predicate requires two explicit flushes; the
fixed predicate requires none. Both versions are cleaned up before the final
flush-count assertion. This is a demonstrated API-progress difference, not a
measured latency or throughput improvement.

## Qualification

| Check | Result |
| --- | --- |
| Pre-change round-trip regression | Intended failure: 2 flushes required, expected 0 |
| Intermediate focused suite | 6 passed |
| Final focused suite | 7 passed |
| Unfiltered all-feature runtime library | 1,543 passed, 3 failed, 28 ignored |
| All-feature runtime doctests | 52 passed |
| All-feature/all-target Clippy with `-D warnings` | Passed |
| No-default-features runtime check | Passed |
| Runtime formatting and staged whitespace checks | Passed |

Build, test and lint commands used offline mode, `CARGO_INCREMENTAL=0` and
`CARGO_BUILD_JOBS=1`. Counts overlap. The three broad failures remain the
`authorized_execution` telemetry tests listed in `qualification.json`, all with
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`. None was
skipped or counted as a pass.

The seven final groups cover:

- Full round-trip data, Pending observation and rejection of host writes while
  custody is active, with no passing-run flush or scripted wait.
- Partial H2D/D2H ranges with unequal offsets, distinct sentinel bytes and stale
  host shadows remaining untouched.
- Native-dirty extents on either endpoint, both directions, outside the copied
  interval: publication remains deferred, authority unchanged, and cancellation
  refunds custody without entering native preparation.
- A genuine published SDMA producer with explicit cross-stream or implicit
  same-stream dependency. Event release preserves the retain; consumer polling
  observes producer completion but still cannot publish previously deferred work.
- Failed and cancelled producers across both directions and ordering modes.
  A scripted preparation blocker is injected and cleared around real ledger
  admission; no native reconciliation is claimed. Consumers fail without
  publication, retain/capacity counts return to zero, and failed-event admission
  rejects before consuming a handle.
- Recovered and terminal initial publication failures in both directions,
  preserving dirty metadata and exact backing/custody.
- Page-sized promotion controls: a clean authenticated host source overwrites
  dirty device backing and promotes with the correct digest; a dirty source
  shadow never becomes compute-ready provenance, even with a retained stale hash.

Two independent read-only reviews found no production blocker. The second
review's failed/cancelled dependency coverage gap was closed and rereviewed.
Scripted retirement and cleanup are not native hardware qualification.

`receipts.tar.xz` retains every test attempt, static-check log, exact command
manifest, toolchain identity, source/signature receipt and hardware-access
failure. Its SHA-256 is
`3edced5065e9e82f6bc79fd89b34a9948cd402bd1be5a6488fecf9204b67187c`.
Archive comparison against the complete original evidence directory passed.

## Remaining Work

The review found a prerequisite for the pending peer-to-compute path: existing
cooperative DeviceLocal host-transfer leaves can execute synchronous SDMA with
a 30-second wait. They cannot simply be wrapped in the directed no-wait SPI.
The [composition contract](../../runtime-peer-producer-composition-v1.md)
now records the required resumable child transfers, accounted staging, exact
owner retention and native-dirty extent/generation reconciliation.

This change only advances initially ready copy publication. Deferred dependencies
and multi-window continuation still require explicit progress. It adds no
directed router implementation, pending copy-to-compute support, mixed-kind
proof, native-XGMI composition or new Worker/device-language/collective authority.
No Verus campaign was changed or rerun. MI300X hostname resolution failed before
any remote process or artifact was created. Matched native HIP/HSA measurements
remain open; no full parity or speedup claim is made.
