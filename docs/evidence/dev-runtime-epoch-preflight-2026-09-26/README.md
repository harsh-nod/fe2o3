# Runtime Epoch Preallocation Development

Developed on `codex/r65-runtime-drain-versions`, based on
`8d2da8697aadfaa57f84c90212c439e095b4639e`. This is development evidence, not
sealed native qualification, new formal refinement, A1/A2 acceptance or parity.

## Implementation

`Gfx942FixedDispatchPreallocationV1` owns vacant scaled epoch storage and its
credit. The public fresh producer borrows the capacity; the next-binding producer
borrows the exact live primary/auxiliary lane. Neither consumes a native owner.
An unused token refunds on destruction. A handed-off token follows the existing
consuming constructor/rebind custody contract, including retained failure paths.

The runtime reserves after its ordinary publication path's attached-reuse
decision, before recycled detach, admitted-device consumption or resident DATA
movement. Fresh primary, bootstrap initial binding, auxiliary construction and
live rebind transfer the same table without another allocation/debit. Default64
and attached reuse skip the new reservation. Attached replacement needs temporary
old-plus-new table headroom. Its seed comes from the recycled predecessor, not a
counter advanced by canceled reservations.

The token grants storage, not execution authority. Rebind tokens bind the exact
queue, ledger, profile and next generation; they are not tied to one unique
pristine continuation. Fresh tokens have no queue target and cannot substitute
for rebind tokens. Handoff revalidates state, and native currentness checks remain.

Only host allocation/credit capacity exhaustion from the explicit runtime
preflight becomes nonterminal `Capacity`. Other errors keep terminal handling.
Public runtime admission has already accepted the launch: it settles `Failed`
and releases logical retains, not synchronous rejection or automatic retry.
Earlier runtime staging may synchronize allocations or release other-lane caches;
the guarantee is local to the destructive steps in ordinary publication.

## Coverage Boundaries

- Runtime exhaustion tests call actual publication with metadata-only recycled
  and resident rosters. They preserve descriptors, allocation contents and the
  credit ledger; they do not fabricate native DATA or an admitted device.
- Accepted-failure coverage uses a CPU admission switch and an exhausted account
  to reach the real preflight before native access. It checks logical retain,
  lane, completion reservation, submission and stream-tail cleanup.
- Attached reuse coverage checks the real decision predicate and reservation
  helper, not native reuse. A source assertion bounded to `publish()` checks all
  four handoff call sites and reservation placement before destructive operations.
- Detached/pristine rebind uses real preparation, validation and settlement
  sequencers with simulated memory, an exact one-table budget and competing
  reservation rejection. Fresh, wrong-queue, same-seed cross-lane and stale
  generation tokens are rejected with exact error categories before preparation.
- Primary and auxiliary selection, including an auxiliary roster hole, use the
  borrowed public producer and verify unchanged parent/poison state, exact target
  identity, stale-handle rejection and unused-token refunds.
- The attached-recycled fixture performs real logical recycle and two canceled
  epoch bindings, reserves through a public session facade, then runs the existing
  detach and rebind sequencers with actual returned DATA and committed ledger.
  One-table exhaustion precedes detach; two-table headroom permits the transition
  and refunds the old table. These are CPU transitions, not GPU completions.
- Existing initial/auxiliary fixtures accept supplied storage under an exact
  one-table budget. Existing preflight handoff tests verify table pointer preservation.
  A compile-fail doctest checks that the public token cannot be cloned.

Only CPU fixture custody is disposed to test refunds. Native terminal custody
is not claimed to be recoverable. Groups overlap and are not distinct totals.

## Validation

Commands use the owned target
`/home/harsh/.codex-tmp/fe2o3-runtime-preflight-target-20260926-t8gM2kIa`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`CARGO_INCREMENTAL=0`. Cargo commands use `--locked --offline`; tests use two
threads. Cargo reports KFD executable `debug/deps/fe2o3_kfd-60551c78c3ce1a5d`
and runtime executable `debug/deps/fe2o3_runtime-f40ec65f6abc2ea1`.

| Raw log | Command scope | Result |
| --- | --- | --- |
| `scaled-final.log` | `cargo test -p fe2o3-kfd -p fe2o3-runtime --all-features --lib scaled_` | KFD: 28 passed; runtime: 17 passed, 1 native test ignored |
| `scaled-qualified.log` | Final KFD executable, filter `scaled_` | 28 passed |
| `rebind.log` | Same Cargo invocation, filter `queue::live::rebind_tests::` | KFD: 56 passed; runtime: no matching tests |
| `dispatch-binding.log` | KFD executable, filter `queue::dispatch_binding::` | 153 passed |
| `construction.log` | KFD executable, filters below, 900-second bound | 67 passed |
| `kfd-non-construction-final.log` | KFD executable, `--skip queue::live::construction_primary::integration_tests`, 300-second bound | 1,293 passed, 1 failed, 296 filtered out |
| `runtime.log` | Full runtime executable, 1,200-second bound | 1,448 passed, 3 failed, 28 ignored |
| `clippy.log` | `cargo clippy -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings` | Passed |
| `default-check.log` | `cargo check -p fe2o3-kfd -p fe2o3-runtime --no-default-features` | Passed |
| `doctests.log` | `cargo test -p fe2o3-kfd -p fe2o3-runtime --all-features --doc` | KFD: 28 passed; runtime: 46 passed |
| `fmt-qualified.log` | `cargo fmt -p fe2o3-kfd -p fe2o3-runtime -- --check` | Passed |

Construction filters are `initial_bind_cases`, `replacement_cases`,
`same_engine_auxiliary_`, `construction_auxiliary::tests`,
`backing_constructor_forwarding`, `persistent_owner_and_queue_layouts`, and
`recycled_detach`. The latter includes the new scaled attached-preallocation
test and existing native-fault/retake matrices using CPU memory fixtures.

The initial expanded test build in `raw/scaled-expanded.log` failed on test-only
constructor naming, record-count type and unsupported authority equality.
The corrected fixture uses `Default`, checked integer conversion and exact
storage identities. The failed log is retained, not overwritten.
The first broad KFD subset in `raw/kfd-non-construction.log` also caught a
source-routing assertion sensitive to rustfmt's method-call line break. Its
replacement checks the continuation arm and exact call independently inside
the bounded settlement source. The corrected rebind run passes all 56 tests.
The initial formatting-check diff is retained alongside the successful final
formatting check.

The full runtime failures are
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each at
`InspectSocket(EPERM)`. They match the preceding development baseline.
The corrected KFD subset's sole failure is
`credential_bound_channel_accepts_typed_failure_before_publication`, reporting
`SocketAdmission`, also present in the preceding baseline. The whole KFD library
was not run; selected construction coverage does not replace all 296 exclusions.

`raw/socket-probe.json` independently reproduces EPERM for all six socket
inspection prerequisites. Production socket admission remains fail-closed.
`raw/mi300x-access.log` and `raw/mi300x-access-recheck.log` fail at hostname
resolution before reaching MI300X; no remote resources were created.

## Cleanup

All validation processes reached terminal status before the exact owned target
was removed with `rm -r --` and independently checked absent. The directory held
624,672 KiB of allocated storage (`raw/cleanup-before.log`); the post-removal
`ls -ld` records ENOENT in `raw/cleanup-absence.log`, and `test ! -e` succeeds.
No unrelated build directories or the user's owner-inspection evidence were
modified or removed.

## Remaining Gates

Aggregate backing/control/slot admission, retryable Context admission, scaled
formal refinement, native retained-depth/reuse/rebind/async-owner execution,
signed independent replay and matched HIP/HSA measurements remain open. Runtime
custody saturation is not native epoch saturation. No performance improvement
or full CPU/native qualification is inferred from these development checks.
