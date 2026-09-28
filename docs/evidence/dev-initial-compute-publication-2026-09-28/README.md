# Initial Compute Publication Custody

CPU developer evidence, not GPU execution, executable formal refinement,
complete #182/A1/A2 closure, HIP/HSA parity, or a performance comparison.

Baseline: `36286c4b63c94e79a264adbf615005f9af0c8136`.
Signed implementation: `72113063d606baa2af480292914d8cef86184d61`.
Source tree: `75fe8e2c875bbbabaca8ec75644575d73e3d6d43`.

## Change

Single- and three-binding persistent publication now install the successful
bind's Armed receipt in the active runtime descriptor before registering the
primary queue lane or consuming the receipt. Initial submission and retries
use the same indexed publication transition. Exact lower retry receipts re-arm
the owner, terminal/unwind paths retain NativeOwned custody, and success installs
Published before dispatch profiling. Single-binding native errors retain their
diagnostic detail, now formatted after custody is indexed.

The caller recognizes the transfer on returned errors as well as unwind. It
removes the pending FIFO entry and releases the pending dependency retains once,
without reinserting a duplicate pending root or settling Active as unpublished.
Unexpected post-index Rejected/Quiescent results become Terminal. The original
panic payload survives terminal poisoning. Pre-Active bind failure handling is
unchanged and remains a separate qualification boundary.

This removes duplicate initial-submit logic and introduces no production heap
allocation for the indexed handoff. The scripted single-input path reserves its
input shell before taking storage custody. The shared preflight visits at most
three bindings and their indexed membership lookups; no speedup is measured.

## Coverage

The fixture uses a public scripted H2D copy and its completed event as an
explicit dependency. This prevents eager submit from bypassing first-flush fault
injection. A trailing launch shares the dependency and buffers. Before flush,
the FIFO is `[first, trailing]`, the copy dependency has two compute retains,
and `first` has one retain from its successor. After handoff only `trailing`
remains pending, the copy has one compute retain, and the successor's retain on
`first` remains intact. Compute inputs are seeded scripted owners, not an actual
GPU upload witness.

Twenty-four subprocess cases cover one/three bindings, authenticated-ready/
initialized-storage origins, terminal submit, consuming unwind, published-profile
unwind, initial observer unwind, and inconsistent post-index Rejected/Quiescent
outcomes. Assertions check exact Armed/NativeOwned/Published phases, input owner
IDs, backing addresses/digests/certificates, allocation custody, reservation and
module/event/dependency retains, stream tail/lane/FIFO, and the unchanged trailing
recipe and roster addresses. Terminal ingress cannot release ownership. Every
unrepaired backend must SIGABRT only after the custody inspection marker; core
dumps are disabled.

Twelve healthy controls cover initial success, initial retry then completion,
and initial retry then cancellation for both arities and origins. Initial success
must already be Published without another poll. Cleanup checks zero live scripted
owners, zero unexpected owner drops, exhausted scripts, and empty custody and
retain ledgers. Existing publication, prepared cancellation and profiler controls
remain part of broad qualification.

The observer-unwind hook models a callback at the post-index boundary; it does
not execute native queue registration. Production source ordering establishes
that Active precedes `retain_primary_compute_lane_v1`. The Rejected/Quiescent
hooks test inconsistent post-index classifications, not native bind failure.

## Regression Evidence

`actual-before-handoff` uses the corrected dependency fixture and the indexed
initial publication with the old caller. It fails because a terminal first-submit
result leaves the same submission both Active and pending. Its healthy controls
pass. The captured diff and new test source preserve that intermediate state.

Earlier `before-fix`, `before-handoff-fix` and `diagnose-handoff` receipts used an
invalid fixture: the fault was armed after an eager submit. Those failures are
retained but are not evidence of a custody regression. The fixture was corrected,
not the production eager-admission behavior.

## Qualification

The `final-` receipts identify commands run against the signed source above.
The all-feature runtime/model command returned 101:

- Runtime unit tests: 1,737 passed, three failed, 28 ignored.
- Runtime-model unit tests: 1,080 passed, 19 ignored.
- Integration tests: 11 passed, three hardware tests ignored.
- Runtime doctests: 52 passed (eight plus 44); model doctests: 29 passed.
- Separate publication selection: 39 passed, none ignored, including all new
  subprocess and healthy controls.
- Separate lower KFD `persistent_` selection: 166 passed, none ignored.

The three failures remain `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each reporting
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`. The full
suite is not green; these verification requirements are not waived.

Strict all-feature/all-target Clippy for runtime, runtime-model and KFD passed.
The runtime no-default-feature check, formatting check, source signature and
source-continuity check passed. `receipts.tar.xz` preserves command lines, logs,
UTC start/finish times and exit statuses, including unsuccessful
development attempts. Its SHA-256 companion identifies the frozen archive.
Receipt labels do not advance milestone acceptance.

## Open Boundaries

Native bind retryable-input restoration, bind terminal capsules and internal
lower-bind unwind are not qualified by the new fault matrix. Coupled native
queue execution, allocator exhaustion, full Context composition, generated
DATA-ADOPT/ISSUE/COMPLETE, generated profiling, aggregate residency, formal native
correspondence and matched HIP/HSA benchmarks remain open. Accepted milestones
and A1/A2/HIP/HSA parity status are unchanged.

The next concrete recovery fix is single-binding retry restoration: H2D-ready,
device and replay paths still allocate a Box after receiving typed retryable
custody. Reserving origin-compatible restoration shells before extraction would
remove that allocation tail. Three-binding and single initialized-storage
restoration already preallocate their shells; this change must preserve their
existing source/custody checks.

The MI300X probe failed DNS resolution for `sharkmi300x-1`; no remote files or
jobs were created. This checkpoint supplies no new hardware or copy-performance
measurements and is not an external issue-state revalidation.
