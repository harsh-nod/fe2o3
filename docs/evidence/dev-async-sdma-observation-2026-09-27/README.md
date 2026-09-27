# Indexed Asynchronous SDMA Observation

CPU developer evidence over scripted native owners. This is not GPU execution,
formal unwind proof, complete #182/A1/A2 closure, or HIP/HSA parity/performance
evidence. The full runtime objective remains open.

Final signed source: `d5186315fbfcb552c0bcb4cfa9bee01a5b969116`.
Source tree: `d8d3a72d32984eab4e81dfb51705a986cc7d79c9`.
Production fix: `231d90317e2a60dab94f6cce37c0dc3ca0c80ca3`.
Production tree: `54a5e681fc2dac935c0f16bf04bac7c1527a4649`.
Baseline: `27288c65bbf36542a8fbc214f0ad04cd8ae50ad5`.
Both source signatures verified for `harmenon@amd.com`.

## Reproduced And Fixed

Public asynchronous copy observation removed its accepted descriptor before
passing a published owner to the lower layer. The existing scripted retirement
panic retains both native buffer owners, but the runtime lost the logical copy
descriptor while its FIFO, allocation custody, dependency/event retains and
completion reservation remained live. Ordinary terminal observation/retirement
errors had the same logical-record gap. This is not evidence of GPU memory
being freed while active.

The shared poll/wait observer now leaves the exact descriptor in `active_sdma`.
It replaces only the native phase with `Quarantined` before handing custody
downward. Pending restores the returned published owner in place; terminal
failure leaves the descriptor indexed, without a recovery insertion. Unwind
poisons the backend and resumes the original payload. Native-owner panic safety
still depends on the lower implementation retaining the owner it consumed.

Ready dependency observers also keep their descriptors indexed across recursive
polls. This preserves every ancestor in a nested observation without a separate
single-slot terminal root or failure-path allocation. Poll/wait diagnostics and
normal status/error classification remain unchanged.

Completion restores native owners before committing logical release. It checks
the local dependency, allocation-owner, stream, completion-slot and reservation
prerequisites before releasing any of those logical retains. The descriptor is
removed last. Partial windows advance in place and remain Ready for explicit
flush. Bounded dependency uniqueness uses a stack-backed sort; allocation and
stream lookup use binary search rather than full-queue scans. These searches
rely on the existing private monotone-index invariant, not a new proof of global
index consistency or arbitrary memory-corruption detection.

## Coverage

The failure matrix has 48 subprocess cases: H2D/D2H, public poll/wait, direct or
two genuinely admitted same-stream dependents, and six failures (retirement
panic, retryable retirement, retirement teardown, malformed completion metadata,
observation retry and observation teardown). Nested waits poll their earlier
dependencies internally; they are not claimed as native waits on those roots.

Snapshots check descriptor identity fields, dependency backing addresses,
published request metadata, FIFO/tails, logical retains, reservations, exact
buffer-owner IDs and in-flight markers. The failing published copy remains
Quarantined; unstarted dependents remain Ready behind terminal ingress. Repeated
poll, wait, flush, cancel and event release are inert.

Nine additional subprocess cases deliberately corrupt private state only after
genuine copy/event admission: a later dependency retain, destination owner,
source owner count, stream membership, reservation, duplicate dependency,
duplicate stream ID, duplicate allocation owner, and dependency result. They
check no logical release prefix, exact restored native owners, and terminal
custody. These mutations are not presented as reachable public admissions.

All 57 failure children inspect custody, disable core dumps, then explicitly
drop the unrepaired backend. Parents require the inspection marker and SIGABRT;
there is no new failure-fixture repair or Drop disarm. Metadata snapshots cannot
themselves keep native owners alive. A separate healthy control completes three
same-stream copies successively, proving each release preserves later owners,
roster backing, FIFO order and reservation counts, then uses the established
successful scripted-fixture cleanup.

Existing directional/same-device, promotion, partial-window, Pending and timeout
tests remain in the qualification. Eight older terminal tests now require a
Quarantined descriptor instead of its absence; their native-owner checks remain.

## Qualification

Final signed-source command, without skipped failing targets or concurrent
compilation/proof campaigns during Worker deadline tests:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test \
  -p fe2o3-runtime -p fe2o3-runtime-model --all-features --no-fail-fast
```

- Runtime unit tests: 1,721 passed, 3 failed, 28 ignored.
- Model unit tests: 1,080 passed, 0 failed, 19 ignored.
- Runtime integration tests: 11 passed, 3 hardware tests ignored.
- Runtime doctests: 52 passed. Model doctests: 29 passed.
- Strict all-feature/all-target runtime/model Clippy passed.
- Runtime/model no-default-feature compilation and formatting checks passed.
- Final source continuity against the signed commit passed.

The full command exits 101. The unchanged failures at
`authorized_execution.rs:1317` report `InspectSocket(PermissionDenied)`:
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`.
They are failed, not waived; no permission or escalation was requested.

Receipts retain development failures, including the initial borrowed-snapshot
compile error, the reproduced missing descriptor, the cooperative caller's
signature update, and an old test requiring descriptor removal. The focused KFD
run passed 457 tests before the final healthy control was added. `qualified-test`
records the production checkpoint; `final-controls`, `final-*` and `no-default`
bind the completed source checkpoint and its qualification.

## Remaining Work

Immediate and flush-driven publication still detach descriptors before native
submission. Existing scripted `Submit(ProcessTeardown)` can demonstrate that
separate loss through public APIs; preserving publication custody is the next
implementation task. Unpublished Failed/Quiescent cleanup, complete owner-arena
invariants, release/disposal composition, and opaque native unwind boundaries
also remain open. No new Verus proof or matched HIP/HSA measurement ran here.
The wider Worker/device-language/multi-device/memory/atomic/collective/profiling
gates remain open; see the [successor plan](../../runtime-successor-writer-plan-v1.md).

MI300X SSH and source pushes to both GitHub remotes failed DNS resolution.
No remote jobs or files were created. Unrelated owner-inspection evidence was
preserved. Evidence-commit publication is attempted separately after freezing
this packet. `receipts.tar.xz` contains commands, statuses, timestamps, logs,
source patches/identities/signatures and read-only review notes; its companion
SHA-256 identifies the archive.
