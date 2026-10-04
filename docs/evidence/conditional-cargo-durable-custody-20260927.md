# Cargo Custody Through Conditional Persistence

Date: 2026-09-27. Base: `cc0f60d9736981aecf05ee40b13bf30070f2f18d`.
Follow-up to [conditional durable publication](conditional-durable-publication-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**The native Cargo continuation now connects finalization to independent durable
recovery while retaining its original parent/readiness custody. It compiles and
its accounting and native-boundary components pass tests. This continuation is
still unselected by production; no complete positive conditional source/Worker/
journal round trip, protected execution, full M0-M7 milestone, or 47/47 GPU
completion is established by this checkpoint.**

## Implementation

`ReadyCompilerExecutionAttemptV3::finalize_after_compiler_success` now returns a
`ParentDurableConditionalArtifact`. It borrows one independently admitted policy
view for finalization, then moves that same view into persistence. The selected
child's successful completion and policy provenance remain obligations of the
outer supervisor/broker; this method does not authenticate either from caller
claims.

Prepared and durable states share a private `ParentArtifactCustody` containing
the actual execution carriage, readiness with its exclusive original-budget
borrow, parent invocation reference and configuration reservation. Transition
consumes the prepared publication through the existing conditional journal
adapter; it does not detach or reconstruct parent custody from stored bytes.

Before persistence and after independent recovery, the shared revalidator checks
the retained input floor, transcript/artifact coordinates, exact readiness,
parent invocation, and source receipt against the retained execution carriage.
The durable result offers borrowed observations and revalidation only: there is
no constructor from a standalone recovered record, conversion to fresh
consumption, detached-parts escape, or new load/launch authority.

The continuation error now owns a private cause and has no public error-source
chain. Diagnostics and owned transaction errors are retained, but a nested
resource failure cannot invite blanket refunds after source custody has already
been consumed. Terminal recovery remains outside ordinary restoring scopes.

## Exact Retirement

The private replacement helper records the original budget address, work-ledger
identity, entry reservation `E`, and retired prepared-publication/parent-header
charge `P`. Its scratch charge is `F`. For a new recovered publication charge
`R` and durable-parent header `H`, the successful sequence is:

1. Reserve scratch: `E + F`.
2. Complete independent recovery, whose returned owner charge is unreserved;
   require exactly the same `E + F` on the original account.
3. Reserve `R`, then revalidate retained parent/readiness/receipt custody with
   the recovered artifact: `E + F + R`.
4. Only after success, retire `P + F - H`, leaving `E - P + R + H`.

The former publication has already been dropped after exact fresh/recovered
comparison in the journal adapter before its reservation is retired. Carriage,
readiness, configuration and parent reservations remain. A replaced account,
hidden/missing charge, arithmetic overflow, unpaid header or resource denial
refuses without retirement. Error and unwind have no automatic refund guard.
Existing work, peaks and denial history survive. This logical accounting is not
a whole-process allocation or RSS bound.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time, with compiled source frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Cargo binary: `compiler_execution_boundary::native::pipeline` | 13 passed |
| Cargo binary: `compiler_execution_boundary::native` | 20 passed |
| Five-package `--all-targets` check | Passed; warnings remain |

The two test selections overlap. The broader selection also exercises existing
sealed configuration, reserved descriptor, readiness and exact receipt checks;
these fixtures explicitly do not authenticate a protected service boot. New
tests cover exact replacement accounting, short input/work/storage, changed
reservations, changed work ledger or budget address, overflow, header payment,
refusal/unwind, preserved denial history and opaque continuation errors. They
do not fabricate a successful compiler, readiness or semantic proof owner.

The build check covers `cargo-fe2o3`, `rustc-codegen-fe2o3`,
`fe2o3-hsaco-finalize`, `fe2o3-compiler-execution-client` and
`fe2o3-artifact-transaction`. Dead-code warnings continue to identify the
unselected native path; a successful check is not runtime activation.

Rustfmt and whitespace checks passed. No unsafe code, dependency change or
inventory allowance was added. The preexisting unsafe-inventory failure remains
open and was not rerun here. All processes terminated and private scratch
`/tmp/fe272custody.PgdAvgoX` was removed.

Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
0e40173b12615f8fafe4fcfad53f62373d56ae7626c53c075bb94e908ba692ed  conditional-custody-pipeline-20260927-r1.log
d728bde99615559fb83ad8dfc938da4dd50343bc6a8ac6cd5cb8eab0a148f220  conditional-custody-native-boundary-20260927.log
7488cc384523a3d2f09cc68144c5ae33d5224efbf715238d7853774b01f38aca  conditional-custody-all-target-check-20260927.log
```

## Positive Replay Gap

The existing `production_conditional_source_wrapper_v1_tests.rs` fixture has no
global output and intentionally refuses aggregate continuation with
`unsupported canonical conditional output`. Other canonical/ranked output tests
exercise genuine component graphs but do not construct an authenticated source
request. The V2 formula fixtures similarly test inert CPU/statement components
and strict import refusals, not genuine formula execution. The V4 signed-content
exporter cannot substitute for conditional V5 source evidence.

The next positive test needs a real output-bearing semantic source and its exact
ranked access/effect correspondence, a checked conditional aggregate request,
matching CPU input and genuine V2 formula execution/import, then the exact final
graph/catalog/descriptor and V5 handoff. That evidence can exercise persistence,
restart and policy/source/provider/transcript/output substitution refusals. A
locally signed stand-in or manufactured private owner would not close this gap.

Independent root-policy admission, selected parent/profile/broker/driver
activation, protected compiler receipt continuity, sealed verifier/generated
host integration, machine/numerical refinement, target-matched 47/47 runs,
selector retirement and release gates remain open.

Native agent spawning again hit the thread limit; implementation and review were
local. All three GPU SSH aliases and GitHub fetch failed DNS from this execution
environment. No remote job or file was created. Push and issue-update outcomes
are reported separately from these local results.
