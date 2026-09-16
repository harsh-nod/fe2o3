# Issue 271 Integration Snapshot

This is a reviewable work-in-progress branch, not production qualification or
milestone completion. Do not merge this historical source tree over current main.
Publish selected changes against current main with fresh qualification instead.

## Source And Test Identity

- Source checkout: `/home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913`.
- Source checkout HEAD: `10b190b8267c0c4011b4ef6bbb52680a3c44b391`.
- Source manifest: 5061 files, SHA256
  `fa5ef440059b572166fd5e2ca4ff4c7f9fd43619b65f641bff540ac311ad376d`.
- Previous WIP commit: `7f7f475446a50c5061c178e8109e18e8944dbbe1`.
- Exact source run: `v334-r1-r2-native-eight`, terminal exit 0, eight library
  suites, 2993 passed, 0 failed, 0 ignored. Source and test-helper guards passed.
- Command: `cargo +nightly-2026-04-03 test --locked --no-fail-fast --lib`
  for `dialect-gpu`, `fe2o3-kernel-ir`, `fe2o3-kernel-analysis`, `fe2o3-pliron`,
  `fe2o3-kernel-opt`, `fe2o3-amdgcn-model`, `fe2o3-lower-mir-kernel`, and
  `rustc-codegen-fe2o3`.
- Local evidence:
  `/home/harsh/work/fe2o3-issue271-diagnostics-20260910/v257-clean-v334-r1-r2-native-eight.7FYvwhaM`.

The added status file is not part of the tested source manifest. The snapshot
procedure verifies every source blob and preserves the checkout HEAD/index.

## New Since The Previous WIP

- Exact U64 literals and their exact U64-to-index casts in formal address
  extraction. This does not admit general dynamic unsigned arithmetic.
- Independent policy-3 semantic replay, separate from untrusted execution
  claims. Replay does not recreate a protected optimizer owner or authenticate
  remote execution history.
- Borrowed source-ranked authentication against the retained original source
  owner, with repaired genuine multi-root fixtures.
- Full-ranked/source-to-optimized physical-address correspondence for admitted
  direct-root GlobalSlice leaves. Genuine scalar-formal index positives and
  wrong-source-use negatives cover both target profiles.
- Exact native-V12 LLVM text/descriptor relation, plus pairwise entry-to-`.kd`
  descriptor binding and documentation. These are inert relations, not final,
  functional, load, launch, hardware, or LLVM refinement authority.

The eight-library run covers the new unit tests. The new native text/descriptor
integration target and compile-fail documentation tests still require execution
on this snapshot. Do not describe them as passed based on the library run.

## Remaining Work

M5/M7 production integration remains active. Source-first SSA capture, mandatory
functional/aggregate presence, physical-address checking before complete formal
analysis, and source-owned descriptor construction are being reviewed in newer
patches. Those patches are not included here and are not yet runtime-qualified.

The production default is unchanged. Native policy-3 lineage attachment,
protected compiler execution binding, verifier/host dispatch, and legacy
retirement remain separate work. Preserve the distinct source N, target-bound B,
and optimized O identities; semantic replay alone is not execution evidence.

The approved Verus runtime qualification remains deferred, not passed. No test
manufactures functional proof presence to bypass it. Full tutorial production
compilation, semantic/reference oracles, target-matched hardware, performance,
release, and website evidence-pin acceptance remain open. No M0-M9 signoff is
claimed by this snapshot.

Current main has peer RustCall/capture-helper work absent from this historical
integration tree. The publication candidate based on current main is qualified
separately. The separate issue-272 WIP branch is not changed or qualified by this
snapshot.
