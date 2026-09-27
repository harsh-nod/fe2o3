# Pending Compute Settlement Custody

CPU developer evidence, not GPU execution, executable formal refinement, complete
#182/A1/A2 closure, HIP/HSA behavioral parity or a performance comparison.

Source baseline: `7236756c06f0799ea7072a8362dec2fd9f106d34`.
Signed source: `e7559c1efccba06a02d23590c74e202b515665d9`.
Source tree: `e059a26201d1d66c1fa9ad7c0b4f85b2c540b1b0`.

## Change

Unpublished compute cleanup previously detached the recipe and changed FIFO,
dependency and allocation/module accounting before checking every release
prerequisite. A new public cancellation regression reproduces a missing recipe
after an injected custody fault and cleanup panic. This is private metadata fault
injection after genuine admission, not an observed GPU failure.

Cancellation now settles while the recipe remains indexed. Failed dependency,
staging and peer-gate paths restore their temporarily detached owner before using
the same fallible settlement. A dedicated inline terminal slot retains the exact
detached recipe if its map key is occupied or fallible reindex reservation fails.
It neither replaces an existing owner nor clones the recipe. Drop and shutdown
explicitly account for that terminal root.

Before release, settlement validates the first-binding allocation projection,
local exact Compute memberships/count prerequisites, module/kernel/stream binding,
dependency uniqueness and disjointness, earlier dependency identities, retain
counts, cursor bounds, peer identity, same-stream ordered predecessor, FIFO
membership, completion reservation and result-key vacancy. Result capacity is
reserved before commitment. Dependency, allocation, module and FIFO release plus
result insertion are callback-free; the indexed recipe is removed last.

Cancellation authenticates FIFO membership before the existing interior-node
TooLate policy. Missing membership now seals the backend rather than masquerading
as a valid late cancellation. Healthy tail cancellation remains Failed(-2), and
failed unpublished execution remains Failed(-1). Published/prepared cancellation
semantics are not expanded by this change.

The predicate uses existing private monotone FIFO/owner-index invariants; it is
not a global owner/count conservation proof or arbitrary corruption detector.
Binding projection uses the bounded O(B squared) admission convention. Explicit
dependencies use fixed stack scratch and sorting; sorted quiescence and local
owner/FIFO lookups avoid scanning unrelated allocations. Reindex/result capacity
reservation may allocate before commitment. Existing custody removal and
sole-stream recomputation costs remain; no new throughput claim is made.

## Tests

Thirty-five child cases cover 19 tail-cancellation mutations, 15 public
failed-dependency settlement mutations and one private reindex collision.
They inspect the original recipe and boxed-roster addresses, module/dependency/
event retains, allocation custody, reservations, FIFO/tails, result records,
exact scripted SDMA pair owners and allocation markers, and SDMA accounting.
Repeated terminal ingress cannot release that custody. Each child then drops the
unrepaired backend and must abort with an inspection marker; core dumps are
disabled. Metadata and Weak snapshots cannot retain native owners or recipes.

The collision case starts with two independently admitted recipes, deliberately
moves one under the detached recipe's key and verifies both exact owners remain.
This exercises collision custody, not allocator exhaustion. Four poll faults
(cursor past the explicit roster, result collision and two missing-FIFO cases)
bypass settlement in current public polling; only cancellation coverage is
claimed for those variants.

Two healthy public controls cover cancellation and failed explicit dependency
without progressing the still-published ancestor. They then complete and release
all scripted resources. Existing interior-cancellation, stream-ordering, peer-gate,
mixed copy/compute and profiler regression tests remain applicable. Clean scripted
helpers disable device scrub-on-release; this is not hardware scrub qualification.

Older private scheduling fixtures now contain actual mock resource records with
distinct module/kernel IDs and normal release cleanup. Their hand-built receipts
remain synthetic, not native or full public-admission evidence. Newer-than-consumer
synthetic dependency IDs were corrected rather than weakening release checks.

## Qualification

Final source signature verification, formatting, no-default-feature runtime/model
checks, strict all-feature/all-target Clippy and unchanged source continuity passed.
The focused KFD suite passed 833 tests with no failures and 28 ignored before the
final test-only Clippy cleanups; the signed full run below includes these tests.

The signed full all-feature runtime/model command returned 101: runtime unit tests
had 1,730 passed, three failed and 28 ignored; model unit tests had 1,080 passed
and 19 ignored. Integration tests passed 11, with three hardware tests ignored.
Runtime doctests passed 52 (eight plus 44), and model doctests passed 29.

The three runtime failures remain explicit, not waived:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. All fail at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`, as in the
preceding qualification. The full suite is not green; these telemetry paths remain
unqualified in this environment.

`receipts.tar.xz` contains developmental failures, read-only review notes,
the full source patch, signature/identity receipts, commands, logs and exit
statuses. The companion SHA-256 file identifies its frozen bytes. Failed earlier
runs include a test-code integer mismatch, an incorrect scripted-step assertion,
legacy fixture inconsistencies and lint diagnostics, all resolved in the signed
qualification except the three telemetry failures named above.

## Open Boundaries

Prepared cancellation still removes the outer runtime active descriptor before
consuming native cancellation/restoration. Lower KFD cancellation retains its own
native prefixes, but that does not prove outer descriptor retention across every
runtime error/unwind. Runtime single-input scripted Prepared tests exist;
three-binding scripted preparation currently proceeds straight to Published and
needs a distinct prepared-state test boundary.

Other detached publication/observation paths, complete queued-owner/arena proofs,
Context release composition, native hardware admission/currentness, Worker and
distributed milestones, and matched HIP/HSA benchmarks remain open. See the
[successor plan](../../runtime-successor-writer-plan-v1.md).

MI300X SSH and both GitHub pushes failed DNS. No remote jobs or files were created,
no permission escalation was requested, and unrelated local evidence was retained.
Post-freeze publication attempts are recorded separately.
