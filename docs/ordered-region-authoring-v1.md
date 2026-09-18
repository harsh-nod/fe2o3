# Author one fixed-register instruction region

This draft implementation tutorial exercises real Rust source, one closed
instruction region, CPU semantics, and a separate final-machine inspection.
It is a development-branch observation, not a supported source-to-GPU workflow
or a curriculum release. The normal compiler CLI, simulation-bundle CLI and
protected production admission do not yet support this V16 region path.

Tracking remains [#280](https://github.com/harsh-nod/fe2o3/issues/280),
[#281](https://github.com/harsh-nod/fe2o3/issues/281), and
[#282](https://github.com/harsh-nod/fe2o3/issues/282). Their complete milestones
remain open; see the [implementation status](assembly-authoring-implementation-status.md)
and the earlier [typed-instruction slice](assembly-authoring-first-slice.md).

## The source contract

Use the complete [source fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/ordered_region_v31.rs)
and its adjacent manifest/root module. This display shows its positive branch;
the actual fixture also contains the explicitly selected negative variants.

```rust
use fe2o3_device::{DisjointSlice, amdgpu_asm, amdgpu_ordered_region, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_xor_add(mut output: DisjointSlice<u32>, a: u32, b: u32, c: u32) {
    let a = amdgpu_asm!(v_mov_b32(a));
    let value = amdgpu_ordered_region! {
        gfx942_xnack_off_wave64;
        scratch(32);
        out(33);
        in(34) = a;
        in(35) = b;
        in(36) = c;
        xor_add_u32_e32;
    };
    let index = thread::index_1d();
    if let Some(element) = output.get_mut(index) {
        *element = value;
    }
}
```

The macro accepts no assembly string. It denotes exactly these two instructions,
in this internal order, with a wrapping `u32` result:

```text
v_xor_b32_e32 v32, v34, v35
v_add_u32_e32 v33, v32, v36
result = (a ^ b).wrapping_add(c)
```

The five VGPR indices must be distinct literals in `0..64`. Data inputs and
result are exactly `u32`; the authenticated source marker receives five actual
MIR `u8` constants for the physical roles. Data expressions are evaluated once.
The marker has no silent host fallback: calling it on the host panics.

Only one unconditional region in one direct kernel root is admitted here, on
`gfx942:xnack-`, wave64, with required workgroup size `64x1x1`. It must precede
conditional source edges and cannot be re-entered through a loop. The old
`v_mov_b32` marker above demonstrates coexistence, not arbitrary pre-region
calls or memory accesses.

The region has **NoMemory**, reads EXEC, and declares no implicit register
writes. Its output is early-clobber and its scratch register is clobbered.
Internal instruction order is retained, including when the result is unused.
This is not a memory fence or a promise to fix the order of unrelated surrounding
operations. Register bindings are local to this operation; handles do not escape.

## Where it enters compilation

```text
actual Rust callback -> semantic MIR V31 -> exact canonical KIR V16 owner
                                             |-> atomic logical CPU operation
                                             `-> complete LLVM22 input
                                                   -> worker/LLD -> static inspection
```

The source qualifier uses a `cfg(test)` continuation of the real collected
transaction. It retains the original authenticated compiler bindings through
normal semantic import, middle-end and SSA construction. The new move-only V16
owner retains the verified decoded module and exact canonical bytes. The same
owner supplies CPU admission and complete-module LLVM lowering; no replacement
simulator graph or reinterpretation as a V12 owner is used.

LLVM IR is not bypassed. The region becomes one `asm sideeffect` call with
constraints `=&{v33},{v34},{v35},{v36},~{v32}`. Ordinary surrounding Rust operations
remain ordinary LLVM operations. Compiler-owned boundary copies are allowed and
reported separately from the authored pair. The V16 renderer selects the existing
reviewed LLVM22 worker data layout at initial header generation; it does not edit
a captured LLVM file or relax the worker's target checks. Historical LLVM layout
contracts remain unchanged.

CPU simulation treats the pair as one logical operation. It checks wrapping
values and surrounding simulated memory, not per-instruction physical VGPRs,
EXEC contents, cycles, register pressure or hardware behavior. The separate
native observer assembles/links and inspects returned machine bytes through an
explicitly unauthenticated test transport. Hash joins establish consistency of
retained observations, not source authentication or protected artifact authority.

The additive [logical debugger exercise](ordered-region-debugger-v1.md) uses the
same live source owner and immutable requests. It distinguishes the authored
register plan from actual logical before/after values, with explicit truncated
and unavailable controls. It does not expose physical scratch or EXEC contents.

## Reproduce the six actual source callbacks

Run from the compiler implementation checkout on Linux with the already installed
`nightly-2026-04-03` toolchain and the components in `rust-toolchain.toml`, including
`rust-src` and `rustc-dev`. The commands below use an already populated dependency
cache (`--offline`); use the repository's normal setup before this exercise.
Serialize this run with other Cargo work. Reserve at least 40 GiB free plus room
for the host build; the fresh AMD dependency preparation itself is capped at
500 MiB. The harness is not a whole-build disk or RSS limiter.

Choose an existing, writable qualification directory outside the checkout on
that filesystem. Replace the example absolute path before running:

```sh
ordered_run=$(mktemp -d -p /absolute/qualification-storage fe2o3-ordered.XXXXXXXX)
ordered_rustc=$(rustup which --toolchain nightly-2026-04-03 rustc)
RUSTC="$ordered_rustc" CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 RUST_TEST_THREADS=1 \
FE2O3_TEST_ORDERED_REGION_OUTPUT_V31="$ordered_run/source" \
cargo +nightly-2026-04-03 test -p rustc-codegen-fe2o3 --lib \
  --locked --offline actual_ordered_region_source_ladder -- \
  --ignored --exact \
  production_rustc_driver_v1::gfx942_ordered_region_qualification_v31_tests::actual_ordered_region_source_ladder \
  --nocapture
```

`source` must not exist. The parent prepares real target dependencies, then runs
six isolated rustc callbacks. Each child must execute exactly one test, not merely
return a zero exit status. Source files remain unchanged. Changing a fixture
requires rebuilding the harness; stale compiled-source bytes are rejected.

| Feature | Required observation |
| --- | --- |
| `ordered-region-v31` | One region, `(a ^ b).wrapping_add(c)` stored correctly |
| `ordered-region-unused-v31` | Region retained, despite storing `a` instead |
| `ordered-region-alias-v31` | Scratch/output alias rejected |
| `ordered-region-dynamic-v31` | Runtime `a as u8` physical binding rejected |
| `ordered-region-divergent-v31` | Region after a conditional source edge rejected |
| `ordered-region-wrong-launch-v31` | Required `32x1x1` workgroup rejected |

Both positives run six independent arithmetic cases, including overflow, across
64 logical invocations with unchanged leading/trailing canaries. The four negative
fixtures are valid Rust; they must fail at their specific source-profile boundary,
not because a process failed or some later operation was unsupported.

Read `source/observation.json`, each feature's invocation JSON and stdout/stderr,
and the positive directories' `canonical-v16.bin` and `observation.ll`. Preserve
the real Cargo artifact/metadata records and hashes. The four occurrence references
identify compiler-observed origins; they are not raw-source digests or proof receipts.

## Inspect the same LLVM bytes after native code generation

Use Node.js and the reviewed LLVM/LLD development package, CMake, a C++ compiler,
and zstd headers/library. The checked-in build script requires LLVM `22.0.0git`
and package claim
`rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540`.
Supply its existing package-identity file; do not invent a matching claim for
different binaries. The package claim is not a runtime-closure attestation.
Replace all input paths below with the reviewed local files:

```sh
node scripts/assembly-region-worker-prototype.mjs \
  --llvm-root /absolute/llvm-prefix \
  --llvm-build-id-file /absolute/reviewed-llvm-build-id.txt \
  --zstd-include-dir /absolute/zstd-include \
  --zstd-library /absolute/libzstd.so \
  --output "$ordered_run/native-build"
node scripts/assembly-region-source-machine-observation.mjs \
  --source-run "$ordered_run/source" \
  --native-build-run "$ordered_run/native-build" \
  --output "$ordered_run/machine"
```

Both output directories must be new. If needed, give the first script explicit
absolute `--cmake` and `--cxx` paths. The build preserves the original four native
fixture tests. The second script validates that build's measured inputs, joins
all six source callbacks to their retained records, and submits the two positive
LLVM files unchanged for O0 and O3 observation. It checks exact physical operands,
one unambiguous contiguous e32 pair, implicit EXEC reads, no implicit writes, and
sufficient descriptor VGPR capacity. Neither script launches a GPU kernel.

Read `machine/receipt.json` and the per-feature native reports. The qualified
development run found little-endian instruction bytes `2247402a` then `20494268`
in all four cases. The unused result did not eliminate the pair. Descriptor
encoded VGPR capacity was 56 at O0 and 40 at O3; the authored binding high-water
was 37, and the reported architected VGPR boundary was 40. These are different
quantities. Encoded capacity is not measured register usage, a lifetime/allocation
proof, occupancy, or a performance result. Whole-kernel instruction counts and
bytes may differ across optimization levels.

## Evidence scope and further work

The retained development observations are source run `ordered-region-source-v31-r4`
and machine join `ordered-region-source-machine-join-r1`. Their JSON SHA-256s are
respectively `b47c20d0ed8cfb819e8070cbf279b1a14852bb9dde91261d41746785138e9d44`
and `e54dee8f50eaf881398eb385f35558abacc39fa7d922b991755c28ae8dfff7d2`.
The final backend library suite passed 754 tests with seven explicit ignores;
the source ladder was separately selected and actually executed. These are
working-tree observations, not a retroactive commit, clean-checkout, compiler
closure or release attestation. Reproductions must keep their own records.

The earlier r3 source run emitted the historical layout and was correctly refused
by native input validation. Its LLVM was not repaired in place: r4 freshly emitted
the reviewed worker header. A failed native stage is not a passing machine case.

Unsupported profiles include SGPRs, register tuples/inouts/aliases, multiple regions,
helpers, loops around the region, divergent placement, partial waves, alternate
instruction sequences or encodings, authored memory/synchronization, matrix
instructions, gfx950, and whole-kernel assembly. General allocation/liveness,
physical debugger views, source-to-assembly promotion, reversible Rust regeneration,
persistent schedule recipes, functional proofs and protected final admission
remain separate work. Existing V6/V11 authoring/debugger commands do not accept
this V16 profile. None of the original milestone scope is replaced by this lesson.

The companion [kernel-author tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/codex/multilevel-resource-views-20260917/docs/ordered-region-authoring-v1.md)
is likewise a draft, with no lesson route or publication-pin change.
