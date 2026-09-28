# Prepared Compute Publication Custody

CPU developer evidence, not GPU execution, executable formal refinement,
complete #182/A1/A2 closure, HIP/HSA parity, or a performance comparison.

Baseline: `ddaf70c940abba02af40dd60e4eecf304175dcda`.
Signed implementation: `4621885cb0ded827c3803417325bdc00ea443486`.
Profiler fixture/lint corrections: `11c04bb1079fc9857f2dbed436bdf4275b6ab076`.
Non-dispatch release fix: `69fb06e62d6792fdcb44d08e0cb340eb67cc9149`.
Final signed source, including the corrected allocation-observer regression:
`e5a9e60047be737458d44b70041efad83c44fb58`.
Source tree: `2b9dadb94877a8f21a11e523e7deba81f78aa1f3`.

## Change

Both prepared persistent retry-publication branches previously removed the
active runtime descriptor before the consuming KFD call. An unrecoverable result
or unwind lost that descriptor, even though the lower queue retained native
attachment custody. Successful publication also called profiling before
restoring the descriptor. A synthetic profiling panic reproduced the latter
loss for the public-admission scripted path before this fix. It is not an
observed GPU failure or a demonstrated ordinary profiler-capacity panic.

Prepared receipts now distinguish `Armed(receipt)` from `NativeOwned` inside
the indexed active descriptor. The shared native/scripted transition installs
`NativeOwned` before consuming the receipt, re-arms only from an exact lower
retry receipt, and restores that receipt before destroying the returned error.
Terminal results and unwinds cannot fabricate retry authority. Success installs
Published custody before timing and profiling; unwind seals the backend and
preserves the original panic payload.

Publication and cancellation share the local allocation/module/stream/retain,
reservation, control-provenance and restoration-shell checks. Cancellation
requires Armed before allocating or extracting restoration custody. Native
publication adds no per-attempt heap allocation or cloned launch/roster. The
preflight visits at most three bindings plus their existing indexed membership
lookups. Private index invariants remain premises, not a global conservation
proof or arbitrary-corruption recovery guarantee. No speedup is claimed.

The complete-trace control also exposed a real profiler release defect:
`release_submission_v1` passed `None` to the recorder for a copy or unpublished
cancellation, despite neither having a dispatch lifecycle event. That falsely
recorded observation loss and froze the remaining trace. Release now observes
only records whose published-dispatch flag is true. An unavailable identity for
an actual published dispatch still records loss; it is not silently skipped.
Zero loss is scoped to V1's existing event vocabulary. This adds neither copy
engine events nor a complete record of every runtime operation category.

## Coverage

Twenty subprocess cases cover terminal, lower unwind and profiling unwind plus
seven malformed/preconsumed-state cases for each of one and three inputs.
Assertions compare exact typed input identities, byte backing addresses/digests,
restore-shell addresses, allocation markers, descriptor rosters/writebacks,
module/dependency/event retains, reservations, stream tails and lane leases.
They distinguish Armed, NativeOwned with retained inputs, and actual Published
variants. Terminal ingress cannot release custody. Each child then drops the
unrepaired backend and must SIGABRT after an inspection marker; core dumps are
disabled. This is scripted ownership evidence, not native queue execution.

Sixteen healthy origin/arity cases perform two retries before cancellation or
publication/completion. They compare exact typed inputs, publication provenance,
logical ledger and restoration shells after each retry, then exercise cleanup.
Origins include seeded H2D, initialized storage, retained replay and initialized
storage after replay. The seeded fixture alone is not an upload witness.

A separate public write/copy/poll single-input control uses the real recorder
with explicitly constructed logical compute-queue lifecycle records because
the scripted backend has no native queue constructor.
Two retries followed by success emit exactly one retained publication when
capacity permits. Exhausted recorder capacity retains a valid prefix without
changing successful execution or cleanup. The profiling-unwind hook above is
synthetic; ordinary recorder exhaustion does not panic.
Copy release checks preserve both retained-event and dropped-event counts;
the existing unpublished cancellation control now requires zero loss through
cleanup as well.
The allocation-while-compute-is-pending regression no longer expects the
spurious loss from its H2D setup: all six lane shapes and both memory kinds
require an unchanged prefix, zero loss and the exact appended allocation event.
That regression checks append behavior, not complete capture validation.

Two added lower KFD tests use constructed three-binding native attachments.
They cover retry, rejected, terminal and callback-unwind outcomes, checking
receipt identity, owner incarnation, initialized digests/effects, actual detached
native lease rosters and process gates. Both exact ring-occupancy outcomes also
exercise classified generation rollback followed by scripted publication. This
uses production lower bodies but does not submit AQL to a GPU or establish a
coupled runtime-to-GPU witness.

## Qualification

Final-source commands and results use the `verified-source-` receipt prefix.
The all-feature runtime/model test command returned 101:

- Runtime unit tests: 1,735 passed, three failed, 28 ignored.
- Runtime-model unit tests: 1,080 passed, 19 ignored.
- Integration tests: 11 passed, three ignored hardware tests.
- Runtime doctests: 52 passed (eight plus 44); model doctests: 29 passed.
- Separate lower KFD `persistent_` selection: 166 passed, none ignored.

The three failures are unchanged: `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. All report
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`. The full
suite is not green and these verification requirements are not waived.

Strict all-feature/all-target Clippy passed for runtime, runtime-model and KFD.
The no-default-feature runtime check, formatting, signed-source authentication
and source continuity passed. `receipts.tar.xz` preserves commands, logs, UTC
start/finish times and exit statuses; its SHA-256 companion identifies the
frozen archive. Receipt labels do not advance any milestone acceptance.

Development receipts preserve the original reproduced ownership failure,
the lower-test diagnostic compile error, profiler fixture/lint corrections,
the real non-dispatch release failure and the stale allocation-observer oracle.
These were corrected rather than excluded. The focused prepared selection
passed 13 tests after the release fix; final broad qualification includes those
tests and the corrected allocation observer.

## Open Boundaries

This checkpoint covers retry publication of an already indexed Prepared owner.
Initial bind/submit still has a separate pending-owner handoff. Its next fix
must index Prepared immediately after successful binding, before primary-lane
retention and first submit, then reuse the publication transition. The caller's
terminal branch must recognize that handoff and remove the pending FIFO entry
and dependency retains exactly once instead of reinserting a second root.
Pre-Active bind retry/terminal behavior must remain distinct and qualified.

Lower callback catches do not prove arbitrary unwind safety in every later
ledger operation. Native end-to-end qualification, allocator-exhaustion
injection, Context composition, generated DATA-ADOPT/ISSUE/COMPLETE,
pending-peer compute gates, executable formal correspondence, aggregate
residency, generated runtime profiling and matched HIP/HSA benchmarks remain
open. Generated release uses its own path and does not emit this ordinary
dispatch event. A1/A2 and broader accepted milestones are unchanged.

The MI300X probe failed DNS resolution for `sharkmi300x-1`; no remote files or
jobs were created. GitHub issue refresh also failed to connect. The local
milestone record is not a successful external issue-state revalidation.
