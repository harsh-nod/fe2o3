# Shared Publication Receipt Cell

Development implementation and CPU qualification only. Full runtime/native
composition, formal refinement, protected Worker and HIP/HSA parity remain open.

## Source

- Signed implementation: `ccc0bd278a0bc0aca1868d528edff336ff211b0f`.
- Source tree: `2845c25cb5637e5dbae1b88dca8a430ff411ac7c`.

The new private `materialized_submission_attempt.rs` is used by runtime initial
binding, Prepared retry and ordered-successor publication. KFD tests compile that
same file against the real lower receipt types, without a reverse Cargo dependency
or public receipt constructor. KFD's existing private-to-public classified-error
conversion is also shared with these tests, not copied into fixtures.

The closure-taking API installs NativeOwned before evaluating submission,
stores a returned batch as Published, stores only confirmed pre-side-effect
retry as Retryable, and propagates rejection/terminal errors unchanged. The two
runtime native call sites still invoke it inside their lane callbacks. The
cell does not itself confer authority to withdraw an indexed owner: callers
still require successful outer close and all existing custody checks.

## Qualification

All commands, timestamps, logs, exit statuses, source identity and exact mutation
diffs are retained in `receipts.tar.xz`, checked by
`receipts.tar.xz.sha256`. Builds and tests are serialized with
`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`.

| Final-source check | Result |
| --- | --- |
| Shared receipt-cell selection | 6 passed |
| Full KFD library suite | 1703 passed, 1 failed, 0 ignored |
| Materialized runtime selection | 35 passed |
| Full runtime library suite | 1794 passed, 3 failed, 28 ignored |
| Backend subset parsed from full runtime log | 897 passed, 0 failed, 28 ignored |
| Three compiled production-body mutants | Detected by intended behavioral assertions |
| Strict all-feature/all-target runtime and KFD Clippy | Passed |
| Runtime and KFD no-default-feature check | Passed |
| Workspace formatting | Passed |
| Mutation-receipt audit and source continuity | Passed |

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

These match the earlier packets' failure boundaries; no test or admission check
was skipped or relaxed. The full KFD harness reports 3751.08 seconds, and the
runtime harness reports 193.08 seconds. These are local debug test-suite timings,
not runtime/GPU performance measurements. The same live qualification process
was retained to completion without restarting it for slow output.

The source is restored byte-for-byte between mutants and before final
qualification. The backend subset is extracted from the full runtime log, not a
separate test invocation. These selections overlap and cannot be summed.
Runtime-model, Verus, runtime doctests and profiler-protocol tests are outside
this packet.

## Receipt Controls

Six registered groups comprise fourteen primary/AUX lane/outcome workflows and
source-wiring smoke checks. The lower fixtures use the actual dispatch generation
owner, completion arena and typed batch/receipt APIs, with injected native submit
and CPU completion observations:

- Three retained batches are completed/recycled out of order, checking exact
  live-epoch and signal counts plus untouched-lane custody.
- A full completion arena returns Retryable without entering native submission;
  its exact custody is unchanged, the dispatch generation is burned once, and
  release permits a successful new publication and complete recycle.
- The actual public no-dispatch entry returns rejection, which is not swallowed
  as a retry. The cell remains in consuming state; the lower session remains
  unpoisoned. Upper runtime terminal policy is tested separately.
- An injected typed terminal result retains consuming state, the live lower epoch
  and occupied signal, records the process-gate action and prevents reentry.
- Actual primary/AUX lane callbacks unwind after storing Published, after storing
  Retryable, or while native submission is consuming. Tests check original panic
  payloads, exact retained custody, stable lane indexing and terminal refusal.

Healthy controls explicitly recycle every fixture receipt. Terminal controls
never clear poison or invoke CPU cleanup helpers to imply native recovery.
The fixtures have no GPU resources; their Drop or failed destroy is not healthy
native teardown. AUX unwind restores AUX before poisoning the restored primary
owner, so the test does not incorrectly require AUX's individual completion
owner to be poisoned. Snapshots cover slot records, identity, counters and backing
pointers, not the complete dependency ledger.

These are genuine typed CPU receipt/accounting controls, not GPU dispatch,
native completion loads, code/kernarg/memory authority, full runtime Pending
handoff, or allocation/latency benchmarks. The source-wiring check tests use of
the shared classifier; it does not prove callback placement or caller refinement.

## Mutation Controls

Each mutation changes the shared production cell and compiles successfully:

| Mutation | Behavioral result |
| --- | --- |
| Omit the pre-call NativeOwned marker | Rejection, typed-terminal and consuming-unwind assertions fail |
| Treat RejectedBeforeSideEffect as Retryable | Rejection test fails because an error is swallowed |
| Discard the returned outcome | Published, Retryable and returned-unwind custody assertions fail |

The mutated selections return 101 with respectively 3, 1 and 3 failed tests, not
compiler failures, aborts or timeouts. Loops stop at their first failure, generally
on primary; do not treat this as separate mutant-specific AUX qualification.
The restored final source passes all six tests. These results establish test
sensitivity, not formal verification or complete source correspondence.

Initial formatting failed because the inline test module resolves under
`src/tests`; the file/include paths were corrected before the passing baseline.
No production guard was bypassed to repair that setup issue. The unrelated
untracked owner-inspection packet and all earlier frozen evidence were preserved.

## Remaining Gates

Next, share concrete completion receipt storage/classification with lower receipt
tests and refine staged pipeline metadata separately from confirmed logical
epochs. Share complete indexed scans and stage/confirm/withdraw mutations, rather
than proving a classifier supplied trusted roster-validity booleans. Completion
controls must exercise actual Pending/Ready and pin-refusal primitives, retaining
the private lower receipts and exact target association. R60's independent proof
does not model physical staging, returned-state custody through outer-close
failure, explicit versus deferred dependency retains,
or optional profiling after logical commit. Its historical capacity is 64, not
the 1024-slot development profile. Preserve those boundaries instead of treating
this receipt cell or the old model as full runtime refinement.

MI300X SSH failed name resolution before connection; no remote jobs or files were
created. Both source pushes, to `origin` (harsh-nod) and `upstream` (powderluv),
returned 128 because `github.com` could not be resolved. No remote synchronization
is claimed. Native R125, Admission R118B, Resources R116/V3 and A0-A7 acceptance
statuses are unchanged.

The archive also contains read-only review findings and next-work boundaries.
It is frozen after qualification and source-push recording; subsequent
documentation-commit publication is outside this archive. Verify the archive from
this directory with `sha256sum --check receipts.tar.xz.sha256`.
