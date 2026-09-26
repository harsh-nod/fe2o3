# Settled Peer Producers For Typed Launches

Date: 2026-09-26. Development evidence only. Accepted milestones, A1/A2,
native qualification, formal correspondence and HIP/HSA parity are unchanged.

## Source And Scope

Signed implementation: `69c688fe676de5499f7a1406aca57b7922220f99`.
Parent: `67e2075a48ca64551c28ac019f0943114b65f718`.
See [the composition contract](../../runtime-peer-producer-composition-v1.md).

Context now admits successful, quiescent ordinary and directed scalar copies as
typed-launch parents. Physical completion alone is insufficient. It preserves
exact identity, destination device, original directed depth and dependency
custody after the public event, source allocation or earlier ancestors retire.
The multi-device KFD router translates only native parents to child-local
dependencies; settled cooperative parents remain authenticated router-owned
dependencies, without synthetic native events or GPU completion records.

Router custody is acquired before child submission. Definite rejection or
quiescent admission failure refunds it under the existing child contract;
terminal failure and unwind retain it. Observation and disposal retire only
the applicable consumer's custody. A quiescent diagnostic from another
operation cannot retire an independently pending consumer. Fan-out counts are
checked, and empty native-only rosters do not allocate peer-producer storage.
Flush scans the retained peer-consumer roster; no performance gain is claimed.

## Qualification

| Check | Result |
| --- | --- |
| Pre-change public settled-copy-to-launch regression | Intended `Validation(Unsupported)` failure |
| Final focused producer suite | 83 passed, 2 ignored |
| Unfiltered all-feature runtime library | 1,536 passed, 3 failed, 28 ignored |
| All-feature runtime doctests | 52 passed |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-features runtime check | Passed |
| Runtime formatting and staged whitespace checks | Passed |
| Two independent read-only reviews | No remaining blocker found |

Builds, tests and Clippy used offline mode, `CARGO_INCREMENTAL=0` and
`CARGO_BUILD_JOBS=1`. Counts overlap. The three full-suite failures are the
`authorized_execution` telemetry tests listed in `qualification.json`; all
report `InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`.
They were not skipped or counted as passes.

Public Context coverage includes exact copied bytes, event-independent
retention, released remote source, retired ancestry, original depths 2/255/256,
duplicate event aliases, wrong destination device, corrupt retained identity,
pending/failed/unknown results and eager physical completion without logical
settlement. The directed depth-255 parent admits a depth-256 consumer; a
depth-256 parent rejects the next edge before effects.

Router tests cover both device orientations, actual host-staged copy completion
into a scripted child compute ledger, mixed native/peer dependency translation,
pending poll/wait/drain, cancellation, exact release, identity/destination
authentication, retain overflow, admission unwind and fan-out. Quiescent
observation injection and flush-selection records are CPU boundary tests, not
reproductions of a confirmed native admission failure or GPU execution.

The archive retains the initial fixture compile errors, corrected intended
baseline failure, three intermediate one-child fixture failures, 82-pass
intermediate run and all final results. These earlier attempts do not qualify
the final source. Commands, toolchain identity and signed source receipt are
included in the archive. GitHub issue queries failed; the empty initial JSON
output is explicitly not an issue-state receipt.

`receipts.tar.xz` has SHA-256
`509fa57e66723c78267a5b0ab0e478d1f4f3f603251950c68cd1daf843a04120`.
The archive comparison against the complete original evidence directory passed.

## Remaining Gates

Pending peer-copy-to-compute is still unsupported. The router needs an explicit
directed-copy contract, exact original read-range checks, an external readiness
gate in existing compute ownership, authenticated internal copy access despite
consumer allocation custody, and coverage of every publication/progress path.
The separate native-XGMI backend still has no compute composition.

The unchanged finite completion projection excludes mixed-kind edges; it does
not establish refinement for this extension or complete Context adapters.
There is no new Worker, device-language or atomic/collective authority.
MI300X hostname resolution failed before remote execution, so no remote process
or artifact was created. Native replay and matched HIP/HSA performance remain
unmeasured. This packet does not close a milestone or establish runtime parity.
