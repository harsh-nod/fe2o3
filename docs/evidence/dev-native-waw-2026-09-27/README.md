# Native Full-Output Reuse

Development checkpoint, not Context successor-writer support, #182 closure,
A1/A2 acceptance, full HIP/HSA parity, formal refinement, or performance acceptance.

Implementation: `527be1ea1b49e5de5c48eb98e08f2b34f5ef52cc`, SSH-signed and verified.
The primary agent implemented and tested; read-only agents reviewed native
ownership/failure semantics and the separate Context writer-queue requirements.

## Implementation

The exact native three-binding R/R/W profile can now wait for a full output that
is owned by an active compute dispatch. This permits write-after-write and
write-after-read ordering through authenticated explicit producers, FIFO ordering
or retained native quiescence ancestors. It does not turn quiescence into success.

The change removes the output-only rejection from existing wait eligibility and
renames the helper to refer to bindings rather than inputs. Full extent, distinct
allocations, device, active execution, admission roster and exact custody checks
remain. Publication still waits its actual gates, restores the owner's storage,
recomputes persistent admission and applies final authorization. There are no new
allocations or traversal structures. No measured hardware speedup is claimed.

The native producer-aware SPI and ordinary submit boundary is unchanged. ReadWrite
remains outside the exact persistent profile; its existing generic path is not
newly enabled or globally rejected by this change.

## Qualification

| Lane | Result |
| --- | --- |
| All-feature runtime library | 1673 passed, 3 existing failures, 28 ignored |
| Eight new native groups and one Context group | All passed in final full run |
| Runtime doctests | 52 passed |
| Strict all-feature/all-target runtime Clippy | Passed |
| No-default-feature runtime check | Passed |
| Runtime formatting, whitespace and source signature | Passed |

Native fixtures use five genuine scripted owners: producer `[a,b,c]` and consumer
inputs `[d,e]`, with output chosen from `[a,b,c]`. The output is the only overlap.
The nine-case success matrix covers both predecessor reads and its write under
FIFO, same-stream explicit and cross-stream explicit ordering. Successful consumers
reuse restored persistent storage with zero user-data materializations.

Other cases cover unpublished cancellation/refund, missing authority, partial or
aliased outputs, wrong owners, transitive explicit failure versus failure-neutral
FIFO ordering, corrupted restoration, final authorization rejection, and terminal
owner preservation. Successful cleanup checks zero owners, unexpected drops,
retains and completion reservations. Fault-only teardown is not recovery evidence.

The Context regression establishes the current boundary: even an exact producer
event cannot admit a pending full output before reconciliation. It checks unchanged
identities, backend calls/memory, journal state, dependencies and allocation credits;
the same destination works after predecessor reconciliation. This is not a positive
Context WAW test.

The full suite is not green. The unchanged `authorized_execution::tests` failures
are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, all failing
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`.

## Receipts

Frozen raw directory: `/home/harsh/.codex-tmp/fe2o3-native-waw-20260927-OInR8E3U`.
The [receipts archive](receipts.tar.xz) includes exact command/result scopes,
development failures and their fixture corrections, final CPU logs, compiler and
test-binary identity, source hashes, signed source patch and signature verification.
Archive SHA-256:
`a4321754cb5d60eafb3de6f1484f59988b8a16fefd472386f650c6bb2a87e24a`.
Archive comparison and all receipt checksum checks passed.

## Remaining Work

The [successor-writer integration plan](../../runtime-successor-writer-plan-v1.md)
records the next Context implementation: exclusive queued writer ownership,
queue-aware admission across every read/write/disposal path, exact activation and
lineage, whole-roster cancellation, terminal custody and separately qualified
proofs. Native wait eligibility alone cannot supply those guarantees.

General multi-destination, mixed ready/busy, partial/ReadWrite and downstream-read
semantics remain part of the objective. Concrete graph/ownership refinement,
mixed DAG/fanout and multi-window qualification, XGMI composition, matched hardware
benchmarks, and broader Worker/device-language/atomics/collectives/multi-device/
profiling/release gates also remain open.

MI300X DNS failed before connection. No remote files or jobs were created.
No GPU, ELF, distributed-host, new Verus or performance campaign ran. Prior scalar
quiescence proofs do not establish formal refinement of this native admission.
