# Queued Producer Read Checkpoint

CPU developer evidence only. Not formal refinement, GPU ordering qualification,
or HIP/HSA behavioral/performance parity.

Signed source: `a89c3ffab1a0ea645a8969dc28e5b7a10e2469f4`.
Source tree: `c6ac8edc2417669faba9d2c9ceb42ed9400f0e03`.
Parent: `9f130a12f1836dfaf191affd5b40c8d2f6681ba5`.
SSH signature verified for `harmenon@amd.com`.

## Implemented

The queued-writer owner now retains exact queued-producer read leases without
inventing future content versions. Stable, active-producer and queued-producer
reads share the same R budget. Attached reads resolve only with their exact
producer's Success, NoEffect or Unknown result; they remain live independently
of writer-slot reuse until consumer quiescence.

Producer-aware Context pure reads may consume the exact latest Waiting/Ready
queued full-Write output named by an explicit event. Original alias coverage,
device/extent/identity checks and independently retained dependencies remain
required. Context keeps distinct typed reference arrays and marker anchors for
active and queued producers. All seven completion ingresses retain producer-first
logical reconciliation even if the backend has already completed the byte effects.

New writers, retirement and Unknown disposal account for queued readers while
already-admitted ancestors remain able to progress. Mixed acquisition preflights
all three reader families before committing. Active-then-queued release is a
committed-prefix operation: an injected error or panic between classes retains
the Context root and quarantines the remaining lease, not a rollback.

Producer validation in model mixed acquisition is deduplicated using fixed
scratch. An indexed-access regression checks affine work for 4/8/16 outputs and
reads of one producer. This is an algorithmic check, not measured latency or a
linear bound for Context's entire binding-authentication path.

## Validation

Nine new model tests and seven new Context tests cover exact-tail binding,
actual byte observations, mixed reader families, cancellation without reparenting,
quiescent/terminal outcomes, released public events, writer-slot reuse, capacity,
stale/corrupt references, attached-list corruption, release fault prefixes,
fixed storage and the producer-scan regression. An existing queued-output test
now admits a pure reader against a Ready producer while retaining invalid-event
and generic/host access rejection.

Final all-feature command, with no failed target skipped:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test \
  -p fe2o3-runtime -p fe2o3-runtime-model --all-features --no-fail-fast
```

- Model unit tests: 1,079 passed, 0 failed, 19 ignored.
- Runtime unit tests: 1,710 passed, 3 failed, 28 ignored.
- Runtime integration tests: 11 passed, 3 hardware tests ignored.
- Runtime doctests: 52 passed. Model doctests: 29 passed.
- Focused development suites: 45 queued-model and 62 producer-launch tests passed.
- Strict all-feature/all-target runtime/model Clippy passed.
- Runtime/model no-default-feature check and final formatting check passed.

The three failures match the previous checkpoint: these authorized-execution
tests fail at `authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

They remain failed, not waived. The raw archive also preserves a development run
that caught the Reserved-writer admission bug, a mixed-fixture writer-capacity
mistake, and the initial strict Clippy naming failure. The code and fixtures were
fixed before the signed final test run.

## Remaining Work

This change does not close #182, A1/A2, or the full runtime parity objective.
Required next evidence includes journal-enabled Context/native scripted backing
continuity, async command/driver carriage, a separately pinned three-phase native
hardware profile, and outer queued-owner shared-body proofs with mutation controls.
Existing R57 two-launch authority must not be stretched to authorize the new
three-stage overwrite/read hardware test.

Partial/ReadWrite queued outputs, broader native launch profiles and the wider
Worker/device-language/multi-device/atomic/collective/profiling qualification
remain separate work. Existing inner-journal and two-family reader proofs do not
prove this new outer acquisition/resolution/release composition. No Verus run,
GPU test or matched HIP/HSA performance benchmark ran for this checkpoint.

MI300X SSH failed DNS resolution; no remote files or jobs were created. No
permissions or escalation were requested. Unrelated local inspection evidence
was preserved. Publication is attempted separately after this packet is frozen.

`receipts.tar.xz` contains commands, exit statuses, timestamps, source/tree and
file digests, the source patch, toolchain identity, signature verification, and
development/final logs. The companion SHA-256 file identifies the archive.
See the [integration contract](../../runtime-successor-writer-plan-v1.md).
