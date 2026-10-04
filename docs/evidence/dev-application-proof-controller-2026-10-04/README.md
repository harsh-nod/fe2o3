# Application Proof Controller Qualification

Date: 2026-10-04. Source commit:
`25ee202a6d0561fc9793e83bc3ecf8aff498d18c`.
Parent: `f9fd8d104dfe99f2342839721eeeabaa634efc57`.
Source patch SHA-256:
`144a3f616807521d877c46949b4a2f4b09473803471addaea317e233f7e2cfff`.

This qualifies the [staged application controller](../../runtime-application-proof-controller-v1.md)
and CPU admission of generated-only native-peer contexts. It does not complete
ordinary application admission, native conditional invocation or multi-GPU execution.
Three native read-only reviewers assessed the implementation, qualification and
remaining integration path; primary integrated and tested the changes.

## Results

- 85 CPU tests passed: 77 across proof-custodian, runtime-protocol, protected-service
  profile/spawn; seven multi-device construction tests; one generated-only launch-gate test.
- Twenty compile-fail doctests passed. Strict Clippy passed for proof-custodian
  and protected-service-spawn (all targets), and runtime (library). The default
  runtime test build still emits its existing unused `snapshot` method warning.
- Both rebuilt musl controllers have the secure entry, no interpreter/dynamic
  segment, no NEEDED libraries and no unresolved symbols. Independent nonroot
  measurement generated fresh separate fixed deployment records.
- All 22 private real-root cases passed. Every outer qualification cgroup was
  removed; an independent task census confirmed each private PID namespace empty.

The four new application cases use a UID-1000 Cargo-role parent and separate
application, original transferred process handles, the Cargo-created proof pair,
and the actual installed UID-61000 controller:

| Case | Observed result |
| --- | --- |
| `app-good` | A request remained queued before Activate. After root aliases closed, the controller produced a genuine retained proof. Application Probe and root Probe recomputed the same subject. Application channel EOF, with app/Cargo still alive, yielded root QUARANTINED with that original subject. |
| `app-payload` | Altered HSACO with a recomputed transport hash rejected at `Closure(FinalizedLengthMismatch)`. |
| `app-duplicate` | Two aliases of the same input object rejected. |
| `app-stale` | A request carrying a different session identity rejected. |

Each helper completed its expected protocol and exited successfully. Cargo reaped
the application before root accepted cleanup; root checked Cargo success and helper
group absence. An unrelated sibling survived. Qualification containment of the
controller is not a production proof release or GPU settlement.

The existing 18 launcher cases were rerun: concurrent proofs, live-Verus and gated
cancellation, pending Drop, startup/proof/probe deadlines, normal proof/release,
empty-scope cleanup, ready cancellation, worker/runtime/controller measurement
mismatch, changed payload, same-byte config replacement, FIFO config and illegal
post-proof control. See the [prior reactor campaign](../dev-proof-custodian-reactor-2026-10-04/README.md)
for their individual contracts.

## Evidence And Limits

`evidence.tar.gz` contains source and binary hashes, the signed source commit check,
canonical deployment records, original inputs, retained subjects, exact scripts,
review notes and build/qualification/cleanup logs. `MANIFEST.sha256` covers members.
Archive SHA-256:
`370b549f82e79bcde61a251f531601dd82bbd1f649aa520cf637c6148d4d960d`.

Inputs came from the prior archived campaign; the compiler was not rerun. Actual
analysis and protected Verus execution were fresh. Registration image/policy/input
records are inert fixtures, not authenticated broker observation or genuine deployed
compiler-origin admission. The app helper does not run the ordinary no-fork sandbox.
Raw root staging must still be joined to the authenticated broker's original
occurrence and exact slot-4 counterpart observation through typed custody.

The first-probe regression checks aggregate-deadline selection on a real queued
probe; it does not inject a greater-than-30-second delay. Process tests and source
review do not formally verify every Rust/syscall lifecycle transition. The manager,
remote proof owner, native invocation join and settlement protocol remain pending;
manager/controller death must not be presented as GPU settlement.

No MI300X run, benchmark or parity claim is made. Host installation paths were
untouched; fixed-path deployment and runtime provisioning used disposable local
namespaces and fresh owned cgroups only.
