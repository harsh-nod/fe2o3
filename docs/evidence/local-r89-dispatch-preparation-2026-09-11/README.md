# Local R89 Dispatch Preparation Evidence

Implementation baseline: signed R88 plus planning commit
`0a040ece9816335fe4ad0178df9213eb84eca108`, published on both topic remotes.
This packet implements the
[CONTROL-2 preparation contract](../../runtime-fixed-dispatch-preparation-custody-v1.md).

## Status

All seventeen frozen-source gates and auxiliary checks pass, with 5,627
non-documentation source identities unchanged. Three final expected-negative
mutations fail at their intended assertions; restored source passes and matches
the accepted source inventory. Production, test-oracle and contract reviews
found no blocking issue. No SSH, GPU workload, remote staging or Verus solver
was started. This is local CONTROL-2 acceptance, not full native construction.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,454 passed and five ignored per target, across 48 harnesses each. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused preparation | Nineteen passed: eighteen behavioral tests and one source-location guard. |
| Existing transitions / allocation / shared memory | Twenty / fourteen / 195 passed; the first two selections are subsets of shared memory. |
| Existing retention / readback / charged / shell / storage / adoption / async selections | Thirteen / ten / fifty / 19 / nine / fifteen / 233 passed. These are subsets; storage and shell overlap by one runtime test. |
| Lint, formatting and policy | All-feature/all-target and production-library Clippy pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production closure | 151 Python tests pass; dependency audit passes for 43 packages and eight build scripts. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

R89 adds nineteen KFD test functions, no runtime/host test function, doctest or
theorem. The first full source-gate attempt, `r89-final`, passes. The
[summary](test-summary.json), [changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[gate record](raw/r89-final-source-gate.json) and
[complete source inventory](raw/r89-final-source-inputs.json) retain the result.
Raw logs, helpers, mutation patches and focused source snapshots are included.

Raw Cargo logs preserve terminal blank lines and whitespace from interleaved
test output. Whole-tree staged whitespace checks report those raw log lines;
source/document checks excluding raw `.log` files pass. Evidence is not
reformatted to hide those warnings.

## Scope And Oracles

Tests use the actual loader, existing planner, shared preparation sequencer and
R86/R88 adapters over fake-native records and the queue foundation. They cover
every program ordinal, code and kernarg stages, currentness/native error and
panic, partial maps, natural model-aperture rejection, final packet resolution,
commit, generation and both persistent wrapper shapes. Native-fault cases must
reach the requested stage, not merely return an arbitrary earlier error.

Oracles compare original data descriptors, full native data bytes and records,
unchanged N1/N2 usage, exact control token ownership and terminal handoff markers,
complete materialized code bytes/digests, authenticated/ABI identities, actual
mapping/descriptor addresses, patched kernarg bytes, roles and generation.
Completed-output retention is exercised through scripted caller unwind. The
two production bind placements also have a source-location guard; neither is
the full actual operation/retake/validation cross-product required by CONTROL-3.

## Intermediate Attempts

The earlier focused fixture attempts remain distinct from final acceptance:

| Attempt | Result and correction |
| --- | --- |
| `r89-preflight` | All sixteen tests rejected an invalid all-zero fixture signature; use nonzero checked signatures. |
| `r89-corrected` | Fourteen passed, two failed. Correct the expected repeated writable ranges for two packets and compare the derived ABI identity, not a raw signature. |
| `r89-expanded` | Seventeen passed, one failed. Existing three-binding admission requires all three inputs initialized; correct the fixture without weakening admission. |
| `r89-refined` | Eighteen passed, one source guard failed. Accept rustfmt whitespace and its optional trailing comma while still requiring the exact preparation owner in both terminal branches. |

Earlier fixture compilation mistakes and the early production KFD check are
available only in the execution transcript, not retained raw logs. They are not
substituted for final gates. `r89-ready` passes nineteen tests; `r89-hardened`
adds exact resolved-address and native-fault-stage assertions and passes the
same nineteen tests. No behavioral oracle was removed to obtain acceptance.

The preliminary current-token mutation already rejected before those final
assertions. It was restored and rerun with the final tests as one of three
isolated mutations, each producing one failed test and Cargo exit 101:

| Final mutation | Intended rejection |
| --- | --- |
| Move current CPU token out of its owner before borrowed materialization | Actual native-access panic preserves its original payload, but the exact native-record oracle reports the missing control owner. |
| Drop original initialized-content descriptor at commit | Full-success comparison observes `None` instead of the original descriptor. |
| Remove the pre-map `InSession` marker | Actual map/currentness failure retains the R88 token but lacks its exact preparation handoff marker. |

From `r89-ready` onward, each focused receipt snapshots the nine R89 Rust files
before and after execution. The final mutations differ from accepted source
only in `preparation.rs`; hardened/restored snapshots equal each other and the
complete final source inventory. Earlier focused logs do not independently
snapshot their historical source. No mutation remains.

## Open Boundaries

CONTROL-3 retains later validation/attachment, simultaneous operation/retake
panic handling and terminal-dominant retry classification. NATIVE-2 retains
ordinary outer preparation, primary/auxiliary/replacement constructor custody.
DATA-ADOPT, ISSUE, runtime completion, generated public API/graph/drain,
control/aggregate budgets and native qualification remain open. Existing
backend mappings that never return, unmap/release/teardown custody and
process-aborting allocation failure are not repaired here.

Model, resource-accounting, completion and manifest/lockfile inputs remain
unchanged from the R73 proof checkpoint. Its 62 positive sources and 1,374
obligations are historical evidence, not proof of this new adapter. No Linux
execution, new executable refinement, performance gain, A1/A2 completion or
HIP/HSA parity is claimed.
