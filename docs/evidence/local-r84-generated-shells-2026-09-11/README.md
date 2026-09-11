# Local R84 Generated Shell Evidence

Implementation baseline: signed R83 `e07de3bfb87955fc885ef0c88788678ce8170aaa`.
Parent topic commit `e7334b8f2c0d8ddf2645e6c6d9c84746da108d81` and intervening
`fb581115bae626ae03c4bd9af9daa37c9205c692` change planning documents only.
This record covers the
[generated allocation-shell contract](../../runtime-generated-allocation-shells-v1.md).

## Status

All seventeen final frozen-source gates pass in `r84-accepted`, with 5,616
non-documentation source identities unchanged. The focused checks, unchanged
proof inventory and production dependency audit also pass. Exact commands and
counts are retained in [the gate record](raw/r84-accepted-source-gate.json) and
[test summary](test-summary.json).

Native adoption hooks remain uninstalled. These results validate inert source,
metadata and accounting transitions; this packet adds no GPU execution or new
Verus theorem.

## Scope

The generated storage representation moves the original image, complete data
roster and inert packet without copying encoded buffers. Opaque source identity
and private one-shot transfer bind registration to the reserved carrier. A single
ordinary/generated backend table preserves the existing handle namespace without
ordinary byte shadows. Whole-roster Context registration reserves all logical
credits before commit; exact metadata retirement validates every retained member
and inverse owner membership before disposal/refund.

Twenty-five new runtime CPU test functions, two actual-host charged-storage test
functions and seven new runtime compile-fail doctests cover this packet. The
tests distinguish unchanged preflight rejection from accepted metadata ownership,
preserve unused/read-only ordinals and check late corruption, ordinary API
isolation, ID/credit exhaustion and drop/unwind ordering. Runtime positive shell
tests use the inner transaction with model-only identity/authority fixtures;
host tests use actual charged storage/readback. Neither establishes successful
protected construction or actual charged-carrier native adoption.

## Acceptance

| Gate | Result |
| --- | --- |
| Five runtime crates, GNU all features/targets | 2,388 passed, five existing ignores, 48 reported harnesses |
| Same runtime scope, musl | 2,388 passed, five existing ignores, 48 reported harnesses |
| GNU runtime plus host doctests | 108 passed |
| musl runtime / default host doctests | 91 / 16 passed |
| GNU all-feature host / musl default host | 248 passed, four existing ignores / 131 passed |
| Generated macro fixture harness | Seven passed |
| All-feature/all-target and production library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy/tests and local CI gate | Pass |
| Standalone lockfiles | All 32 pass |
| Focused shells / storage | 19 / nine passed |
| Existing unpublished lifecycle / complete async suite | 15 / 233 passed |

Focused checks are subsets of the full suites, not additional independent test
counts. The storage selection has seven runtime and two host tests; one runtime
test also appears in the shell selection. The unchanged negative-proof inventory
passes 686 files without running Verus. The production musl metadata audit passes
43 packages and eight permitted build scripts; metadata SHA-256 is
`6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.

Cargo uses `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. The complete
non-documentation tree was hashed before and after the gates and checked again
when retaining the evidence. Model/accounting/completion and root Cargo inputs
remain unchanged from signed R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.

## Intermediate Checks

The runtime/host all-feature, all-target compilation check passed after the
initial common-storage refactor. New test compilation initially needed a missing
geometry import and the correct existing `Read`/`Write` access variants.
The first shell run passed thirteen tests and failed two mock-state assertions:
the backend already has one vacant auxiliary lane. The tests now check that
preexisting lane has no owner, native state or active work. All sixteen shell
tests in the next run passed. Further regressions were added afterward, so that
run is not the frozen-source acceptance result.

Two new actual-host charged-storage tests and seven runtime tests selected by
`generated_storage` passed; one of those runtime tests is also in the shell
selection and must not be double-counted. Three read-only workers reviewed
backend ownership, source/host custody and Context accounting. Their review
conclusions do not replace tests, a solver run or Linux qualification.

The first full `r84-final` attempt passed runtime/host tests, doctests and macro
fixtures, then stopped at Clippy's large-enum-variant lint for the allocation
table. The final candidate explicitly preserves ordinary records inline instead
of adding a per-record heap allocation to preflighted insertion. Its targeted
lint exception and storage tradeoff are documented. A fresh full frozen-source
attempt was required; the failed attempt's original logs are retained separately.

The `r84-release` attempt additionally passed all-feature lint, then production
lint found `has_generated` unused outside qualification/tests. It is now gated
to those callers. Both earlier attempts remain distinct from the final source
acceptance; neither partial pass is promoted to a complete release gate.

## Open Acceptance

Positive protected construction, actual charged-carrier native integration,
native prefix/lane adoption, publication, typed completion, Linux qualification,
executable refinement and matched performance remain open. No SSH or GPU work
was started for this packet; no shared-machine cleanup was needed.

## Retention

Raw commands/logs and all three source-gate attempts are retained without merging
failed attempts into acceptance. The final source inventory lives in
`raw/r84-accepted-source-inputs.json`. `source-files.sha256` is repository-relative;
`retained-files.sha256` is evidence-directory-relative. Both manifests are checked
before signed publication to both topic remotes. Raw logs and the executed
`r84-source-gate.py`, `r84-auxiliary-gates.py` and `r84-doc-links.js` snapshots retain
their original whitespace, including trailing blank lines. Staged authored-source
whitespace checks exclude only those three raw snapshots and this packet's
`raw/*.log` files; their exact bytes remain covered by the retained hash manifest.
Documentation links are checked again after the acceptance/roadmap updates.
