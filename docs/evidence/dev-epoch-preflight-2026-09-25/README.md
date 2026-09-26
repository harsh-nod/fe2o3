# Owned Epoch Preflight Development

Developed on `codex/r65-runtime-drain-versions`, based on
`4e4acf79cebd86e820a77e2e77c5dea090cb5d4b`; validation completed on 2026-09-26 UTC.
The directory retains the work's start date. This is development evidence, not
sealed native qualification, new formal refinement, or A1/A2 acceptance.

## Implementation

The Qualification1024 native path now allocates and charges a move-only epoch
table before entering mutable preparation. Initial binding reserves before
currentness and DATA initialization. Auxiliary construction reserves before its
opening callback. Detached/pristine rebinding reserves before moving rooted
inputs into preparation and before pristine-continuation consumption. Fresh
primary and replacement construction reserve before resource planning, while
retaining their already-consumed inputs.

Preparation validates the table's pristine state, exact account identity, profile
and generation before moving that same allocation. A missing/mismatched scaled
reservation fails closed; there is no late fallback or second allocation.
Default64 still allocates and evaluates its generation seed at preparation entry.

Auxiliary initializer captures rejected by the new epoch-table preflight are
deliberately retained without running their destructors. They may own native DATA
and cannot be returned by the consuming API. This is not a refund, cleanup or
retry capability. Post-entry failures retain
the table in the existing terminal root/preparation. Runtime error classification
has not been weakened.

## Tests

CPU tests exercise the production preparation and construction sequencers with
simulated native memory, not Linux queue execution. Coverage includes:

- Same table pointer and full debit through transfer, competing-credit rejection,
  one-table byte/record budgets, and unused-owner refunds.
- Wrong ledger, profile, seed and non-pristine owner rejection before consumption.
- Independent byte and record exhaustion, unchanged DATA-vector storage and
  identities, continuation generation/account, lane metadata and process gate.
- Initial, auxiliary and detached/pristine post-entry errors and panics retaining
  the debit before and after generation handoff.
- Panicking uninvoked initializer captures and a panicking retention callback.
- Existing Default64 preparation, rebind, initial/auxiliary and replacement fault
  matrices, plus source-routing and inline-layout guards.

Only CPU fixture custody is dropped to check final refunds. Native terminal
custody is not claimed to be recoverable.

Cargo uses the owned target
`/home/harsh/.codex-tmp/fe2o3-epoch-preflight-target-20260925-e0cHbCB1`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`CARGO_INCREMENTAL=0`. Test commands use two test threads. KFD builds have a
900-second timeout; runtime, lint, default checking and doctests have 1,200-second
timeouts. Direct executable runs use the binary reported by Cargo:
`debug/deps/fe2o3_kfd-0b12f6a7b1bdedc6`.

| Raw log | Command scope | Result |
| --- | --- | --- |
| `scaled-final.log` | `cargo test -p fe2o3-kfd --all-features --lib scaled_` | 23 passed |
| `scaled-qualified.log` | Final post-format KFD executable, filter `scaled_` | 23 passed |
| `dispatch-binding.log` | Same Cargo command, filter `queue::dispatch_binding::` | 152 passed |
| `rebind.log` | Same Cargo command, filter `queue::live::rebind_tests::` | 53 passed |
| `construction.log` | Direct executable, filters listed below, 900-second bound | 49 passed |
| `kfd-non-construction.log` | Direct executable, `--skip queue::live::construction_primary::integration_tests`, 180-second bound | 1,289 passed, 1 failed, 295 filtered out |
| `runtime.log` | `cargo test -p fe2o3-runtime --all-features --lib` | 1,444 passed, 3 failed, 28 ignored |
| `clippy.log` | `cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings` | Passed |
| `default-check.log` | `cargo check --locked --offline -p fe2o3-kfd -p fe2o3-runtime --no-default-features` | Passed |
| `doctests.log` | `cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --doc` | KFD: 27 passed; runtime: 46 passed |
| `fmt-check.log` | `cargo fmt -p fe2o3-kfd -p fe2o3-runtime -- --check` | Passed |
| `socket-probe.json` | Existing observer packet's `socket_probe.py` | All six socket prerequisites return EPERM |

Construction filters are `initial_bind_cases`, `replacement_cases`,
`same_engine_auxiliary_`, `construction_auxiliary::tests`,
`backing_constructor_forwarding`, and `persistent_owner_and_queue_layouts`.
The groups overlap; their totals must not be added as distinct tests. The
whole KFD library was not rerun, and the non-construction subset explicitly
excludes 295 cases. Selected construction matrices do not replace that omission.

The KFD subset's failure is
`credential_bound_channel_accepts_typed_failure_before_publication`, reporting
`SocketAdmission` at `target_debug_telemetry_v2.rs:1173`. The independent socket
probe reproduces EPERM for socket-domain/type, peer-name/credentials and
pass-credentials operations. Production validation remains fail-closed.

The full runtime failures are
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each reports
`InspectSocket(EPERM)` at `authorized_execution.rs:1317`, as in the prior packets.
Full CPU qualification did not pass; no admission check or test was weakened.

Earlier attempts are retained: `scaled-initial.log` has the missing nested-test
module path; `scaled-second.log` has fixture constructor/field compile errors;
`scaled-third.log` reports 21 passes and one incorrect trace expectation. The
last assertion was corrected to include borrowed owner validation, which precedes
credit reservation without native mutation. Expanded final tests pass as above.

## Open Gates

SSH failed before connecting to MI300X (`mi300x-access.log`): hostname resolution
for `sharkmi300x-1` returned a temporary failure. No remote jobs, artifacts or GPU
reservations were created. No new native execution, musl validation, GPU overlap,
high-depth qualification, formal proof or HIP/HSA comparison is claimed.

Consuming APIs still retain rejected inputs, so borrowed native preflight does
not yet establish retryable public Context admission. The next runtime boundary
needs an opaque owned reservation before recycled-dispatch detach, admitted-device
consumption or resident DATA movement. It must skip attached reuse, preserve the
selected queue/ledger/seed, and account for old/new table overlap. Aggregate
backing/control/slot admission, scaled allocator/queue refinement, native
reuse/rebind and async-owner depth, signed replay, and matched HIP/HSA performance
remain open. Accepted checkpoints and A1/A2 status do not change.

## Cleanup

All validation sessions were confirmed terminal before cleanup. The exact owned
target above occupied 730,432 KiB (`du -sk`). A force-removal command was rejected
by the execution policy; non-forced `rm -r -- <exact-owned-target>` then completed
successfully. An independent `test ! -e <exact-owned-target>` passed. No remote
cleanup was needed, and the unrelated user-owned
`docs/evidence/dev-owner-inspection-2026-09-23/` directory was left untouched.
