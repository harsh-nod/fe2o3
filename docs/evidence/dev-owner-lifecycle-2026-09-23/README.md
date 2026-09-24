# Constructor-Origin Owner Lifecycle Correspondence

This is a source-bound developer proof checkpoint for the named owner lifecycle
relations. It is not native GPU qualification, whole-runtime verification, or
HIP/HSA parity. The production Context still rejects pending-producer reads.

## Scope

Separate actual-owner and independent logical relations cover fourteen mutation
families, twelve getters and eighteen query families. Matching inputs do not
presuppose matching results or post-state representation. Constructor-origin
induction derives representation, owner invariants, result agreement, fixed
capacity shape and historical reader non-reissue at every reached prefix.
Failed constructors admit no continuation. Successful construction retains an
explicit storage-admission premise; machine-sized input and physical storage
conditions are not removed.

Concrete no-precondition witnesses exercise constructor failure, a mixed trace,
and a thirteen-event reader trace. The latter includes producer settlement,
writer-slot reuse, retained producer lookup and stable-reader incarnation reuse.
Actual and model calls, returned payloads and exact event/state prefixes remain
separate until their correspondence is established.

Seven smaller Begin proof helpers and localized stable-release proof unfolding
resolve the earlier solver resource failures without changing the three original
function contracts or production Rust bodies. The source checker locks those
contracts, the original/helper bodies and all surrounding source independently.
The [maintenance record](../dev-owner-lifecycle-proof-maintenance-2026-09-23/README.md)
preserves development measurements and earlier failures.

## Qualification

Exact signed source: `0da85d666e17988d65cce7a30c7ee67319487e45`.

The complete recorder campaign contains 62 accepted terminal commands and 190
raw record files, with 449 unchanged source inputs:

- Both entire-lifecycle positive runs: 1,240 verified obligations, zero errors.
- All 49 scoped logical negative controls reject under the strict logical-failure policy.
- Raw-owner regression: 384 verified, zero errors.
- Inspection regression: 1,179 verified, zero errors.
- Constructor regression: 1,156 verified, zero errors.
- Both pinned Verus distribution checks: 190 files, 129,019,839 bytes.
- Recorder selftest, Rust compiler identification and formatting pass.
- Runtime-model unit tests: 1,021 passed, 18 existing ignored; 27 doctests pass.
- All-target strict Clippy and release test compilation pass.

The obligation counts overlap and must not be added as independent coverage.
The 49 negatives comprise 28 actual/model transition-relation controls, twelve
getter controls and nine constructor/input/history/domain controls. They are
logical mutations, not 49 production executable-body or machine-code mutations.
Frontend failures, arithmetic faults, resource exhaustion, abnormal termination
and incomplete process-group cleanup cannot satisfy the negative policy.

Whole-root commands retain 900-second bounds, scoped commands 600 seconds,
four verifier threads, default SMT limits and `--no-cheating`. The recorder
authenticates fifteen Python helpers before import and isolates authoritative
Git reads from environment overrides and replacement refs. The historical
inspection selftest runs against its frozen 437-file closure in the outer
recorder's owned process group. Rehashed source-policy controls remain distinct
from solver controls.

The older inspection campaign remains failed at its constructor timeout. Its
successful prefix is not reused by this packet. This campaign began afresh and
resumed only its own clean, exact-source checkpoint after fourteen negatives.
Raw output, including trailing whitespace, is retained without normalization.

## Replay

From the `SOURCE` checkout, run
`crates/fe2o3-runtime-model/verus/check-owner-lifecycle.py` with `--repo`,
`--verus`, `--output` and `--target`, using fresh output and owned Cargo target
directories. The exact command shapes and tool identities are in `records`.
Do not use `--probe` for source-bound qualification. `--stop-after` and `--resume`
allow clean checkpoints, not promotion or replacement of failed records.

Run `/usr/bin/python3 -I -B audit.py --repo <Git-repository> --selftest` from this packet to
audit the complete records and execute the corruption controls. The auditor
reconstructs sources from Git objects and supports a bare repository. It checks
all source identities, command/result rosters, strict proof outcomes, regression
roots, verifier naming, CPU counts, receipt ordering, cleanup observations and
the complete acceptance ledger. It does not rerun Verus or verify signatures.

## Limits

This composes named normal-return contracts; it is not an arbitrary executable
trace interpreter or a proof of all runtime call paths. Physical allocation and
Vec behavior, pointer identity, panic/unwind/destruction, cross-owner context
freshness, native publication and producer-result reconciliation remain separate
obligations. Historical execution and process-group absence are recorder
observations, not independently attested machine facts. CPU testing is not a
hermetic build attestation. The frozen-source operating model excludes a hostile
writer racing file authentication and path-based Python imports.

No native capabilities are enabled. No GPU operation, copy benchmark or HIP/HSA
comparison ran for this checkpoint. A1/A2 and broader runtime parity remain open.
