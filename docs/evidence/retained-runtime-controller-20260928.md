# Runtime Inventory and Proof-Controller Custody

Checkpoint, 2026-09-28. Final local checks passed and remote cleanup completed.
This continues the [compiler approval checkpoint](compiler-approval-runtime-20260928.md).
It is not a release report and completes no issue #272 M0-M7 milestone or 47/47
launch gate.

The next [native aggregate cleanup checkpoint](native-domain-cleanup-20260928.md)
adds retained cgroup custody and actual Linux descendant cleanup validation.

## Implemented Boundaries

Commit `63c52c43eb69cbd5e9d145eec57de3bed3dafed4` adds a canonical, inert compiler
runtime inventory and its retained fixed-path owner. The inventory includes all
six unchanged compiler pins, the transition and proof-runtime identities, and
bounded executable/library roles. Admission checks exact bytes, ownership,
permissions, immutability, no-follow path traversal and retained object identity.
The original work ledger and storage charges remain attached through Cargo's
compiled native continuation; its positive execution remains refused. Neither
decoding nor retaining this inventory implements
ELF dependency enforcement or observes a compiler execution.

The private proof-executor transport binds each request to a session, runtime,
source, sequence and absolute deadline. Frames and cumulative traffic are
bounded; malformed, timed-out or incomplete sessions cannot complete. Absolute
deadlines require a bootstrap-admitted common `CLOCK_MONOTONIC` time namespace;
the transport does not establish that prerequisite. Actual
per-packet SCM_CREDENTIALS are checked on the SEQPACKET transport, rather than
treating a pre-fork socket creator as its later writer. The transport remains
inert: arbitrary replies cannot construct a positive execution owner or receipt.
Root-to-helper bootstrap and production execution admission remain missing.

The retained proof controller now uses gated raw fork followed by pre-exec
SEIZE/INTERRUPT. It stops tracked sharers before inspecting mutable mappings,
descriptors and clone arguments, completes one admitted kernel operation,
checks its result and only then releases parked tasks. Creation and vfork
transitions retain actual event custody. Cleanup discovers unresolved births
before fatal signals and requires real terminal waits, not procfs disappearance.
This does not exclude external writers or enforce the full compiler runtime.

## Measured Group-Exit Failure

The first protected r12 candidate used commit `63c52c43e`, not the later fix.
The installed runtime audit passed, but genuine Verus execution failed with
`exiting task skipped its authenticated exit checkpoint`. The remaining false
proof, helper, race, actual-source and F-prefix tests did not run. The attempt
was drained, fully archived and its owned run removed. No retry or success may
be inferred from that failure.

A new local regression reproduced the failure: after the requester's checked
EXIT event, a sibling terminated normally without its own EXIT stop. Commit
`7faa8da01aeca34c93580373d2385c478c01bbca` distinguishes `Task(status)` from
`Group(status)` evidence. The latter is recorded only after the actual group-exit
requester reaches a checked EXIT event. GETEVENTMSG and every terminal wait must
match the admitted exit status. A group boundary is not a fabricated per-task
checkpoint, and a signal or different status remains refusal.

The regression exercises both leader and nonleader requesters, eight times
each, with real process-tree terminal observations and no cleanup-kill fallback.
Separate mutations reject mismatched task/group statuses. The full verifier
suite then passed 483 tests with 18 explicitly ignored and no SKIP marker.

## Local Evidence

All Cargo commands used the pinned nightly, locked offline dependencies, one
build job, serial tests, empty GPU visibility, bounded memory and an outer
deadline. Guard records preserve matching before/after source and tool hashes.
These are local tests, not protected-runtime or hardware observations.

| Scope | Result | Guard label |
| --- | --- | --- |
| Inert runtime inventory codec | 7 passed | `runtime-manifest-tests-rtwo` |
| Retained runtime inventory owner | 15 passed | `runtime-inventory-tests-rtwo` |
| Owner compile-fail doctests | 5 passed | `runtime-inventory-docs-rtwo` |
| Cargo native continuation/refusal | 22 passed | `runtime-cargo-native-tests-rtwo` |
| Exact normal group-exit regression | 1 passed, 16 actual drains | `runtime-group-exit-regression-rthree` |
| Full verifier library target after exit fix | 483 passed, 18 ignored | `runtime-verifier-full-rtwo` |
| Quarantine and kill-refusal negatives, before pending-creation fix | 10 passed, 3 ignored | `runtime-quarantine-tests-rtwo` |
| Retained runtime regression, before pending-creation fix | 100 passed, 7 ignored | `runtime-retained-quarantine-regression-rone` |
| Full verifier library target including pending-creation custody | 495 passed, 22 ignored | `runtime-verifier-full-rthree` |
| Retained induction analysis after private accounting extraction | 12 passed | `runtime-retained-induction-split-rone` |
| Five-package, all-target integration check | passed | `runtime-integration-check-quarantine` |
| Cargo native continuation/refusal after quarantine | 22 passed | `runtime-cargo-native-quarantine-rone` |
| Reviewed unsafe-source inventory, before spawn-lease fix | 5 passed, 1 maintenance test ignored | `runtime-unsafe-inventory-rtwo` |
| Spawn-lease lifecycle and custody regressions | 5 passed | `runtime-spawn-lease-regression-rone` |
| Full verifier library target after both lifecycle/fixture fixes | 502 passed, 22 ignored | `runtime-verifier-full-rfour` |
| Final five-package, all-target integration check | passed | `runtime-integration-check-final` |
| Final reviewed unsafe-source inventory | 5 passed, 1 maintenance test ignored | `runtime-unsafe-inventory-final` |

The earlier exit-fix full verifier run names 9,175 files, source identity
`46233be110e88c39844632f02884bb09471be4b4fd63a769f0d6aef86ae43158`,
and log SHA-256
`a7a237fac055da1e6f2c099bd12354e8d0efa53811fe1efbd5feb174f09c1304`.
The other rows have their own earlier guarded source identities; they must not
be relabeled as tests of a subsequent merged revision. Scoped formatting,
whitespace, DCO, hygiene delta and eight hygiene-policy tests passed for the
exit-fix integration. Existing compiler warnings remain.

The earlier full verifier library test target includes the quarantine and
pending-creation fixes and the strengthened repeated-wait regression committed
in `fd88664b4`.
Its 9,188-file source identity is
`8b4b96c6108332bc8c35d2de784df6fcf844057cc0f006314db331939351f13b`,
with log SHA-256
`1e68b93df2c362323cd2f9cac71cc08bf2eea1fbd80e722aa8b8716cd93ab067`.
All 495 enabled tests passed in 139.19 seconds; 22 were explicitly ignored,
and there was no SKIP marker. Intentional, caught panic cases are part of the
resource-ownership tests, not failed tests.

Commit `96c38c282f50728606710ed67b7ee99ac7232d6c` then extracts the retained
induction analysis's unchanged 33-row frame accounting into a private module.
Its parent is reduced from 1,348 to 1,061 lines; the source-contract test scans
both files. All 12 retained-analysis tests and the five-package all-target check
passed with matching 9,189-file source identity
`828a4254bf7e09631f5655e4185f3ecfe52971896b2c1ab9d3e75ee8e6b462a9`.
Their log SHA-256 values are
`f1f03e06cde8dc323062c488e2b94532f35e61da92a8966ca7ccb903ea69b464`
and `607affc9f1d009bde1d2a2e1f0c70c617f2981135c125a0a6b805a43a8bdd243`.
The earlier 495-test run is not relabeled as a post-extraction run.
The Cargo native regression also passed all 22 tests on that same source
identity, with log SHA-256
`2d712694c8108d14fb0df46aa2d9dc93cdfdb0e5ed4d5617edd09d53bf9c6bac`.

Scoped pinned formatting, whitespace, the hygiene delta against shared main
`41fc1167f1a94810482fcf51024215d6361ca4cf`, and eight hygiene-policy tests pass.
A broader historical comparison against `05559db5a` still reports three oversized
backend files already present on shared main; the scoped pass is not a claim that
all historical hygiene issues are resolved. No policy waiver was added.

The tokenized unsafe-source gate initially found eight unreconciled files, all in
the reviewed proof controller or its fixtures. The deliberate additions, moves
and removals were documented and reconciled without refreshing unrelated entries.
The gate then passed five tests with only its explicit maintenance command
ignored (log SHA-256
`df9a76e3483bdb215c151b3fb6c8c594ae47f073204c8e4de0ab1ffaa48eb522`).
The accompanying review also found a separate lifecycle bug: the raw-fork wrapper
released its artifact spawn lease while the child could still be pre-exec.
The inventory pass does not approve that lifecycle. Its separate fix and
verification are described below.

## Protected Retry Not Started

The preserved, pre-quarantine protected candidate is
`c09ce9d01a6fb16fc008c542add8529ec81f7e3c`, including the exit fix and the other
team's retained capability/constant-analysis commits. Its 9,183-file source
identity is `29345096624bf12bfb52b60f9170acc628c3d5dbb16ea29284c079dc93f73a5a`.
The fresh artifact and preparation guards passed on that identical source and
tool tuple. The preparer passed one exact test with zero ignored in 604.30 seconds.
The independently copied source, prepared trees and both binary hashes were checked
before the live source was unfrozen. The preparation metadata identity is
`894679e7bf05ce6643256a6b15f24e63ba94f372926117ba3c2f993c7167ae68`.
These artifacts lack the later quarantine, kill-failure and pending-creation
changes; executing them would not validate the current controller.

SSH then timed out before authentication on both the initial host check and one
bounded reconnect. Zero new protected attempts, incoming directories, requests,
run tokens, services or cgroups were allocated. There is no uncertain new run.
This is a transport failure, not another proof failure or a successful replay.
The transport report SHA-256 is
`2da90f379ae123b22886662c5abaefa8891d7ad8ca81c77a0f581da6d292b5b0`.

The prior failed run was drained and removed. Its old upload, private provisioning
root and six-file temporary nightly support installation were initially held on
MI350-2 during the outage. A later, cleanup-only reconnect succeeded, followed by
fresh exact-inventory, hash, mode, inode and no-use checks. All three owned roots
are now removed and their absence was checked; no residual or mismatched root
remains. Shared runtime objects, the image, and toolchain parent directories were
preserved. This invoked no proof, build, preparation or new run.

The final cleanup report SHA-256 is
`6bee302046b77bf71af7a2dbbc5db48cfc5cbd91e999853ef1bbb72c6ba01c11`;
its terminal removal report is
`519676eefd9c5b9c51712af9b92a57b20570c7ade265ca4915cc3f2560e704b1`.
All 22 cleanup evidence files have checked hashes. Local source snapshots,
prepared inputs and reports remain available. Inactive owned compiler, Cargo
and verifier test binaries were relocated to RAM-backed storage, with matching
before/after hashes, to make room for local linking. Historical outage reports
remain unchanged; this paragraph supersedes
only their held-root cleanup status.

## Failure Quarantine

The integrated quarantine changes retain one owned attempt before fork, with a
fixed 33-task reservation, originating-process/thread affinity and a nonblocking
process-wide gate. The gate remains held through receipt or channel publication.
Unresolved cleanup, or unwind while custody remains unresolved, leaves the task
inventory, sealed source, runtime backing and process resources in permanent
static custody, independent of caller
lease/error destruction. There is no reset, cross-thread reaper, destructor wait
or positive authority constructor. Passive quarantine is not eventual cleanup or
whole-domain containment.

Review found and addressed two further error paths. A failed kill, including
ESRCH, now inhibits every cleanup CONT: a ptrace restart's signal argument cannot
substitute for actual kill delivery at a nonsignal stop. See Linux's
[ptrace restart implementation](https://github.com/torvalds/linux/blob/v6.6/kernel/ptrace.c)
and [signal/stop handling](https://github.com/torvalds/linux/blob/v6.6/kernel/signal.c).
Creation is also recorded
before the syscall step. A parent's terminal wait cannot discharge an unknown
child obligation; only an authenticated child registration or a checked negative
syscall completion can do that. Removal, PID replacement and the cleanup stop
condition preserve unresolved creation state.

The first quarantine compile found one obsolete ignored-test call site. After it
was updated to retain an attempt, the 10-test negative suite and 100-test retained
runtime regression passed. The later pending-creation fix adds actual fatal-parent
and refused-birth scenarios plus a separate state-invariant test; all passed in
the 495-test full-suite run above. A separate read-only review of the pending-
creation transition found no actionable bugs under the trusted-waiter,
originating-thread and isolated-domain assumptions.
Protected backing-retention and receipt/channel publication tests remain
unexecuted, not credited from the local tests. Neither the tests nor the review
establish external-writer isolation or successful whole-domain teardown.

## Artifact Spawn Custody

Commit `1e289ad10181d1b4b73d4f4fa4c9eb1c89c8c4c5` replaces the short-lived
fork wrapper with the existing transferable `ArtifactProcessSpawnLeaseV1`.
The attempt publishes the lease before creating the child and retains it across
logical spawn return, attachment failure, unwind and unresolved cleanup. Only
the actual owned initial EXEC event, confirmed terminal disposal, or a no-child
refusal can discharge it. Missing root custody and unresolved child creation
cannot be treated as terminal disposal.

The new fixtures use a real artifact transaction on a separate worker thread,
observe its inherited lock alias in the gated child, and test release against
real exec/terminal events. Permanent-quarantine cases use the existing independent
diagnostic domain for teardown; they never join an unconfirmed blocked worker.
A separate state-only test covers missing, removed, terminal and unresolved root
records without fabricating OS execution evidence. All five focused tests passed
with zero ignored in 1.61 seconds. The 9,190-file source identity is
`85e392c3b65a55cf53de155c09030fb354cf2f9036d3ae4eb31d5aeaa7a55210`,
and the log SHA-256 is
`851f6b490422ed05e58defa0b7f97ece8f7382d521cdad73b5366139429878d9`.
Independent source review cleared the production lifetime fix, while identifying
a pre-driver-spawn scratch-cleanup gap in the fixture. Commit `829b1f294` adds a
pre-spawn scratch owner, transfers it into the published diagnostic domain before
spawn, and releases it only on confirmed no-child failure or terminal drain.
Two additional controls cover a real NUL-argument spawn refusal and pre-spawn
unwind. Independent review then closed both findings for source review.

The final verifier library run on code commit
`829b1f29478239ff7af44c07095077d2a7f7736c` passed all 502 enabled tests in
151.91 seconds, with 22 explicitly ignored and no SKIP marker. Its 9,190-file
source identity is
`a338c754db7c0dd7044673a816aa35d12243623447a10933577405612fa170cd`,
and log SHA-256 is
`7afa7495258bc85720d76d7f4b1ed83c30973402cbc622e1524a60814726a67b`.
The final five-package all-target check passed on that same source and tool
snapshot, with log SHA-256
`7c9f70cb32fab1462c9c6839ec8ca6d8a779effbc51252070edbf085ddd226ab`.
The final unsafe-source gate also passed on that same snapshot: five tests
passed, and only the explicit maintenance command was ignored. Its log SHA-256
is `62ce6ed9d12ae7ab3706411b20cf06637663964061970a4626091e5ba7d35631`.
Documentation-only edits followed these code checks. No protected-runtime or
hardware credit is inferred from them.

The existing shared coordinator's mutex acquisition and lock-release wait are
not deadline-bounded. The nonblocking proof-attempt gate does not change that
contract, and the patch does not claim it does. Kernel fork failure has a reviewed
no-child discharge branch but is not fault-forced by these fixtures.

## Remaining Production Work

The nondumpable, capability-free production helper still cannot use the SEIZE
launch transition without an admitted external-writer isolation boundary. A
gated child-only dumpability transition is also still unimplemented and is not
enabled; the helper itself must remain nondumpable. A root-created per-attempt user
namespace with retained domain cleanup is a reviewed direction, not implemented
isolation. Positive unisolated controls are required for every access-denial
probe; host security settings must not be weakened to obtain a result.

Both unconditional Cargo `RuntimeEnforcementUnavailable` refusals remain.
Full compiler runtime enforcement, approved proc-macro execution, closed sibling
bootstrap, exact invocation/receipt joins, V3 release/broker delivery, fresh
Worker admission, machine refinement and safe GPU launch are still required.
