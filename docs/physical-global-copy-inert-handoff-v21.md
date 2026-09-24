# Normal inert handoff for authored global-copy kernels (KIR21)

The normal source extraction driver can continue the bounded MIR38/KIR21
global-copy profile through mandatory ranked/formal checks, exact compiler ABI,
canonical LLVM, a typed descriptor and an inert compiler-module handoff.

This is not host admission, native execution, protected publication or GPU
qualification. The source diagnostic endpoint in
[physical-global-copy-source-v21.md](physical-global-copy-source-v21.md) stays
pre-ranked and diagnostic-only. It is not silently upgraded. The checked owner
and conditional memory model are described in
[physical-global-copy-checked-v21.md](physical-global-copy-checked-v21.md).

## Actual source and compiler ABI

Use the existing source fixture
[physical_global_copy_v21.rs](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/physical_global_copy_v21.rs).
The logical signature has readonly input0 and owned disjoint output1, both
u32 slices. The retained actual Rust source requires Pair ABI for each, with
SharedBorrow and ExclusiveOwner respectively. Four native eight-byte slots
are input pointer at 0, input length at 8, output pointer at 16 and output length at 24.
The explicit compiler kernarg prefix is 32 bytes aligned to 8; it is not a third
logical argument.

The checked owner keeps the actual source, launch, executable, mandatory ranked
reports and combined formal report. A private fixed-size ABI preparation
retains all four authored reads, exact source sites and SSA results, LGKM
readiness, the full-EXEC input load/VM wait, original loaded value, output
compare/mask/store/wait/restore and the following unresolved conditions:

- Input must provide a readable, initialized 128-u32 prefix (512 bytes) for the
  source launch envelope 128. The input read is unguarded and happens even if
  output length is zero. Loaded data stays opaque.
- Output must provide 512 writable bytes under the conservative formal
  LaunchEnvelope. A ragged CPU test does not reduce this production condition.
- The actual input/output bindings must satisfy their shared/owned source
  borrowing contracts and required non-overlap. Distinct logical arguments,
  readonly annotations or clean conditional analysis do not establish this.
- The compiler kernarg prefix must stay live, readable and immutable throughout
  execution, and disjoint from output writes. No input/kernarg disjointness is
  invented: both are readonly.

The normal descriptor constructor replays the same checked owner, joins exact
source/ABI rows and consumes both the real input/output report and the separate
compiler-kernarg conditions. Its existing Rust-ownership geometry check does
not discard or discharge the retained runtime alias requirements. Only the
exact capability on that same immutable module is permitted. Generic,
complete-body, unknown and near-matching extension routes remain closed.

The worker preparation is move-only and retains the checked source, ABI
conditions, typed roots and same cumulative ledger. Its only outgoing method
explicitly demotes to inert handoff/descriptor data. There is no
native_output_parts conversion, caller-origin constructor, protected/finalizer
or host-admission conversion. Semantic V3/protected extraction is a named
refusal for this profile.

## Public command

Build the normal backend and wrapper with the repository's pinned nightly and
offline workflow. No private rustc callback is needed for these commands:

    export RUSTC=/absolute/path/to/nightly-2026-04-03/bin/rustc
    export FE2O3_PHYSICAL_GLOBAL_COPY_BIN_DIR_V21=/absolute/path/to/build/debug
    node scripts/physical-global-copy-checked-v21.mjs one /new/absolute/output-one
    node scripts/physical-global-copy-checked-v21.mjs registers /new/absolute/output-registers

Every output directory must be new. The script uses the existing normal LLVM
and compiler-handoff extraction settings, two isolated Cargo target directories
and the unchanged source fixture. It records bounded process logs, source and
backend file hashes, canonical.ll, handoff-v2.bin and an inert diagnostic
relation. It never invokes a native worker, finalizer, runtime or GPU.

The normal handoff path emits a closed diagnostic record containing canonical
identity, unchanged canonical LLVM hash, serialized handoff hash, descriptor
hash and explicit unavailable-authority fields. The script checks that the
LLVM and handoff file hashes match this record. Missing, duplicated, ambiguous,
unknown-field, altered or authority-claiming records refuse. The canonical
identity and descriptor hash are declared observations, not imported source
custody or independent compiler authentication. Complete compiler-closure
attestation is explicitly unavailable.

Seven exact negative source commands are also accepted: wrong-launch,
missing-lgkm, missing-vm, wrong-carry, wrong-store, wrong-offset and
reserved-register. They must fail at their named source/profile boundary
without creating extraction outputs. A generic compile failure, timeout or
signal is not an accepted negative.

The script observes at most 16 MiB process output, a 300-second timeout and
1 GiB/100000 entries in its fresh output tree. These are stop/observation bounds,
not whole-machine disk/RSS reservation or descendant-custody proofs. Existing
compiler allocation domains remain separately bounded.

## Qualification boundary

The isolated actual-source ladder is the compiler integration control:

    FE2O3_TEST_PHYSICAL_GLOBAL_COPY_PRODUCTION_OUTPUT_V21=/new/absolute/ladder \
      cargo test -p rustc-codegen-fe2o3 --lib \
      production_rustc_driver_v1::gfx942_physical_global_copy_production_v21_tests::actual_physical_global_copy_production_ladder \
      -- --exact --ignored --nocapture

Use the pinned RUSTC and serialized repository test environment. The ladder
runs actual source observation, normal LLVM extraction and normal handoff
extraction in separate rustc sessions. Expected, not yet an evidence claim:
27 sessions; 128 CPU positives; 6 CPU input/alias negatives; 21 precise source
negative runs; 88 actual-owner ABI/source-descriptor mutation refusals; 8
denial-only resource controls; 6 exact descriptor-extension mutation refusals.

The CPU oracle is the existing engine, admitted from the same checked owner
and resource ledger. It exercises grids 64/128, varied opaque input words,
ragged/empty output and canaries. Short/uninitialized input must fail even with
empty output. Same-backing views remain conservatively refused even when their
byte ranges do not overlap. None of this discharges authenticated host/runtime
conditions.

The ladder checks the exact existing descriptor serializer extension after
unchanged canonical LLVM and requires public normal driver output bytes to
equal the corresponding current-source observation. Exported bytes and logs
carry no source, runtime, host, protected or hardware authority.

No successful qualification, native/GPU result or full M3/M6 milestone closure
is asserted by this implementation document. Retain actual root-run receipts
separately; broader memory/synchronization and protected admission exits remain
open.
