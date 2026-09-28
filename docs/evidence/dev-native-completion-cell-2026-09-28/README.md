# Shared Completion Receipt Cell

Development implementation and CPU qualification only. Full runtime/native
composition, formal refinement, protected Worker and HIP/HSA parity remain open.

## Source

- Signed implementation: `6455855305d5000806cb23bca0264201cc454008`.
- Source tree: `6f60bcc24bbe77a40cb881de91e83346930bc05a`.
- Signed source-shape test correction: `108dfa8b1ed1b1ccb9113dac7f4932789721755e`.
- Corrected source tree: `2261083fa40ed093d8c0d6ebc5e0d8a48d727a45`.

The private `materialized_completion_receipt.rs` now stores Published, Completed,
Retired and Consuming(Poll/Recycle) receipts inside the actual runtime Active
execution state. KFD tests compile the same source against real lower receipt
types. The private lower completion-failure classifier is shared with production:
for errors from that completion primitive, only SignalPinned returns completed
custody for retry.

Poll/recycle reject an invalid phase before consuming or invoking the lower
operation. A consuming marker is installed before the call, and every returned
receipt is stored before control leaves the native lane callback. At the runtime
cell boundary, returned completed custody authorizes retry regardless of the
diagnostic; absent custody propagates failure. Native errors and unwinds never
fabricate a reusable receipt. Runtime timing, indexed selection, accounting and
logical commit remain in the existing adapter. Persistent scalar/three-binding
paths are unchanged. No public receipt constructors or new Cargo features were
added.

## Qualification

The full qualification run on corrected source `108dfa8b1` is finished. Commands, UTC
timestamps, logs, exit statuses, toolchain identity, source signatures and exact
mutation diffs are retained in `receipts.tar.xz`, checked by
`receipts.tar.xz.sha256`.

| Corrected-source check | Result |
| --- | --- |
| Shared completion-cell selection | 9 passed |
| Corrected source-shape oracle | 1 passed |
| Full KFD library suite | 1712 passed, 1 failed, 0 ignored |
| Materialized runtime selection | 35 passed |
| Full runtime library suite | 1794 passed, 3 failed, 28 ignored |
| Backend subset parsed from full runtime log | 897 passed, 0 failed, 28 ignored |
| Strict all-feature/all-target runtime and KFD Clippy | Passed |
| Runtime and KFD no-default-feature check | Passed |
| Workspace formatting | Passed |
| Earlier mutation-receipt audit and source continuity | Passed |

The aggregate qualification exits 1, not success. Four telemetry failures remain
unwaived:

- KFD `target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
  fails at `target_debug_telemetry_v2.rs:1173` with `SocketAdmission`.
- Runtime `authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records`
  fails at `authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
- Runtime `authorized_execution::tests::failed_session_end_is_explicit_and_terminal`
  fails at the same socket-inspection boundary.
- Runtime `authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned`
  fails at the same socket-inspection boundary.

The source-shape regression is resolved in the corrected full run; no test or
admission check was skipped or relaxed. The KFD harness reports 3724.01 seconds
and runtime reports 203.30 seconds. These are debug CPU test-suite timings, not
runtime/GPU performance measurements. Focused/full/subset results overlap and
cannot be summed. The backend subset is parsed from the full log, accounting for
two subprocess tests whose successful results appear on following lines; it is
not a separate invocation. Runtime-model, Verus, doctests and profiler-protocol
tests are outside this packet.

Builds/tests use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`. No implementation source
edits are made while qualification is live. Earlier packets are frozen and not reused as
current-source full-suite evidence.
Both full qualification drivers run commands sequentially and each live process
was retained until terminal. A preflight formatting check overlapped the corrected
oracle's compilation without source edits; both finished before the corrected
commit. That overlap is recorded in `review.md` rather than represented as a
serialized preflight. Continuity guards cover tracked crates/tools/Cargo files,
not unrelated untracked evidence or whole-worktree cleanliness.

The first full run on `6455855` is preserved: KFD 1711 passed and 2 failed;
runtime 1794 passed, 3 failed and 28 ignored; focused selections and static checks
passed. In addition to four telemetry failures, a source-shape test still required
the recycle predicate inline after its extraction. The correction updates only
that oracle to inspect both the exact pinned-only shared helper and native caller
custody handling, retaining stale-generation checks. Its focused test passes.
Production bodies and the nine receipt tests are unchanged. The original result
is not replaced or represented as a passing full run.

## Receipt Controls

The nine groups cover both primary and AUX lanes using the production completion-slot
owner and dependency ledger with synthetic mapping metadata, actual dispatch
generations, event record/bind/release, `observe_once_with_progress` and
`recycle_retaining`. CPU currentness, observation and reset results are
injected; exact signal-slot call prefixes, including post-observation/reset
currentness and panic prefixes, are asserted.

- Repeated Pending, then Ready and Retired, preserve original occurrence identity.
- Genuine event pin refusal makes no native observation/reset call. Releasing
  the event permits retirement and reuse with advanced physical/dispatch identity.
- Wrong-phase calls preserve the receipt and never invoke the callback.
- A facade diagnostic control containing a genuine returned completed token
  checks that the cell's retry decision depends on custody, not error spelling.
- Opening, operation and closing failures preserve consuming state and fail closed.
  A reset may have taken effect before error; no safe retry is inferred from it.
- Signal callback and outer lane unwinds retain the exact returned or consuming
  state, restore lane indexing and refuse reentry.
- Three in-flight receipts occupy slots 0/1/2 with distinct packet IDs. A remains
  Published and C Completed while B retires; D reuses B's slot with a new
  generation. A/C event identities and the other lane remain unchanged, followed
  by healthy D/C/A retirement and drained ledgers.

Healthy controls explicitly drain receipts, pins and ledgers. Terminal controls
never clear poison or imply native cleanup. The external test dispatch owner's
poisoning on outer unwind is harness bookkeeping, not proof that production
poisons a restored AUX dispatch owner. Actual lane restoration, retained cells,
slot snapshots and session-wide refusal are observed. Snapshots do not include
the complete dependency ledger or arena phase.

## Mutation Controls

All six isolated mutations compiled on `6455855` and failed at intended
behavioral assertions. They were not rerun on `108dfa8b1`, which changes only the
separate source-shape test; production bodies and the nine receipt tests are
unchanged:

| Mutation | Failed tests |
| --- | ---: |
| Discard returned Pending receipt | 2 |
| Discard returned Ready receipt | 8 |
| Discard returned retry-completed receipt | 3 |
| Discard Retired observation | 6 |
| Swallow terminal failure without returned custody | 1 |
| Return completed custody after lower Recycle failure | 1 |

Each mutated nine-test selection exits 101, not a compilation failure, abort or
timeout. The Retired mutant emits an expected unused-variant warning but compiles.
The exact baseline is restored between mutations. Failure loops generally stop
on primary; this is not separate mutant-specific AUX qualification. Mutation
sensitivity is not formal verification or full caller correspondence.

Initial strict Clippy rejected the large inline retry-failure type in new
closures. Narrow `result_large_err` allowances preserve the existing move-only
representation rather than adding heap allocation; no behavioral guard or test
was relaxed. Read-only reviews also prompted exact signal-slot/panic-prefix
assertions and the distinct-packet, three-inflight reuse control.

## Remaining Gates

These tests do not execute Linux signal loads, GPU submission/completion,
code/kernarg/data authority, native teardown, or full runtime Active/Pending
handoff. Runtime successful completion fixtures still use scripted states; new
nested selectors, predecessor eligibility and timestamp wiring are inspected and
compiled, not behaviorally established by the lower-cell tests. The diagnostic
control does not execute native facade preselection. No allocation/latency or
matched HIP/HSA performance claim follows from these receipts.

Next, couple genuine receipts through actual runtime selection, Pending
publication and logical commit without exposing unscoped native authority or
adding a production CPU fallback. Separately, share complete staged-metadata
scans and stage/confirm/withdraw mutations with proofs over actual borrowed slots
and scalar heads. Preserve opaque Active payloads, exact refusal framing and
64/1024 capacity/exhaustion behavior. Current metadata guards are not comprehensive
corruption validators; invariant preservation and exact rejection predicates
require separate contracts. R60's independent 64-epoch model does not prove these
production transitions or the 1024-slot development profile.

MI300X SSH failed name resolution before connection; no remote jobs or files were
created. Source pushes to both `origin` (harsh-nod) and `upstream` (powderluv)
returned 128 because `github.com` could not be resolved. The fresh issue API read
also failed connectivity; a cached browser page was not used to refresh the
milestone observation date. No remote synchronization is claimed by these source
push receipts. Native R125, Admission R118B, Resources R116/V3 and A0-A7 acceptance
statuses are unchanged.

The archive is frozen after qualification, review and source-push recording.
Subsequent documentation-commit publication is outside this archive. Verify it
from this directory with `sha256sum --check receipts.tar.xz.sha256`.
