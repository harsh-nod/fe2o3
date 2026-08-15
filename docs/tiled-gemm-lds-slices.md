# gfx942 LDS-tiled GEMM delivery slices

This document defines the promotion boundary for the first production-directed
LDS-tiled GEMM path. It is a delivery contract; every slice remains Partial
until all of its required evidence is joined by identity.

## Source model

The canonical user program is an ordinary Rust function annotated with
`#[kernel(typed, ...)]`. The collector must authenticate the MIR of that
generated kernel root and bind it to the selected Kernel IR, compiler module,
final HSACO, and generated typed launch API.

An explanatory source file, a separately maintained `macro_rules!` body, a
canonical IR builder, or a digest-pinned externally supplied HSACO is not a
substitute for that chain. Small macros may adapt target-specific primitives,
but a production kernel algorithm must remain visible in the annotated
function body and its collected MIR.

All initial slices are restricted to `gfx942:xnack-`, code object version 6,
wave64, and upstream LLVM/LLD. COMGR is outside this path.

## Current implementation boundary

The repository now has one exact, identity-joined Slice 1 vertical slice plus
independent evidence for later slices:

- the Slice 1 algorithm is ordinary Rust inside `#[kernel(typed, ...)]`. Its
  compiler-only typed LDS acquisition, lane identity, stage operations,
  barrier, MFMA, and output stores are visible in the collected MIR;
- the collector authenticates that exact attributed root, reviewed portable
  MIR and FnAbi identities, WG64 geometry, and compiler-derived pair of aligned
  512-byte LDS tiles before a single-use receipt selects the canonical Slice 1
  Kernel IR. Hostile barrier, index, and same-spelling-helper mutations fail
  before IR selection;
- an identity-bound Verus source model checks 96 obligations for exact lengths,
  same-epoch LDS initialization, publish-barrier ordering, unique output
  ownership, and the portable-MIR/correspondence/canonical-module identities.
  Four hostile source-model mutations fail at their intended obligations;
- canonical Slice 1 Kernel IR has hostile verifier tests, a 93-obligation Verus
  model, dedicated upstream-LLVM lowering, final HSACO inspection, and an older
  observational six-case MI300X run with allocation canaries;
- a sealed Slice 1 profile continues that source/IR groundwork through direct
  upstream LLVM target-machine emission and the in-process LLD library API in
  Worker V2, exact COV6 finalization, a generated borrowed A/B/C host adapter,
  and the private, non-`Clone` one-shot
  `Joined -> Loaded -> Completed -> Unloaded` lifecycle, with no COMGR or
  command-line linker;
- the exact protected path binds artifact target `gfx942:xnack-` to a compatible
  observed target and rechecks grid `[1,1,1]`, workgroup `[64,1,1]`, 1,024
  static LDS bytes, zero private/dynamic bytes, and 48 explicit plus 256 hidden
  COV6 kernarg bytes before its single synchronous dispatch;
- an MI300X `gfx942` run of that path matched all 256 output bits against the CPU
  reference, left A/B immutable, preserved prefix/suffix guard canaries, and
  validated terminal unload;
- exact K32 Kernel IR carries accumulators across two K16 phases and lowers to
  inspected gfx942 machine code with two static barriers and one loop-body
  MFMA;
- the Slice 2 proof model covers one through four phases with 196 verified
  obligations and expected-rejection mutations for missing reuse and reset
  accumulators; and
- the fixed-K16 Slice 3 proof model adds 101 verified obligations for exact
  grid derivation, padded strides, global A/B bounds, and disjoint C ownership
  across workgroups, with three expected-rejection mutations. Its exact
  `M=64,N=48,K=16`, `lda=33,ldb=79,ldc=96` Kernel IR lowers to inspected
  gfx942 machine code with a `3x4` workgroup grid, workgroup-X/Y reads, 1,024
  LDS bytes, one barrier, and one BF16 MFMA; and
- Slice 4 has a 101-obligation edge/tail/alpha-beta proof plus an exact
  `M=17,N=19,K=18` Kernel IR. The IR represents a `2x2` grid, two predicated
  K16 phases, BF16 zero-fill tails, carried FP32 accumulators, unconditional
  publish/reuse barriers, predicated C reads/writes, and `alpha=2,beta=-1`.
  Dedicated upstream-LLVM lowering and COV6 inspection pin workgroup-X/Y,
  1,024 LDS bytes, predicated memory, two barriers, one loop-body BF16 MFMA,
  zero private segment/spills, and no COMGR, calls, scratch, or atomics.

Slice 1 is therefore functional and measured for this exact bounded profile.
The executed HSACO is identity-joined through the closed profile and canonical
re-lowering, but the chain does not authenticate compiler origin or prove that
the machine code refines the Rust source, MIR, Kernel IR, or Verus model. It is
not production proof authority and does not establish general illegal-memory
or race freedom. General shapes and protected Slice 3/4 execution remain open,
so all four slices and every affected parity row remain Partial.

The source/IR groundwork is recorded in
[#85](https://github.com/harsh-nod/fe2o3/issues/85),
[#86](https://github.com/harsh-nod/fe2o3/issues/86), and
[#93](https://github.com/harsh-nod/fe2o3/issues/93). Shared integration
[#94](https://github.com/harsh-nod/fe2o3/issues/94) and children
[#96](https://github.com/harsh-nod/fe2o3/issues/96),
[#97](https://github.com/harsh-nod/fe2o3/issues/97),
[#99](https://github.com/harsh-nod/fe2o3/issues/99), and
[#100](https://github.com/harsh-nod/fe2o3/issues/100) are closed. Production
Verus certificate consumption [#91](https://github.com/harsh-nod/fe2o3/issues/91)
and refinement [#106](https://github.com/harsh-nod/fe2o3/issues/106) and
[#107](https://github.com/harsh-nod/fe2o3/issues/107) remain open with the other
Slice 2-4 work.

## Slice 1: one LDS tile

The fixed operation is one `16x16x16` BF16-by-BF16 product with FP32 output,
launched as one 64-thread workgroup. It has no M, N, or K tails and no alpha or
beta operands.

The implemented exact path uses upstream LLVM 22.1.8 build
`upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1` and measured
worker
`fe2o3-worker-v1-sha256-6c3dfd5f784b3babe140006aba57a214a897b171860928440184fa201b6f96db`.
Its protected MI300X marker was:

```text
FE2O3_PROTECTED_SLICE1_WORKER_V2_OK outputs=256 max_abs_error=0 finalizer=078e9b523164b679ff7af3b4e819ad041713c53c6841399ac7cea95090f09774 unload=df2f77ee798444a9e1fe5e27f219bdf720386eb8603a9a74fccc0df8efb3921c
```

Promotion requires all of the following:

- one ordinary `#[kernel(typed, ...)]` Rust body with compiler-authenticated MIR;
- two distinct, aligned, static 256-element BF16 LDS allocations;
- one-to-one cooperative global-to-LDS writes for A and B using the admitted
  XOR4 physical layout;
- one converged workgroup barrier transferring initialization and visibility;
- bounded LDS fragment reads, exactly one admitted
  `v_mfma_f32_16x16x16_bf16`, and four uniquely owned output stores per lane;
- final source-derived COV6 HSACO linked by upstream LLVM/LLD and loaded from
  the exact bytes authenticated by the compiler/publisher chain;
- metadata and descriptor agreement for target, ABI, WG64/wave64, nonzero
  static LDS, zero unexpected private segment, and the kernel symbol;
- ISA admission for expected global/LDS traffic, barrier, and MFMA, while
  rejecting scratch, atomics, calls, and unreviewed control flow;
- Verus proofs for bounds, XOR4 permutation, cooperative write disjointness,
  initialized LDS reads, barrier convergence, output disjointness, and the
  fixed-tile arithmetic correspondence; and
- MI300X tests covering every output element, input immutability, surrounding
  canaries, and zero, identity, dyadic, randomized, and adversarial BF16 data.

The functional lifecycle items above are identity-joined. The source-derived
proof-authority and general-safety items are not, so Slice 1 remains Partial.

## Slice 2: multiple K phases

Extend the fixed M=N=16 kernel to K values that are positive multiples of 16.
The phase loop must carry an explicit accumulator and an LDS epoch invariant.
Every phase performs cooperative writes, a converged barrier, initialized
reads, MFMA, and a converged reuse barrier before the next overwrite.

Tests must include K=16, 32, 64, and larger randomized cases. Proofs must cover
loop termination, phase bounds, accumulator correspondence, initialization by
epoch, and safe LDS reuse.

## Slice 3: multiple output tiles

Add an M/N grid for dimensions that are multiples of 16 and explicit row/column
strides. The typed launcher derives checked grid geometry. Verification must
establish workgroup-to-tile injectivity and disjoint output ownership across
the entire grid.

The bounded representative now has exact proof, Kernel IR, upstream-LLVM
lowering, and final machine-shape inspection. Protected Worker V2 publication,
MI300X numerical/canary execution, and attributed grid-aware source remain
open in [#88](https://github.com/harsh-nod/fe2o3/issues/88) and
[#90](https://github.com/harsh-nod/fe2o3/issues/90).

## Slice 4: complete edge semantics

Add M, N, and K tails plus `alpha` and `beta`. Loads and stores are predicated
without allowing any lane to skip a required workgroup barrier. Define and
test the BF16-input/FP32-accumulation numerical contract separately from exact
finite-corpus bitwise evidence.

The bounded representative now has exact proof, Kernel IR, upstream-LLVM
lowering, and final machine-shape inspection. Protected MI300X execution is
tracked in
[#89](https://github.com/harsh-nod/fe2o3/issues/89), and general attributed
source in [#90](https://github.com/harsh-nod/fe2o3/issues/90).

## Slice 5: performance qualification

After correctness and authority are established, add vectorized global loads,
double-buffered LDS, and prefetching as separately admitted profiles. Promotion
requires resource and ISA checks for bank conflicts, VGPR/SGPR pressure,
occupancy, spills, and dynamic stack use, plus reproducible MI300X benchmarks
against an explicitly pinned rocBLAS baseline.

An optimization is never allowed to inherit the safety, functional, or
artifact evidence of a structurally different profile.
