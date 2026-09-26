# Scaled Native Depth Canary Development

Development based on `24a82c3b81d35e1c466c17f72e7a18876a7ba49f`. This packet
implements a native qualification canary and CPU consistency tests. It is not
a native result, formal refinement, A1/A2 acceptance or HIP/HSA parity result.

## Implemented Checks

The ignored, scale-feature-only runtime test is
`kfd_backend::scale_capacity::native_depth::native_scaled_two_lane_2048_retained_receipts_and_cleanup`.
It uses the real exact-vecadd opt-in, two streams, six disjoint HostVisible
buffers, and 1,024 identical-recipe launches per lane. It flushes each launch
and inspects all 2,048 original native receipts before any completion poll,
wait or readback. Queue observation authenticates each retained receipt and
rejects observation on the other lane. A frontier-only census cannot pass.

The typed checker requires exact ordered submission identities, lane leases,
stream tails, module references, predecessor retains, pipeline phases and all
six 1,024-owner custody rosters. Its membership digest binds each receipt hash
to its logical coordinates and phase. The native capture also retains runtime
slot identities, recipe identities, publication times and the profile prefix
for before/after rejection comparison. Addresses are not exported in JSON.

Two separate negative cells run on both full lanes:

- Public same-buffer launch 1,025 must reject at runtime custody admission.
  Backend receipts, indexes, handles, profile history, account/backing usage,
  Context drain snapshots/counts and original typed-token mappings remain equal.
  Context's monotonic ID allocator is deliberately not part of this equality.
  Unexpectedly accepted typed tokens remain owned, are waited/polled and released
  before the original tokens, and fail qualification only after cleanup. Later
  full-depth snapshot/probe assumptions are skipped once admission changes state.
- Direct ordinary resubmission of the already-authorized native recipe must
  return `RejectedBeforeSideEffect(DispatchBinding(DispatchEpochCapacity {
  maximum: 1024 }))`. This bypasses the runtime custody limit without adding
  new authority or preparing DATA. Its retained snapshot and Context state
  must remain equal. Native private generation counters are not exposed by
  this observation; equality of those counters is not claimed. An unexpected
  successful native publication keeps its exact lane/token, is polled and
  recycled separately, and still fails qualification after normal teardown.

Completion requires both lane tails and every typed submission to succeed.
Every byte of all six final buffers is compared with the fixture oracle.
Repeated identical writes validate final contents, not distinguishable output
from each of the 2,048 invocations. The full profile must validate with no drops;
every publication must equal metadata derived from the retained recipe,
including kernel, shape, geometry and bindings. Publications precede observed
completions, completion is ordered per lane, releases follow completion, and
queue destruction follows all releases. Shared conversion helpers check
consistency, not independently implemented profiler semantics.

The table ledger must contain ten retained records at peak: two runtime tables,
two native tables and six custody rosters. Native shutdown must leave only the
original two runtime-table records; backend destruction must leave all resource
axes and record counts zero. Existing primary-release instrumentation separately
checks native backing disposal. Payload accounting excludes allocator overhead,
maps, snapshots, kernargs, GPU backing and total process residency.

Routine depth, equality, output and profile assertions run after successful
teardown. Native API errors, structural ownership failures and deadlines can
still abort the isolated process with custody unresolved. No success/cleanup
record is emitted on those paths; in-process cleanup is not guaranteed on failure.

## Validation

Final validation results are retained in `raw/`. The initial console-only
compile failed because the SHA-256 output array does not implement `LowerHex`;
the canary now formats individual bytes. `raw/focused.log` retains the next
passing run (nine CPU tests, one native ignore), before the native-capacity
probe and its final custody/deadline refinements were added.
The first full run in `raw/runtime-all-features.log` reports 1,444 passed,
three telemetry socket failures and 28 ignored, before the last recovery/lint
refinements. The first strict Clippy run (`raw/clippy.log`) rejected a collapsible
match, a range-index loop and the native recycle closure's large error type.
The first two are rewritten; the recovery function has a narrow lint allowance
because its existing error type returns original linear custody without another
allocation. No production policy or native admission limit is relaxed.

Final checks use the environment below and 900-second Cargo limits; none timed out:

| Command | Result | Log |
| --- | --- | --- |
| `cargo test -p fe2o3-runtime --all-features --lib -- --test-threads=2` | 1,444 passed, 3 failed, 28 ignored; exit 101 | `raw/runtime-all-features-final.log` |
| `cargo test -p fe2o3-runtime --features scale-qualification --lib scale_capacity::native_depth -- --test-threads=2` | 9 passed, 1 native ignore; exit 0 | `raw/scale-feature-focused.log` |
| `cargo clippy -p fe2o3-runtime --all-features --all-targets -- -D warnings` | Pass, exit 0 | `raw/clippy-final.log` |
| `cargo check -p fe2o3-runtime --no-default-features` | Pass, exit 0 | `raw/default-check.log` |
| `cargo test -p fe2o3-runtime --all-features --doc` | 46 passed (4 + 42), exit 0 | `raw/doctests.log` |
| `cargo fmt -p fe2o3-runtime -- --check` | Pass, exit 0 | `raw/fmt.log` |

The same three unchanged telemetry tests fail at `authorized_execution.rs:1317`
with `InspectSocket(EPERM)` in both full runs:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

The independent retained socket probe was rerun; all six inspections/options
report `EPERM` in `raw/socket-probe.json`. Production socket checks remain
fail-closed. This packet does **not** pass full CPU qualification.
`raw/source-sha256.log` records all six touched Rust files;
`raw/source-check.log` confirms their final validation bytes remain unchanged.
Rust/Cargo versions are retained in their corresponding raw logs. These source
hashes do not constitute a whole-workspace or independent native replay proof.

Two read-only agents reviewed receipt/profile consistency and native saturation
ownership. Primary integrated the fixes, including full profiler metadata joins,
Context rejection preservation, deferred routine assertions and custody-preserving
unexpected-admission recovery. Agent review did not execute hardware tests.

The CPU tests are synthetic consistency and mutation checks, never GPU receipt
evidence. Mutations include missing, duplicate and cross-lane rows; phase and
owner corruption; ledger misuse; incomplete or reordered profiles; and thirteen
publication-metadata mutations on both lanes at ordinals 0, 255, 256 and 1,023.

## Reproduction And Limits

CPU validation uses GNU/Linux with this owned target and environment:

```sh
env CARGO_TARGET_DIR=/home/harsh/.codex-tmp/fe2o3-scale-depth-target-20260925-YLX22UGi \
    CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
    timeout 900 cargo test -p fe2o3-runtime --all-features --lib -- --test-threads=2
```

Native reproduction requires an independently checked idle admitted gfx942
device, its hexadecimal unique ID, and a dedicated process with an external
process-group deadline. Run only this exact ignored test, not all ignored tests:

```sh
env FE2O3_TEST_NATIVE_UNIQUE_ID="${FE2O3_TEST_NATIVE_UNIQUE_ID:?selected idle GPU unique ID}" \
    timeout --kill-after=10s 600s cargo test -p fe2o3-runtime \
    --features scale-qualification --lib \
    kfd_backend::scale_capacity::native_depth::native_scaled_two_lane_2048_retained_receipts_and_cleanup \
    -- --exact --ignored --nocapture --test-threads=1
```

The canary precommits 64 MiB/32 records for host-table payload and
128 MiB/512 records for native host backing. Per-tail and unexpected-native
recovery deadlines are 60 seconds. These are separate limits, not an aggregate
memory bound. The profiler ceiling is 16,384 events. Reuse/rebind cycles and
async-owner high-depth integration require further qualification cells.

On success the development JSON exports every receipt row, including the
pipeline phase (`null` for the active frontier), final buffer hashes and
separate runtime/native negative labels. The membership phase encoding is
null=0, published=1, completed=2, physically-retired=3, quarantined=4.
The full profile is also printed. This is not an authenticated external replay
format: the private custody snapshots and checker inputs are not all exported.
Retained publications are not necessarily unfinished GPU work, and physical
overlap is explicitly unmeasured. No matched HIP/HSA benchmark is run here.

SSH failed before connection at hostname resolution; `raw/ssh-attempt.log`
retains the error. No native job, GPU reservation or shared-host artifact was
created. No musl or new formal proof run is claimed in this packet.

After all build/test sessions terminated, the exact owned local target above
was removed (668312 KiB). A separate `test ! -e <exact-owned-path>` returned 0.
Cargo caches and the unrelated `dev-owner-inspection-2026-09-23` evidence directory
were left untouched.
