# Pending Directed Peers For Context Launches

Date: 2026-09-26. Development Context CPU and finite-projection evidence.
This does not close A1/A2, issue #182, native qualification or HIP/HSA parity.

Parent: `032640003296fad3aaf4700898e160ad46930923`.
Signed implementation: `fcfbfbe4f9be520281564af487f19c53d408cd97`.

## Implementation

Context producer-aware launches accept exact pending directed peer parents,
including mixed native/peer input rosters. Original Read aliases must each lie
inside the producer's captured destination interval. Whole-allocation journal
leases remain lifetime custody, not evidence of wider written contents.
Ordinary pending peers and pending writable aliases still reject. Exact writer
identity and generation survive slot reuse. Depth includes settled ancestry.

Physical consumer success remains separate from logical settlement. The
unchanged shared planner reconciles retained parents first, keeps canonical
Context callback order, and preserves the original backend dependency order.
Parent failure/cancellation/Unknown remains structurally valid for an already
admitted consumer. Contradictory observations preserve terminal custody.

Production KFD still rejects pending peer-to-compute. Its child compute custody
would block the producer supplying inputs; native publication gates and exact
predecessor access are not implemented by this checkpoint. Mock tests apply
actual bytes and independently inject logical observations, not native GPU work.

## CPU Qualification

| Check | Result |
| --- | --- |
| Producer-launch focused groups | 47 passed, including 10 new pending-peer groups |
| Unfiltered all-feature runtime library | 1,590 passed, 3 failed, 28 ignored |
| All-feature doctests | 52 passed (8 + 44) |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-features check, formatting, whitespace | Passed |
| Source continuity and SSH commit signature | Passed |

The three failures are unchanged telemetry socket-permission failures at
`authorized_execution.rs:1317`: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. None are skipped or
counted as passes. Focused and broad counts overlap.

New controls cover all seven completion ingresses, actual mixed input bytes,
reversed explicit dependency order, early event release, whole-allocation leases
with partial-range aliases, ordering-only dependencies, parent failure/cancel,
late discarded results, contradictory physical/logical results, consumer cancel,
writer-slot reuse, pre/post-observation corruption, and depth 255/256 admission.

## Formal Qualification

All 38 campaign phases pass. Signed opening, exact relocated and closing full
proofs each report 56 verified / 0 errors with the pinned default solver limits.
These are repeated measurements of 56 obligations, not 168 distinct theorems.
Both 190-file verifier closure measurements match; source snapshots are exactly
equal. Every owned subprocess record reports its process group absent.

The finite projection now admits explicit directed-peer-to-directed-peer,
launch-to-launch and launch-to-directed-peer edges. It retains exact identity,
generation, local-order, depth and successful cursor-prefix validation. A
two-node executable constructor witnesses an exact two-iteration Observe of a
pending peer under a physically successful launch, preserving every node and
quarantine state. The general planner has no added graph-validity precondition.

All 28 logical mutations reject cleanly: 23 change the shared production body,
five change projection adapters. Three new executable profile controls reject
denial of the mixed edge, admission of its reverse, and admission of an ordinary
kind-0 parent. Their specifications and identity/depth checks are unchanged.
The controller rejects resource failures, including those accompanying logical
failures. Calibration passes nine campaign, twelve inherited classifier, and
eight source-correspondence groups. The previous 53-obligation packet is not
reused as evidence for this expanded root.

This remains a finite-graph projection, not complete Context/journal refinement.
Exact mixed completion/quiescence outcomes, ordinary terminal-peer constructor
correspondence, native publication and device execution remain open. The kind-0
mutation exercises malformed finite input, not real ordinary-peer construction.

## Retained Evidence

The archive contains signed implementation/patch, repeated CPU receipts,
source/tool measurements, all 38 campaign phases, exact relocated and mutated
sources, review scope and network checks. Early test failures were a private
test-field access and an incorrect pending-release expectation. Exploratory
proof attempts 1-5 failed helper assertions or default resource limits; attempt
6 passed after explicit lookup facts and opaque parent-ID staging. Earlier
attempt outputs are retained, but not complete source closures for each attempt.
They are not accepted qualification results. No assume/admit/external-body
shortcut or increased SMT budget was introduced.

`receipts.tar.xz` was compared against the complete original receipt directory.
Archive SHA-256:
`961dce99429cca80394ae5a3bf28e726fc7e5dd958fc91527a9f5f510f6967b9`.

MI300X hostname resolution failed before execution, so no remote artifact was
created and no shared-machine cleanup was necessary. API access was intermittent;
a direct read confirmed #182 open, while the final scripted API read failed.
No permission request, escalation, native timing or speedup claim was made.

The [composition contract](../../runtime-peer-producer-composition-v1.md) tracks
the remaining native gate, predecessor access, all-memory-path, accounting,
native XGMI and matched HIP/HSA requirements. Accepted Native R125, Admission
R118B C1-C3 and Resources R116/V3 milestones are unchanged.
