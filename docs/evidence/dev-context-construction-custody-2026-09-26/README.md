# Context Construction Unwind Custody

Base: `3d165841d009357c609f1209fb345a3dc025096e`.

Status: scoped CPU development qualification, not native or formal acceptance.

## Change And Boundary

`RuntimeContextV1::open_configured_v1` now roots its accepted backend in
`ManuallyDrop` before invoking backend initialization hooks. Enumeration or
allocation-profile unwind propagates its original payload without first running
the backend destructor. That backend and its owned resources remain retained
until process exit; this is not recoverable custody, cancellation, native
quiescence, cleanup, or permission to retry an uncertain native operation.

Journal bounds still reject before any backend hook. All ordinary returned
failures transfer the original backend to `RuntimeContextOpenFailureV1`.
Successful construction extracts the backend in the final struct initializer,
after the other fields have been initialized. The guard adds no heap allocation,
new account, backend call, or per-command cost.

The legacy `RuntimeContextV1::open` API still drops that failure's backend when
mapping an ordinary error. Journal and Worker constructors return the failure
owner, whose custody remains the caller's responsibility. Arbitrary factory
work before Context construction, and a factory panic after a completed Context
has been returned to it, are outside this guard. Process abort is not recoverable.

Current-thread and background owners retain their existing `InitializerPanicked`
classification. The Worker V3 application therefore distinguishes Context-hook
unwind from its returned `ContextRetainedUntilProcessExit` error. Neither path
fabricates a shutdown report before an owned engine exists.

## Regression Scope

Six focused test groups exercise the actual generic Context constructor, Worker
request-owner constructor, current-thread owner and background owner:

- Enumeration/profile panic across legacy, journal and Worker constructors:
  exact panic allocation identity, zero backend drops, live backend identity,
  original retained accounting token and no subsequent initialization hook.
- Rejected, Quiescent and Terminal errors from both hooks through both retaining
  APIs: original backend identity, exact failure class and caller disposal once.
- Invalid journal bounds, description, Required roster and Worker Legacy policy:
  correct hook prefix and original backend returned without accidental retention.
- Successful journal/nonjournal construction and shutdown: original backend
  transferred once, explicit fixture disposal releases credits. Legacy ordinary
  errors still drop once and quarantine their fixture token.
- Both async factories: a non-Send backend is created on its owner thread and
  hook panic produces `InitializerPanicked` without backend destruction.
- An isolated subprocess with an aborting backend destructor survives both
  constructor-hook panics because that destructor is never entered.

The accounting token and native-like destructor are CPU probes only, not KFD
authority or proof of real GPU cleanup. No native operation, protected Worker
application transaction, injected KFD hook, or formal correspondence is tested.

The initial build accidentally used an incomplete `--exact` filter and selected
zero tests (`raw/test-build-initial.log`); it is not a passing qualification.
The corrected exact baseline selected one test and failed at the intended
`backend dropped` assertion, observing one Drop rather than zero
(`raw/baseline.log`). `raw/baseline-source-sha256.txt` records that source.
The final fixture additionally makes the backend non-Send and creates it inside
the background factory. Neither change weakens the failing baseline assertion.

## Qualification

All Cargo commands use `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1` and `--offline`.
Library tests run serially. Counts overlap and are not additive.

| Gate | Result | Log |
| --- | --- | --- |
| Focused constructor regressions | 6 passed | `raw/focused.log` |
| Unfiltered all-features runtime library | 1,523 passed, 3 failed, 28 ignored | `raw/runtime-all-features.log` |
| Runtime and host doctests | 80 passed (52 runtime, 28 host) | `raw/doctests.log` |
| Strict all-features/all-targets runtime and host Clippy | Passed with `-D warnings` | `raw/clippy.log` |
| Runtime and host no-default-features check | Passed | `raw/minimal-check.log` |
| Formatting and whitespace | Passed | `raw/format.log`, `raw/whitespace.log` |
| Final source manifest | Passed | `raw/source-sha256.txt`, `raw/source-check.log` |

The three broad-suite failures are the unchanged
`authorized_execution::tests::{cooperative_debug_telemetry_emits_only_bounded_logical_records,
failed_session_end_is_explicit_and_terminal, pre_native_telemetry_failure_is_returned_and_poisoned}`.
Each fails with `InspectSocket`/`EPERM` at `authorized_execution.rs:1317`, as in
the prior accounted fail-stop packet. They remain failed qualification; no
permission exception, skipped failing test, or weakened assertion was used.

Independent read-only agent review found no production blocker and checked the
ownership transitions, extraction order, fixture scope and documentation. The
source manifest identifies tested source, not an authenticated Verus/native
receipt. Toolchain details are in `raw/toolchain.txt` and `raw/cargo-version.txt`.

## Reproduction

Prefix each Cargo command with the environment above:

```sh
cargo test --offline -p fe2o3-runtime --lib construction_custody_tests -- --test-threads=1
cargo test --offline -p fe2o3-runtime --all-features --lib -- --test-threads=1
cargo test --offline -p fe2o3-runtime -p fe2o3-host --doc
cargo clippy --offline -p fe2o3-runtime -p fe2o3-host --all-features --all-targets -- -D warnings
cargo check --offline -p fe2o3-runtime -p fe2o3-host --no-default-features
cargo fmt -p fe2o3-runtime -p fe2o3-host --check
git diff --check
sha256sum -c docs/evidence/dev-context-construction-custody-2026-09-26/raw/source-sha256.txt
```

## External Gates

MI300X still fails hostname resolution (`raw/mi300x-connectivity.log`); no remote
job or file was created. The pre-existing owner-inspection packet is untouched.
No new Verus, native or matched HIP/HSA performance campaign is claimed.
Production providers/refinement artifacts, native fault qualification and the
remaining A0-A7 exit criteria are unchanged.
