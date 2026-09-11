# Pristine Dispatch Abort

R82 implements the never-published fixed-dispatch abort/rebind prerequisite
against signed dispatch baseline
`3c534e88843a8fdd64a5b3397ec6937ce3696b48` on
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-pristine-dispatch-abort-v1.md) separates this transition
from generated adoption, publication and completion.

## Scope And Review

The new move-only continuation preserves the exact next generation without a
synthetic recycled receipt. Admission checks every epoch slot and complete data
premise before effects. Control cleanup retains the complete ordered data owner;
only successful cleanup and closing retake expose detached data. Partial cleanup,
closing failure and panic retain terminal custody without retry. Rebinding
consumes the continuation and roots a successfully prepared owner before checking
closing or validation failure. Primary and auxiliary lanes preserve distinct
unpublished provenance through switching, destruction and unwind.

Three read-only agents reviewed native custody, runtime lifecycle and accounting.
Primary implemented and tested the changes. Review found consuming rebind
preflight could leave a usable continuation and cleanup error could omit process
poisoning after a successful closing retake. Both are fixed with regressions.
Fault coverage was expanded from kernarg to all three control ordinals, and
confirmed disposal is now observed separately from attempted frees. The
currentness matrix asserts all eighteen boundaries before sweeping them.

Nineteen new KFD CPU test functions cover exact generation/history, complete
roster and descriptor retention, malformed premises, control disposal errors
and panics, malformed/partial unmaps, currentness, lane restoration, loan and
closing outcomes, rebind settlement and terminal process gating. These fixtures
use real fake-native allocation records but synthetic dispatch/Device
initialization premises. Scripted model-envelope outcomes are not Linux model
loan/retake or production-constructor qualification.

## Acceptance

The frozen-source `r82-final` attempt passes all seventeen gates with 5,607
non-documentation source identities unchanged. Exact commands, logs and
counts are retained in `raw/` and `test-summary.json`. Focused checks are
included in the full suite totals, not additional independent test counts.

| Gate | Result |
| --- | --- |
| Five runtime crates, GNU all features/targets | 2,348 passed, five existing ignores, 48 reported harnesses |
| Same runtime scope, musl | 2,348 passed, five existing ignores, 48 reported harnesses |
| GNU runtime plus host doctests | 101 passed |
| musl runtime / default host doctests | 84 / 16 passed |
| GNU all-feature host / musl default host | 246 passed, four existing ignores / 129 passed |
| Generated macro fixture harness | Seven passed |
| All-feature/all-target and production library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy/tests and local CI gate | Pass |
| Standalone lockfiles | All 32 pass |
| Focused pristine abort / Host backing | 19 / 36 passed |

The unchanged negative-proof inventory passes 686 files without running Verus.
The production musl dependency audit passes 43 packages and eight permitted
build scripts; metadata SHA-256 is
`6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.
Local documentation links and anchors pass. The three-agent follow-up breakdown
is recorded in the [current dispatch](../../runtime-a1-a2-next-wave.md#next-three-assignments)
and does not add implementation or acceptance for its queued packets.

Cargo uses `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. Preliminary
compilation found a missing module path and a test assertion requiring an
unsupported error comparison; both were corrected before final gates. The
native preparer's visibility was narrowed to remove a private-interface warning.

## Open Boundaries

R82 adds no authenticated Verus theorem, solver rerun or executable adapter
refinement. Existing epoch/recycle/cost proofs do not prove this new transition.
Negative-proof inventory checking is not solver execution. Model,
resource-accounting, completion and root manifest/lockfile inputs are unchanged
from signed R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.

Signed same-queue Linux abort/rebind/disposal, primary/auxiliary lane coverage,
actual initialized Device contents and adapter refinement remain open. Rebind
constructor failure retains underlying native records, not a recoverable
complete Rust vector. Native generated adoption still needs ADOPT-LIFE and
DATA-ADOPT before ISSUE/COMPLETE. Neither HIP/HSA parity nor speedup is established.
No SSH, remote staging or GPU process was started for R82, so no shared-machine
cleanup was needed.

## Retention

Final commands, raw logs, source identities and counts are retained in `raw/`
and `test-summary.json`. `source-files.sha256` is repository-relative;
`retained-files.sha256` is relative to this directory. Source and evidence
hashes are checked before signed dual publication. Preliminary focused output
is not represented as retained final-gate evidence.

Raw Rust test transcripts retain their original trailing spaces and final blank
lines. The post-retention staged whitespace check excludes `raw/*.log`; the
unrestricted staged check reports only those transcript whitespace warnings.
