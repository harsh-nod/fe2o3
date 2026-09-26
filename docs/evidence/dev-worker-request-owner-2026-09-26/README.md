# Worker Server-Local Request Ownership

Development evidence on parent `43cae16075ddfb5f8365efb3722a4de8652b8faa`.
This supports composed request accounting inside explicit Worker servers. It
does not close A1/A2, issue #182, Worker V3 compiler/application authority or
HIP/HSA parity. There is no new hardware, formal-refinement or performance result.

## Architecture

`RuntimeWorkerRequestOwnerV1<B>` owns an actual private `RuntimeContextV1<B>`
and a raw-handle index. Construction reuses complete Required-profile admission;
Legacy and empty profiles reject. Returned errors preserve the original backend.
Constructor panic retains Context's existing unwind/Drop behavior, not a new
recoverable native-construction guarantee. Preexisting allocations are not adopted.

The owner reuses Context's existing allocation/release state machine instead of
duplicating request reservation, exact witness authentication or settlement.
Its raw-handle index is reserved before Context allocation. Native success and
credit custody are rooted before encoding/writing a response; invalid zero or
duplicate handles are retained by the sealed Context without publishing an index.
Release removes the index only after confirmed disposal. The bounded hash index
avoids scanning all live allocations on each access or release.

The opt-in `serve_runtime_request_owner_v1/v4/v5` APIs borrow the persistent owner.
Other operations still use canonical backend dispatch. All allocation references
in read/write, ordinary submit, peer copy, async copy and typed atomic/collective
submissions are checked after full decoding and before backend entry. Existing
unscoped servers pass no scope, and enumeration serialization is a byte-equivalent
extraction. This adds no wire opcode, response tag or implicit V1 progress support.

I/O/EOF/protocol failures, terminal responses and serving panics seal the owner,
quarantine every local retained credit and prohibit later backend entry. Records
and raw mappings remain owned; this is not cleanup. Original panic payloads are
preserved. A lost release response after confirmed native disposal refunds only
that local request; other requests quarantine and a remote client remains uncertain.

`try_into_backend` is ownership transfer, not native teardown. It rejects local
records, terminal state or unhealthy Required sessions, including an unidentified
generic-Quiescent allocation attempt. Healthy external reservations or retentions
alone do not block transfer. The private Context does not inventory directly
dispatched Worker streams, modules or submissions, so its general `shutdown`
method is not used as a complete Worker cleanup oracle. Empty shutdown frames end
framing only. Recovered backends still require their explicit native teardown.

Server accounts and borrowed witnesses never cross the wire. Host and worker
roots remain independent. Local settled-no-owner refunds still encode the frozen
generic Quiescent tag, not a stronger cross-process settlement claim. Existing
unwrapped servers continue rejecting witness-free composed allocations.

## Qualification

Production tests cover Required-only open rejection with backend recovery,
terminal-aware error classification, scope-helper behavior and the existing
canonical Worker codec/dispatch/child-process suite. Doctests check public V5
usage and V1's compile-fail immediate-progress boundary.

The isolated qualification copy adds private accounting/model fixtures from the
pinned single-device packet and one new test module. It executes the actual public
owner and V1/V4/V5 serving bodies through framed in-memory I/O and a scripted
backend, with real typed request credits. It does not mint checked-device or
native authority, execute KFD/native constructors, or run a native worker process.

Eleven new fixture groups cover:

1. Missing, empty, extra, duplicate, aliased and unhealthy Required rosters,
   preserving the original backend on returned errors.
2. All three exact handshakes, immutable cached enumeration, both endpoint
   witnesses/charges, occupied transfer rejection and clean release/transfer.
3. Unsupported, Rejected, SettledNoOwner, generic Quiescent, Terminal and zero
   handle allocation results with exact wire tags and refund/quarantine phases.
4. Rejected/Quiescent release retry and Terminal retention of both endpoints.
5. Allocation response header/payload/flush loss: exact bytes/record charges,
   retained handle set, all-credit quarantine and no cleanup or replay.
6. Lost successful release response: only confirmed disposal refunds.
7. EOF, truncated frame/header, unknown opcode and trailing bytes: no backend
   effects and quarantined root survival after ordinary owners disappear.
8. Allocation/release/direct-dispatch panics with original payload identity,
   plus direct terminal response quarantining every active request.
9. Duplicate successful backend handles retain both Context records.
10. Invalid device/extent, request capacity and unknown release reject before
    backend effects; healthy external credits do not block backend transfer.
11. Foreign copy source/destination and first/middle/last launch bindings reject
    across all seven memory-reference families; valid references dispatch, and
    previously valid references reject again after release invalidates the index.

The inherited 14 typed single-device/generated groups also run. Source closure
checks all copied Cargo/toolchain/crate/example inputs, rejects nonordinary nodes,
pins inherited fixture hashes and permits only exact recorded test overlays.
This is development replay evidence, not an independently authenticated proof.

## Results

| Gate | Result |
| --- | --- |
| Production Worker focused filter | 75 passed |
| Runtime all-feature library suite | 1,508 passed, 3 failed, 28 ignored |
| Runtime doctests | 52 passed |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-feature check, formatting and whitespace | Passed |
| Isolated typed qualification | 25 passed (14 inherited, 11 new) |
| Scope-forwarding mutation | Intended foreign-reference assertion failure, exit 101 |
| Sealing/quarantine mutation | Six intended custody assertions fail, exit 101 |
| Exact source/fixture restoration | Passed |
| Restored isolated qualification | 25 passed; original executable SHA-256 reproduced |

Counts overlap and must not be summed as distinct-test totals. The three known
library failures are `authorized_execution::tests::{cooperative_debug_telemetry_emits_only_bounded_logical_records,
failed_session_end_is_explicit_and_terminal, pre_native_telemetry_failure_is_returned_and_poisoned}`:
each reports `InspectSocket` with `Operation not permitted`. They are failures,
not passing qualification. The initial isolated compilation error used a nonexistent
generic semantic codec method; it was corrected to the actual typed atomic and
collective encoder methods. Its diagnostic is retained separately.

Both deliberate mutations compile and fail on runtime assertions, not on compiler
errors. They are applied only to the isolated copy, never the shipped source.
`qualification/mutation-*.patch` records the changes; observed deltas, executable
hashes, command statuses and replay logs are in `raw/`. The scope observed delta
also includes its test-module overlay. Large intermediate lists/hash-check logs
are archived with deterministic gzip; replay regenerates uncompressed files.
`raw/sources.sha256` remains directly checkable.

After every build/test process terminated, both owned temporary trees were
removed: the production target (553,820 KiB) and isolated qualification copy
(513,012 KiB at final cleanup). `raw/cleanup-absence.log` confirms neither path
remains. No files belonging to other jobs or users were removed.

## Remaining Gates

- Native composed single-device/multi-device/XGMI worker deployment and process
  failure replay, checked constructor cleanup, device execution and native shutdown.
- Worker V3 semantic-to-machine/application authority, cross-process shared-root
  accounting, replay epochs and stronger allocation-settlement acknowledgments.
- Production Context/worker/backend/ledger refinement and concurrent sealing
  exclusion. No new Verus proof ran; lower KFD/model/accounting sources are unchanged.
- Full memory closure, native overlap/scaling and matched HIP/HSA measurements.
- SSH still cannot resolve `sharkmi300x-1`; issue retrieval cannot connect to
  `api.github.com`. No remote artifacts or new issue-state evidence were created.

## Reproduction

```sh
bash docs/evidence/dev-worker-request-owner-2026-09-26/validate.sh /tmp/fe2o3-owned-target
bash docs/evidence/dev-worker-request-owner-2026-09-26/qualify.sh /tmp/fe2o3-unused-copy
```

Use unused owned paths and remove only those paths after all processes finish.
