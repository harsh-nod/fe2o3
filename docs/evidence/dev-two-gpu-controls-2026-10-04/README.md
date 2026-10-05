# Two-GPU Failure Controls

Base: `203c0e43e9b49efbb6cf6e83f730942593118e61`.
The genuine two-device fixture now implements two explicit negative controls.
CPU qualification and a fresh genuine admission-only campaign pass. **Neither
control nor the positive two-GPU path has executed on hardware in this campaign.**
This does not close A3/A7, HIP/HSA parity, native timeout coverage or performance.

## Implemented Cases

With the existing complete qualification environment and two explicit GPU UIDs,
use `FE2O3_PROOF_INSTALL_CAMPAIGN=genuine-two-gpu` and select
`FE2O3_GENUINE_TWO_GPU_CASE`:

| Case | Required behavior before reporting |
| --- | --- |
| `positive` (default) | Existing two fills, bidirectional native peer copies, complete output/guard checks and released shutdown. |
| `second-coverage-reject` | First invocation accepted; second 65-element invocation uses G64/WG64 and must fail with exact `Coverage(Underlaunch { elements: 65, grid_x: 64 })`. Retain and complete the first invocation, check its result, drain and release. |
| `peer-deadline-before-submit` | Both fills and four uploads complete. Enqueue the first tracked peer operation, require Queued before and after an expired wait, explicitly cancel before submission, then observe the same future's exact cancellation acknowledgement. Require zero native peer completions and unchanged sources and sentinel frames before release/drain. |

The control selector is qualification-only. It chooses a fixed ignored test and
an exact fixture argument; it is not added to application authority environment
or used to bypass admission. Unknown, empty and misplaced selectors reject.
Zero fixture arguments remain admission-only; two UIDs remain the positive case.

Both controls require inspected owned shutdown (`Released`, complete cleanup,
no native failure or worker panic), zero reply/snapshot credits, fully refunded
charged-result budgets and retained-artifact revalidation. Unexpected errors do
not produce a control record. A matched negative test exits normally and emits
only `fe2o3.genuine-two-gpu-control.v1`; this keeps all production Cargo postflight
checks. The harness requires clean exit plus exactly one matching mode/UID/order/
shutdown record, rejects mixed positive/control output, and checks manager and
coordinator continuity. Normal status alone cannot qualify either control.

The earlier proposed nonzero application exit was removed: Cargo collapses it
and skips some final checks. No production CLI behavior was changed to accommodate
the controls. The default positive schema and matcher remain unchanged.

The deadline control proves **host wait expiry followed by pre-submission
cancellation**, not native admission, a GPU timeout, cancellation of running GPU
work or quarantine settlement. It deliberately does not submit a hanging kernel.
Actual native in-flight failure/timeout coverage remains separate work.

## Validation

- 5 fixture parser/data-oracle tests pass, including exact mode/arity/encoding and
  second-invocation-only geometry changes.
- 39 static-musl custodian tests pass; 14 environment-dependent tests are ignored.
  Strict control-report checks include wrong status, mode, devices, shutdown,
  duplicate/mixed/escaped-positive records, unknown fields and invalid UTF-8.
- 41 current-thread runtime tests pass. The new composed regression requires the
  original peer future, an expired drive, explicit queued cancellation, exact
  owner acknowledgement, credit retention until consumer drop, no backend issues
  and complete owned cleanup. These are mock CPU tests, not GPU evidence.
- 4 actual conditional-invocation packing tests pass. The undercoverage control
  now checks the exact error and extent rather than merely any error.
- 8 host-link shell integration tests and the expanded two-GPU selector/mount
  shell checks pass. Shell syntax, scoped ShellCheck and whitespace checks pass.
- Fixture all-target typechecking and strict binding-only Clippy pass, as does
  strict custodian Clippy. The runtime test build reports the existing unused
  `MaterializedSourceEventV1::snapshot` warning; no general runtime Clippy claim.
- Fresh genuine installed-service admission-only campaign passes **1/1 in
  487.36 seconds**, **501.51 seconds** including deployment and cleanup. It runs
  real selected compilation, issuer/anchor publication, Worker finalization,
  ordinary static host compilation, FD195/current-record checks and retained
  conditional proof with the controls compiled into the application. Manager/
  coordinator continuity passes. This run uses the existing direct-input profile;
  it does not repeat hidden-home input projection or the optional link observer.
- Implementation and measured binaries stayed fixed through the campaign. The
  owned cgroup was removed and independently checked absent. The later manager
  quarantine message is service teardown, not evidence of native GPU settlement.

Custodian test SHA256:
`e34c138b45d081e7d0cfbe96e93661bd0f1a6c4ff8252efa715335516eb4a34e`.
CLI SHA256 (unchanged):
`c959ad064c604e62a9b7a8b0c9e37b34a30f55dfe8278e54fe958161ad112e44`.
Backend SHA256 (unchanged):
`2bfad4b0dcc64f4fba38a88dbb6123919da730a06d5867009397a7e0edc7102d`.

Three native agents provided read-only review and hardware observation; the
primary owned edits and tests. Local disk pressure required moving only this
task's generated fixture cache to owned scratch. A symlink alias correctly
rejected; using its explicit canonical target path passed without relaxing policy.

## Remaining Hardware Gate

At **2026-10-05 04:15:25-04:15:44 UTC**, all eight MI300X GPUs still had live KFD
users; neither fixed fe2o3 service was installed, and noninteractive sudo required
a password. No remote workload or mutation was performed. The immediate work
remains real-root private service deployment and a positive admitted two-device
run, followed by these two controls. No wider opcode, loader-packaging or
performance work takes precedence over that path.

Archive: [qualification.tar.gz](qualification.tar.gz). It contains commands,
environment, logs, source patch and review notes, not executable/cache bytes or
private keys. No HIP/HSA comparison or new whole-runtime formal proof is claimed.
SHA256: `65ea737c9422246fb55594a4e0430ff2ffe4a8daa46b26c40dbae265c533a6f6`.
