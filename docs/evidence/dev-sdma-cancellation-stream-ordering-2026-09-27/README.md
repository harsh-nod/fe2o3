# SDMA Cancellation And Stream Ordering

CPU developer evidence over scripted native owners. This does not establish GPU
execution, formal cancellation/unwind refinement, complete #182/A1/A2 closure,
HIP/HSA behavioral parity, or performance parity. The full objective remains open.

Final signed source: `617d5e1cd51f9537ec67eac69b471ce2648d8101`.
Source tree: `c76dc0574f8c1c80ff2f92acc92c487d2124696e`.
Cancellation checkpoint: `f4506fee60fe580469e20df8feda6832f3d6edce`.
Baseline: `2ba769049798e0453e574aabb905b65dfc23b822`.
Both source signatures verified for `harmenon@amd.com`.

## Reproduced And Fixed

Cancellation removed an accepted SDMA descriptor before validating logical
cleanup. After genuine public A -> B admission, deliberately removing B's
destination custody entry made cancellation release the dependency/source prefix
and panic without restoring B. The before-fix subprocess records the missing
descriptor. This is private invariant-corruption coverage, not an observed GPU
fault or evidence of native storage being freed while active.

SDMA cancellation now uses the common indexed settlement path. Only exact Ready,
zero-progress, no-window custody can settle Cancelled, with the existing
`Failed { code: -2 }` result. Published or partially completed copies remain
TooLate. Other inconsistent unstarted states fail terminally without removing the
descriptor or releasing a logical prefix. The active SDMA check precedes the
completed-record shortcut so a duplicate completed record cannot mask that
preflight. Interior cancellation remains allowed, with successor dependency
retains and stream position intact.

Review also found a healthy stream-ordering bug. B could fail its first explicit
D dependency while earlier same-stream A remained Published. Releasing B then
cleared the stream tail, leaving subsequent work without A as its predecessor.
The public before-fix regression reproduces this missing ordering node.

Release and all three cancellation paths now restore the newest unfinished
same-stream owner. Candidates are the pending-compute and SDMA FIFO backs, plus
every primary/auxiliary active compute head and pipeline entry. Terminal results
are not candidates: restoring old failures after later work succeeded would
violate failure-neutral ordering. The helper selects the maximum monotone ID and
updates the existing map slot without allocation. It does not scan the terminal
result HashMap; nonempty compute-pipeline traversal is bounded by admitted slot
capacity, not claimed constant-time for arbitrary pipeline sizes.

These transitions use existing private FIFO/owner/lane invariants. Local release
preflight and restoration are not full owner-arena conservation proofs or general
corruption detectors. Pending/prepared compute cleanup remains separate from
the newly shared SDMA settlement path.

## Coverage

Sixty-six subprocess cases cover H2D, D2H and same-device copies, with or without
a retained successor, across eleven deliberate post-admission mutations:
destination custody removal, dependency retain removal, zero source owner count,
missing FIFO membership, zero reservation, Quarantined phase, inconsistent window
intent, duplicate dependency, duplicate FIFO membership, duplicate allocation
owner, and active/completed identity collision.

Each checks complete descriptor metadata, original dependency-vector backing,
native owner IDs and wrapper address, FIFO/tails, completion records, logical
retains, reservations, endpoint markers and driver noninterference. Repeated
cancel/poll/wait/flush/event-release ingress remains terminal and unchanged.
Parents require inspection markers and real unrepaired Drop SIGABRT; core dumps
are disabled. No terminal test disarms or repairs the backend, and snapshots do
not retain native owners.

Nine healthy cancellation controls cover all three copy kinds: tail cancellation,
interior cancellation followed by successor failure, and successive interior/tail
cancellation. They preserve exact predecessor owners, successor recipes,
`Failed(-2)` status, retained-completion Busy behavior and later successful cleanup.
Three partial-copy controls really complete an eight-byte first window of a
sixteen-byte logical copy, observe TooLate without mutation, then complete the
second window. The former H2D fixture no longer resets progress for cleanup.
The bounded-window policy is test-only, not production GPU geometry evidence.

The public failed-D/A regression releases B while A remains Published, then
actually admits disjoint C behind A and completes both in order. Four private
compute-roster controls exercise primary/auxiliary active and pipeline roots,
foreign-stream exclusion, retained terminal-history exclusion and stable map
capacity. Existing mixed compute/copy cancellation and profiler tests remain in
the broader suite. Clean scripted helpers verify retirement/recycling but disable
device scrub-on-release; they are not production scrub qualification.

## Qualification

Final signed-source qualification completed. Formatting, no-default-feature
runtime/model checks, strict all-feature/all-target Clippy, both source signatures,
and unchanged source continuity passed. The focused KFD run passed 831 tests,
with 28 ignored and no failures.

The full all-feature runtime/model command returned 101: runtime unit tests had
1,728 passed, three failed and 28 ignored; model unit tests had 1,080 passed and
19 ignored. Integration tests had 11 passed and three hardware tests ignored.
Runtime doctests passed 52 tests (eight plus 44); model doctests passed 29.

The three runtime failures remain explicit, not waived: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each failed at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)` in this
environment. They also failed in the preceding source qualification. The full
suite is not green and these receipts do not qualify those telemetry paths.

`receipts.tar.xz` contains commands, exit statuses, logs, before-fix reproductions,
the complete source patch, review notes and final source/signature receipts.
Its companion SHA-256 file authenticates the frozen archive bytes.

## Remaining Gates

No new Verus proof or matched HIP/HSA performance run is claimed. Full queued-owner
and arena invariants, Context release/disposal composition, pending/prepared
compute cleanup, native unwind boundaries, authenticated MI300X qualification and
the wider Worker/device-language/multi-device/memory/atomic/collective/profiling
requirements remain open. See the [successor plan](../../runtime-successor-writer-plan-v1.md).

MI300X SSH and source pushes to both requested GitHub remotes failed DNS
resolution. No remote jobs or files were created. No permission escalation was
requested. Unrelated owner-inspection evidence was preserved. Evidence-commit
publication is attempted separately after freezing this packet.
