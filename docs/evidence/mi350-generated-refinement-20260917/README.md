# Protected Generated Refinement Checkpoint

This records the 2026-09-17/18 UTC follow-up on MI350. It is not a production-safe
GPU launch or tutorial qualification report.

## Identities and Isolation

| Input | Identity |
| --- | --- |
| Final code checkpoint | `cf6f1137414d3e1e9c5cae6603ad1c94875aab24` |
| Final frozen source roster | 5,223 files; SHA-256 `2dcfdc5cf47648a0d2ed5cdbe360f924193292942c95aad5e99b4e6863dbb840` |
| Ordinary-source test checkpoint | `f69fbf998425bd08e2fafd9ac496e35e351b2a33` |
| Ordinary-source frozen roster | 5,221 files; SHA-256 `39f54bcf3a621ade0a19e484b3011e7561fe5ca23bf962d32660bfbaabcd6e9c` |
| Toolchain | `nightly-2026-04-03` |
| Runtime manifest SHA-256 | `ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c` |
| Temporary compiler-tools image ID | `sha256:cdbbaae92eb74959917ec3bfdbe175a8a44622ac42290eae334b1fa78ed2a055` |
| Retained protected-runtime image ID | `sha256:a2c76f4d0c6781a44d42479162479c48f6cfc44f0e00a21ada814b3b2ca6847d` |

The compiler-tools image derives from the previously audited runtime image,
adding build tools inside the image, not the host. The pinned runtime closure
was audited before every batch. Neither its pins nor `--no-cheating` changed.
Compiler/script inputs matched the frozen roster; subsequent documentation
changes are not part of that roster.

The final checkpoint moves unchanged byte-encoding helper bodies into a
focused module and cleans up three test-helper lint findings. Ordinary-source
tests ran before those changes; their separate source and harness identities
are retained rather than relabeled as final-checkpoint executions.

Tests ran with UID 61074, no capabilities or network, a read-only container
root, private namespaces, two CPUs, 12 GiB memory, 256 processes, and a private
writable Cargo cache/target. Sources and the compiler toolchain were mounted
read-only. The container-local outer seccomp exception permits the existing
supervisor to install and enforce its own proof-child filter. No host-wide
security setting was changed. Cargo was offline, single-job, and incremental
compilation was disabled.

## Protected Proof Results

All eight tests passed separately in **both debug and release**, with exactly
one passed and zero ignored for each selection. Test listings and terminal
counts were checked; ignored-test defaults cannot produce a passing batch.

| Cases | Debug | Release |
| --- | --- | --- |
| Public runtime true proof / false proof | 2 passed | 2 passed |
| Generated wrapping scalar / changed scalar | 2 passed | 2 passed |
| Generated effect receipt / changed coordinate | 2 passed | 2 passed |
| f32 operator congruence / changed operator | 2 passed | 2 passed |

The [debug](protected-debug.log) and [release](protected-release.log) transcripts
include the installed-runtime audit, exact selections, results, and timings.

The generated negative tests require a real Verus assertion failure and then
require receipt production to reject. A missing runtime, syntax error, or
deadline failure cannot satisfy them. The six generated tests retain the
compiler's 60-second per-proof deadline. The two existing public smoke tests
retain their 120-second deadline.

The initial attempt exposed a real generator error: the global uninterpreted
function declaration was forbidden by the pinned `--no-cheating` policy.
The fix quantifies a `spec_fn` argument, shared through aggregate replay,
rather than relaxing that policy. Integer nested bitwise-NOT grouping and
library-only source generation were also corrected.

The f32 tests prove identical operator trees congruent for every interpretation.
They do not prove target IEEE behavior, arbitrary Rust equivalence, or an
approximation error bound. Effect tests use typed ranked fixtures, not an
advanced tutorial kernel.

## Local Regression Results

- Host 93, kernel analysis 186, KFD 424, and runtime 275 library tests passed.
- After the module split and test cleanup, the complete five-library run
  passed all 1,088 tests at the final code checkpoint, with 13 ignored.
- The first verifier run had 109 passes and one controller-test timeout
  (`TimedOut` instead of the expected `Process` rejection). The unchanged full
  verifier rerun passed all 110 tests. Twelve verifier and one KFD test were
  ignored in ordinary runs; the protected tests above were selected explicitly.
- EXEC integration: 19 passed. Authenticated f32 rejection integration: one
  passed. The initial untaken-path arithmetic test had an invalid unreachable
  CFG fixture; it was repaired to use a reachable conditional edge before the
  final passing run. Production validation was not weakened.
- The aggregate replay script passed with `--no-cheating`: single-output and
  multi-output proofs verified, and all three semantic mutations were rejected.
- The shared EXEC harness passed the pinned development Verus with
  `--no-cheating`: 14 verified, zero errors, plus its unused-macro warning.
  These conditional arithmetic/mask lemmas are not protected machine-refinement
  evidence.
- Formatting, source-growth hygiene, and workspace dependency policy passed.
  Final selected Clippy completed with no findings in newly changed code;
  existing production and test warnings remain, including three constant
  assertions in the process-controller tests. Strict repository-wide Clippy
  is not claimed to pass.

The authenticated f32 integration no longer accepts its old arbitrary-byte
fixture. Private checker tests cover valid encodings and artifact round trips;
a genuine positive authenticated analyzer replay remains a coverage gap.

## Remaining Acceptance Gate

The ordinary positive Rust reference reaches protected functional proof, then
fails `FE2O3-OWN-002` because its dynamic launch cannot be admitted by finite
ownership tracing. Its strict successful-callback test is intentionally not
weakened. The integrated symbolic `TotalView` coverage proof and launch/extent
premise are still missing; see the [integration status](../../production-safe-launch-integration-status.md).

Both debug and release runs had the same positive failure and passed the
mutated-reference test, which requires the actual Verus assertion diagnostic.
Their [debug](source-debug.log) and [release](source-release.log) transcripts
retain the unsuccessful positive result.

The complete reviewed-host gate is not green. Its separate legacy
authenticated-execution fixture suite was not rerun in this checkpoint.
Production protected-verifier and machine-refinement backends, independent
service deployment, source-to-machine refinement, and GPU completion remain
open. No GPU kernel was launched, no tutorial qualification was added, and no
website coverage count was changed.

While validation was running, `main` advanced separately to
`1ca7e12974ec77f28ea250847b7b3ae93182232c` with checked memory-forwarding and
guarded-output stages. This branch's tested base is `70aadc048`; the report
does not claim integration testing of that newer main checkpoint.

## Evidence and Cleanup

[Final library tests](final-libraries.log), [integration tests](final-machine-integration.log),
[Clippy](final-clippy.log), [conditional EXEC lemmas](exec-lemmas.log), and
[aggregate replay](aggregate-replay.log) record the local checks. The initial
library timeout and successful rerun are retained as separate transcripts.
[The log manifest](logs.json) records SHA-256 values for both original and
published bytes; published transcripts only normalize trailing whitespace and
terminal blank lines.
The local archive also retains the complete source rosters and staging,
build, provisioning, execution, and cleanup scripts.

The final remote source roster was [rechecked byte-for-byte](source-recheck.log).
Both evidence archives were retrieved and validated before deletion. The owned
MI350 scratch directory (about 3.69 GB of logical contents) and temporary
compiler-tools image were [removed](cleanup.log), with no owned test containers
or private test-UID processes remaining. The reusable protected-runtime image
was retained. The owned 2.08 GB RAM-backed local source/build directory was
also [removed](local-cleanup.log). No shared source worktree or other team's
cache was deleted.
