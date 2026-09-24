# Native Directed Owner Witness

Status: [the exact native diamond passes](evidence/dev-xgmi-directed-owner-native-mi300x-2026-09-24/README.md)
after the [shared-source backend correction](runtime-xgmi-shared-source-v1.md).
The earlier refusal remains recorded and the witness itself is unchanged.
GNU and musl each pass six example tests; fourteen Python tests, compiled CLI
checks, formatting and strict Clippy pass. This is not general native or A1/A2
qualification.

The example `gfx942-runtime-xgmi-directed-owner-smoke` exercises the
[directed async adapter](runtime-directed-async-peer-v1.md) against the native
two-device XGMI backend. It adds no production API, Worker transport or generic
multi-device compute support. Historical ordered-owner sources and receipts
remain unchanged: their pending-input refusal is a different profile.

## Exact Workload

Five 65,536-byte device-local allocations have homes `[0, 1, 0, 0, 1]`. Four
copies each transfer 32,768 bytes on distinct destination-owned streams:

| Copy | Source offset | Destination offset | Dependencies |
| --- | --- | --- | --- |
| A0 -> A1 | 4,096 | 8,192 | None |
| A1 -> A2 | 8,192 | 12,288 | Root |
| A1 -> A3 | 8,192 | 16,384 | Root |
| A2 -> A4 | 12,288 | 20,480 | Both branches |

This is a dependency diamond, not a two-source data merge. Source values are
always below 127; each destination has a distinct canary above 127. The oracle
derives every destination directly from the original source pattern, never
from an earlier readback. All five complete buffers are checked, including the
right branch which supplies ordering but not data to the final copy.

The first three pending operations are admitted through an owner Context
command. A bounded gate queues the tracked async join and a subsequent event
release command. With one command and one operation advance per tick, release
occurs after join admission but before its first directed progress. The release
command requires all four streams to retain one Pending operation and the
journal to retain four input records before releasing all three public events.
No independent event or stream progress registration is installed.

After awaiting the join, the witness requires four exact typed Succeeded
submissions, success-only stream accounting, no rejected observations, zero
snapshot charge, zero journal input records, 131,072 payload bytes, 131,072
destination guard bytes and the unchanged 65,536-byte source. All commands
assert the same non-caller owner thread.

## Failure And Cleanup

One absolute 60-second observation deadline covers setup replies, gate entry,
gate release, join observation and final readback reply. Backend construction
precedes that deadline and is bounded by the outer 300-second workload process
limit. A deadline is not evidence of cancellation or quiescence.

Any workload Result failure still invokes explicit owned shutdown. Dropping
the gate sender disconnects its receiver, so an enqueue failure cannot leave
shutdown permanently blocked behind the gate. If cleanup also fails, the
workload error remains the primary error and cleanup is reported secondarily.
Success prints only after Released disposition, no owner panic/native failure,
and a complete cleanup report without failures. Existing backend constructor
fail-stop and uncertain-custody policies are not weakened.

## Campaign

`benchmarks/runtime_gfx942/xgmi_directed_owner_campaign.py` builds the signed
clean source as a release musl example with default features. A fresh strict
parser accepts only the exact single-line receipt, ordered endpoint identities,
complete cleanup and explicit false performance/refinement/reservation flags.
The source-bound campaign includes compiled invalid-CLI checks that must run,
not skip. Their inputs reject before native initialization.

The new sibling runner reuses the pinned host observer and process controller.
Both endpoints require fresh preflight and fixed two-second settled and
twenty-second delayed postflight checks. Source, toolchain, host and ELF
identities are bracketed. Results must be inventoried and collected byte-exactly
before marker-owned cleanup, followed by independent process/path absence.
An admission refusal is retained, not replaced by a later idle sample.

Run only on a freshly admitted pair on `mi300x`; idle observation does not confer
exclusive reservation. The captured passing witness qualifies only this exact native
graph and ordinary cleanup, not fault recovery, physical compute/copy overlap,
executable refinement, matched bandwidth or A1/A2 completion. Its separate sealed
native packet and offline verifier preserve that scope.
