# Pinned Wrapping Shift Production Expansion

Pinned rustc: nightly-2026-04-03, commit
`55e86c996809902e8bbad512cfb4d2c18be446d9`.

The reviewed precompiled host-core shape of the safe inherent
`u32::wrapping_shr(u32, u32) -> u32`
body masks the count with `31`, branches on `UbChecks`, conditionally calls
`u32::unchecked_shr::precondition_check(masked_count)`, and returns
`ShrUnchecked(value, masked_count)`. The precondition tests `< 32`.
The proof validates all blocks, operand roles, signatures, inline helper
identities, helper bodies, failure construction, effects, and graph budgets.
For every u32 count, `count & 31 < 32`, independent of runtime UB-check mode.
No concrete input samples establish this proof.

The retained-call matcher also checks the captured three-block entry: exact
core `u32::BITS` is evaluated to 32, `SubWithOverflow(BITS, 1)` and its original
overflow assertion are authenticated, and the masked count is passed to the
exact unsafe helper. Its complete UB-check/runtime/precondition graph is
validated, including retained `fmt::Arguments::from_str`, string pointer/length
construction, the constant formatting-shift assertion, and the precise
nonreturning panic call. None of those callees receives independent admission.
All Copy/Move roles are exact; a Move cannot consume the count before a later
shift reads it. Retained bodies have no storage markers, and added markers or
statements are rejected. Each retained body must be optimized runtime MIR.
The failure message must contain the captured bytes in immutable, initialized
memory; authentication uses a bounded allocation read, not a diagnostic-only
read of possibly uninitialized bytes. The retained `str::len` reads
`PtrMetadata(copy _2)`; substituting a Move is rejected. The producer constructs
its two operations directly; it no longer copies statements or temporaries
from profile-specific positions.

## Production Path

- `production_rustc_intrinsic_v1::{ProductionMirV1, production_mir_v1}`
  re-exports immutable compiler-private selection from the sibling
  `trusted_device_items/production_mir_v1.rs`.
- `DeviceCollector::collect` selects before recursive call discovery. The
  original wrapping callable and its caller edge remain collected. Only the
  proved body is expanded to `masked = count & 31; return value >> masked`.
- Source-safety admission recognizes only a successful complete safe-entry
  proof. It grants nothing to unchecked helpers or panic functions.
- `rustc_semantic_plan_v1` retains the selected bodies through both preflight
  passes and node construction. Node identities bind the original MIR hash,
  validated helper-closure fingerprint, versioned rule, and expanded MIR.
  Original function, definition, generic, source-MIR and ABI identities remain
  in the existing transcript. Unexpanded functions keep their previous IDs.
- `collector/production_importer_v1` consumes `plan.function_mir(function_id)`;
  it does not re-read the unexpanded rustc body. The ordinary semantic body
  constructor admits `BitAnd` and `ShiftRight` and preserves the original ABI.

Failed proofs preserve the original borrowed MIR and recursive rejection.
There is no new panic-name exemption, unsafe-helper terminal, workload selector,
semantic opcode, or target-specific lowering shortcut.

## Verification

`standalone.rs` documents the small standalone rustc command. Its tests
exercise pinned host-sysroot MIR and mutation negatives,
production selection, unchanged fallback bodies, and expansion identity binding.
Header mutations include actual pinned coroutine metadata, spread arguments,
substituted source owners, and promoted bodies. Precondition mutations include
operand/control-flow substitutions and panic arguments, flag, destination,
return target, and unwind action. A nine-body graph budget fails and ten passes.

The report-2 MoeRoute rejection identified the retained-call profile. Local
captured-layout replay alone does not certify a cached artifact.
`profile_tests.rs` runs the proof and selector on actual supplied
metadata. Each profile has separate tests for default MIR optimization and
`-Zmir-opt-level=0`, both with `-Cpanic=abort`, `-Copt-level=0`,
`-Zinline-mir=no`, and `-Zmir-enable-passes=-JumpThreading`.

Using the existing standalone test executable on the parent's isolated host:

```sh
FE2O3_WRAPPING_PROFILE=host-cached \
FE2O3_WRAPPING_HOST_CORE="$HOST_CORE" \
FE2O3_WRAPPING_HOST_BUILTINS="$HOST_BUILTINS" \
FE2O3_WRAPPING_DUMP_MIR=1 \
"$WRAPPING_TESTS" core_wrapping_shr_selected_metadata_profile --nocapture --test-threads=1

FE2O3_WRAPPING_PROFILE=amdgpu-cached \
FE2O3_WRAPPING_TARGET_CPU=gfx942 \
FE2O3_WRAPPING_AMDGPU_CORE="$AMD_CORE" \
FE2O3_WRAPPING_AMDGPU_BUILTINS="$AMD_BUILTINS" \
FE2O3_WRAPPING_DUMP_MIR=1 \
"$WRAPPING_TESTS" core_wrapping_shr_selected_metadata_profile --nocapture --test-threads=1
```

Paths must name the existing matching core/builtins rmeta or rlib files, not
directories. Run the AMDGPU probe again with gfx950 and its matching metadata.
Default `host-sysroot` mode is a comparison only, not rebuilt-core evidence.
Failed proofs print bounded source headers, locals, scopes, blocks and helper
edges before failing; they do not admit a fallback body. A new rustc invocation
cannot change MIR already encoded in a supplied core artifact.

For a retained profile these same tests also mutate every checked helper's
operations, operands, calls, assertion messages/flags, destinations, return and
unwind edges, headers, scopes and storage markers. The retained graph requires
nine body visits: eight fails and nine passes. Source-span changes that preserve
semantics must still alter the closure fingerprint. The standalone
`core_wrapping_shr_retained_capture_replay_and_mutations` test builds the captured
layout using pinned rustc types/identities; it exercises the matcher, not cached
AMDGPU provenance. Diagnostics now traverse six edges and show the usually
hidden left operand of overflow assertions.

`retained_regression_tests.rs` also runs against both the replay and supplied
cached profiles. It checks phase changes, out-of-range source scopes,
assignment destinations, Copy/Move substitutions, and same-length message
substitutions, including truncated, mutable and uninitialized allocations.
Expansion is exercised on the captured six-local source body:
the source stays immutable, the safe Rust signature and source identity remain,
and only the exact two-operation mask/shift body is produced. Changing any
helper's source span changes the final expansion fingerprint even when the
expanded operations stay the same.

Resumed local verification on 2026-09-10 used only standalone pinned rustc
with `-D warnings`: 22 tests passed, with the unrelated cached-AMDGPU primitive
comparison test ignored. This includes both host-sysroot optimization profiles
and the complete retained replay/mutation checks. Both actual gfx942 cached
metadata tests also passed, with default MIR optimization and
`-Zmir-opt-level=0`, including all nine helper bodies and their mutations.
The completed metadata came from the parent's offline nightly-2026-04-03
exporter-flags build with debuginfo 2 under
`local-amdgpu-metadata/amdgcn-amd-amdhsa/debug/deps`:
`libcore-d4cdd9c8b80fe1e2.rmeta` and
`libcompiler_builtins-dfaaf6f113af6683.rmeta`, matching the captured core
disambiguator. The actual profile exposed and verified the `str::len` Copy
operand correction. This certifies the bounded proof and immutable selection
for that metadata; compiler integration and GPU execution remain parent work.

Compiler tests are wired as a child of the production importer. The positive
kernel tests require gfx942 AMDGPU core/builtins metadata; no host ABI or
ordinary-helper role is substituted. Both are explicitly ignored by ordinary
test runs, with the required metadata variables named in the ignore reason.
Explicit invocation still fails if metadata is absent or the proof rejects;
there is no runtime skip or fallback. Run on the parent's cached environment:

```sh
FE2O3_WRAPPING_AMDGPU_CORE="$AMD_CORE" \
FE2O3_WRAPPING_AMDGPU_BUILTINS="$AMD_BUILTINS" \
RUST_BACKTRACE=full cargo +nightly-2026-04-03 test -p rustc-codegen-fe2o3 \
  --lib core_wrapping_shift_compiler_tests -- --ignored --nocapture --test-threads=1
```

`core_wrapping_shr_collection_to_semantic_producer` exercises the real collector,
preflight, type/ABI producers, and complete semantic request admission, checking
original identity/ABI and exactly two ordinary binary operations.
`core_wrapping_shr_collection_rejects_unproved_panic_and_unsafe_helpers` covers
direct unchecked calls, reachable panic, and an unreviewed width. Updated
AMDGPU compiler tests have not been run locally. They are producer tests, not remote
production build, LLVM, or GPU execution evidence.

All files in this subtree are required in the source snapshot, including
`profile_tests.rs` and `compiler_tests.rs`. Arithmetic and primitive/result
helper modules are outside this ownership scope.
