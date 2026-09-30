# Requested Allocation Charge Profile

## Shared Construction

Integrated at `ee849c0fc` from signed candidate `67e73a3d0`.

`r67_requested_allocation_charge_v1(bytes)` constructs the same resource vector
for the runtime and KFD request-account wrappers. Its nineteen coordinates are
exact: `RequestedAllocationBytes` is `bytes`, `AllocationRecords` is one, and
all seventeen other coordinates are zero. A zero-byte request still consumes
one allocation record. Construction uses a fixed-size literal without rounding,
heap allocation or fallible arithmetic.

The shared executable body is
`crates/fe2o3-runtime-model/src/request_charge_body.rs`. It is included by both
the native model function and its separate Verus root. Source calibration binds
the actual resource-kind order, vector dimension, aliases and wrapper calls.

## Boundaries

This is a request-accounting profile, not proof of allocation admission, backing
size, residency, device currentness or aggregate memory bounds. It does not
replace live-account observations, locking or resource custody. The
[independent](runtime-independent-retained-observation-v1.md) and
[domain](runtime-domain-retained-observation-v1.md) observers retain their own
contracts and qualification boundaries.

Runtime CPU contamination tests exercise the General account branch. KFD-local
tests exercise its actual composed request account and exact wrapper profile.
These are not a fabricated runtime Composed device admission or native GPU test.

## Qualification

The signed candidate passes three full `--no-cheating` runs, each with three
verified obligations and zero errors, including an exact three-file relocated
proof. All 21 actual-body mutations fail logically: four alter the byte/record
profile and seventeen contaminate otherwise-zero coordinates. Both verifier
release checks and all 29 fresh campaign process-group closures pass. The
three positive runs overlap; their obligations are not additive.

Earlier focused model/runtime/KFD groups and the complete accounting suite pass
106 distinct CPU tests. Four-crate no-default compilation, strict all-feature,
all-target Clippy and scoped formatting pass. All 6,300 selected inputs are
bound to the signed candidate; eleven subsequent metadata-only changes leave
the CPU-tested Rust and proof bodies unchanged. This is pre-signing CPU/static
evidence reuse, not fresh signed execution of those checks.

The merged implementation matches the candidate. Two inherited KFD source
guards additionally include the separately qualified snapshot-fixture changes;
their executable proof closures, counts and policies are unchanged. The nine
affected source-calibration suites pass locally without new older-proof runs.

Records remain local under
`/home/harsh/.codex-tmp/fe2o3-request-profile-qualification-20260930-retained`.
The signed campaign result is `signed-campaign-attempt-1/results.json`, SHA-256
`8130f92a349975bde32c8162b24fa43affdc42d53aae3e5277bcd739575db364`.
No complete merged-runtime suite, native execution, performance improvement or
milestone exit follows from this component qualification.
