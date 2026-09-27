# Queued Producer Integration Checkpoint

CPU developer evidence only. No GPU execution, formal refinement, native memory
ordering or HIP/HSA performance claim follows from this packet.

Signed source: `75dc08e4b2092e632951989c55451c06703b1df4`.
Source tree: `0e55dafb2a75a95d903acc6fdb0d090631e05f7c`.
Parent: `9dbac2d1da795b01ecc92e7e99fcac0736ea0bfc`.
SSH signature verified for `harmenon@amd.com`.

## Integration Coverage

This source change adds tests and fixture support, not new production behavior.
It qualifies the queued-output read implementation from signed source
`a89c3ffab1a0ea645a8969dc28e5b7a10e2469f4` across two adapter boundaries.

Journal-enabled Context drives the real KFD backend implementation over five
scripted persistent owners. A writes x, B overwrites x, and C reads B's output.
Same-stream and cross-stream tests check exact dependency/retain accounting,
public-event release, delayed logical reconciliation, the same x owner identity
through A/B/C, and B's distinguishable bytes in C's actual input storage. They
verify no user-data materialization on this scripted path, all native
retain/reservation release, allocation-credit conservation and exact recycling.

Cancellation tests check that a cancelled consumer preserves its ancestors, a
cancelled middle writer never reparents its reader, and same-stream interior
cancellation remains TooLate until its downstream reader is cancelled. A failed
reader's Unknown output writer remains retained until its allocation is disposed.
Cleanup disables native SDMA synchronization; it tests scripted demotion/recycling,
not actual GPU readback or native teardown.

Frozen async drivers cover ordinary, tracked and event-producing enqueue APIs,
exact A -> B -> C dependencies, once-only argument capture, observer loss,
preissue cancellation and producer-first logical reconciliation. A real non-Send
owner-thread test covers live and orphan consumer observers, retirement of all
reply cells before shutdown, complete cleanup, and owner-thread-only backend
calls. Its MockBackend validates orchestration using supplied consistent terminal
facts, not kernel execution, memory effects or native scheduling.

Two read-only agent reviews found no blockers. Review suggestions became exact
owner-identity/credit assertions, pre-shutdown reply retirement and an exact
pre-submission cancellation-result assertion. Review is not proof evidence.

## Validation

Final all-feature command ran against the signed source, without skipping a
failed target or concurrent compilation during Worker deadline tests:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test \
  -p fe2o3-runtime -p fe2o3-runtime-model --all-features --no-fail-fast
```

- Runtime unit tests: 1,716 passed, 3 failed, 28 ignored.
- Model unit tests: 1,079 passed, 0 failed, 19 ignored.
- Runtime integration tests: 11 passed, 3 hardware tests ignored.
- Runtime doctests: 52 passed. Model doctests: 29 passed.
- Focused queued-path development suite: 37 passed.
- Real owner-thread case: 50 consecutive pre-freeze repetitions passed, including
  live and orphan consumer variants. The final full suite also runs this case.
- Final strict all-feature/all-target runtime/model Clippy and formatting passed.
- Final runtime/model no-default-feature check passed.

The overall test command exits 101. These unchanged tests fail at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

They remain failed, not waived. Development logs preserve the incorrect initial
Unknown-writer cleanup expectation, reuse of a stateful argument encoder, and
owner-thread fixture compile errors. Those fixture problems were corrected before
the signed final test run. The repeated-test binary digest is recorded separately.

## Open Gates

This does not close #182, A1/A2 or the runtime parity objective. Remaining gates
include integrated native terminal/unwind custody retention, a separately pinned
and authorized three-phase native hardware profile, and outer queued-owner shared
production-body proofs with mutation controls. Existing two-launch R57 authority
does not authorize this three-stage workload. No Verus or GPU run occurred here.

The next concrete queued-owner proof candidate is the actual `resolve_reads`
loop: select only the exact attached producer's reservations, record actual settled
epoch/lineage only on Success, detach attachments and retain slot/incarnation,
consumer and reader accounting. An independent logical update and represented
storage premise are required; admission, activation, cancellation/refund and
native outcome authentication remain separate obligations.

Context's active-then-queued reader release also needs a shared-body proof of its
committed prefixes: retain the root/marker on either failure, retire only after
both releases succeed. Abstract release-call contracts alone would prove only
sequencing, not concrete queued lease release or refund correspondence. New proof
roots/checkers must not repin historical evidence.

Partial/ReadWrite queued outputs and broader native profiles, Worker V3,
device-language, multi-device, memory, atomics/collectives and profiling
qualification remain separate roadmap work. No matched HIP/HSA benchmark ran.

MI300X SSH failed DNS resolution. Both source pushes also failed because
`github.com` could not resolve. No remote files/jobs were created; no permissions
or escalation were requested. Publication of the evidence commit is attempted
separately after freezing this archive. Unrelated inspection evidence is untouched.

`receipts.tar.xz` retains commands, exit statuses, timestamps, development and final
logs, signed source identity/patch/digests, toolchain identity and failed network
attempts. Its companion SHA-256 identifies the archive. See the
[integration contract](../../runtime-successor-writer-plan-v1.md).
