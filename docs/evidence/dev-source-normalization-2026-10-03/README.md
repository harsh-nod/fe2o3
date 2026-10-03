# Typed Source-Statement Normalization Qualification

Local CPU/Verus checkpoint, 2026-10-03. Base:
`bc03cb84983a19c516225e99814d16a04a8a1976`. The archive contains the candidate
source patch, complete command/source receipts, retained diagnostic attempts,
solver inputs/outputs, executable identities, replay auditor and file manifest.
`SHA256SUMS` binds this record and `qualification.tar.xz`.

## Change and Boundary

The existing checked-u32 entry-prefix checker now forwards actual borrowed
semantic MIR types, locals and statements to four shared executable normalizer
bodies. Ordinary Rust and Verus use those same bodies. Acceptance remains limited
to Nop/count0, unprojected typed Copy/count0 and u32 Constant/count1, with exact
destination/result type identity. Constants must have four-byte representation
and fit u32 before narrowing. Individual normalization allocates nothing.

Exact acceptance, direct AST statement denotation and origin-step composition
are proved. The model includes every variant of the six inspected AST enums and
every inspected field; irrelevant payload types are explicitly erased. Boxed
sequences become modeled sequences with preserved length/order. Reviewed token
guards bind actual schemas, getters, typed-ID implementation and caller forwarding;
they reject conditional/nested declarations, macro token-tree relocations and
unexpected getter attributes. Whole-byte pins additionally bind reviewed wrappers
and contracts. This bridge is not a theorem about Rust parsing, layout or the
entire semantic MIR implementation.

This does **not** prove ABI discovery, KIR normalization, whole-prefix/span
assembly, compiler execution authenticity, machine entry, continuation, memory,
completion or protected launch admission. Diagnostic `normalization_adapters_proved`
therefore stays false. No public authority path was added or relaxed.

## Accepted Qualification

- Pinned Verus `0.2026.08.09.92f466f`: 45 verified obligations before and after
  33 intended logical mutants. Seventeen mutants target the new normalization;
  the previous sixteen basis/fold mutants remain. Eleven runner controls pass.
  Both authenticated release-closure checks pass: 190 files, 129,019,839 bytes.
- All 38 solver stages pass with exact selected theorem/contract diagnostics,
  unchanged source snapshots, exact staged bodies and absent owned process groups.
  Function-exit diagnostics are accepted only for the exact pinned one-macro
  source-step wrapper and target postcondition. Type, translation, bounds,
  timeout and unexpected diagnostic failures are not logical-negative acceptance.
- Verifier library: 125 passed, four ignored; public checked-u32 integration:
  seven passed. The complete 136-test roster is matched to outcomes. Five new
  tests cover actual typed inputs, rejected effects, count/type mismatches,
  exhaustive small copy states and distinct live lowerer owners.
- Thirty documentation tests pass. Strict all-feature library Clippy,
  no-default-feature library check, changed Rust formatting, shell syntax,
  `git diff --check` and local CI-dispatch tests pass.
- Genuine pinned-rustc extraction passes two positive source profiles and four
  expected rejections. Copy/reassignment resolves to argument 0; the empty prefix
  resolves to argument 1. Distinct source/KIR identities and boundary evaluations
  are checked. The owned compiler scratch directory is absent after completion.

Final acceptance uses `proof-05`/`solver-05` and the final CPU cohort declared in
`final_cpu.py`, replayed by `audit.py`. Command bounds, explicit environments,
stdout/stderr hashes and whole-source snapshots are retained. Cargo JSON receipts
identify the verifier, public integration, extraction test and compiler wrapper
executables. Rust uses `nightly-2026-04-03` with overflow/debug assertions enabled
for tests. No GitHub Actions execution or GPU run is claimed for this checkpoint.

## Non-Acceptance and Remaining Work

Earlier proof attempts are retained, not silently counted: a wrong module
selector, an extra cast recommendation, a function-exit diagnostic shape, and an
unsupported or-pattern-with-guard mutant were corrected. The fourth attempt also
lost source continuity while review fixes landed. Final `proof-05` is clean.
An earlier extraction child passed but its source snapshot changed during the
run; only the final unchanged-source extraction is accepted.

The broader strict test-target Clippy attempt fails on existing fixture-module,
dead-code, option-env and constant-assertion warnings outside this change. The
accepted Clippy scope is the production library, not all targets. The four ignored
verifier cases are subprocess helpers and the pinned functional-refinement runtime
closure test, not hardware tests. Existing rustc target-feature warnings remain.

Native multi-GPU behavior is unchanged; previous hardware evidence retains its
own source/binary scope. Next compose the actual span/KIR assembly, then finish
one bounded kernel's machine and protected Worker admission. A3, issue #182 and
HIP/HSA parity remain incomplete. There is no performance claim.
