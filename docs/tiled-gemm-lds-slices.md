# gfx942 LDS-tiled GEMM delivery slices

This document defines the promotion boundary for the first production-directed
LDS-tiled GEMM path. It is a delivery contract, not evidence that any slice is
complete.

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

The repository currently has independent evidence for important parts of the
first two slices, but not the artifact-identity join required for promotion:

- the Slice 1 algorithm is ordinary Rust inside `#[kernel(typed, ...)]`; it
  fails closed at the unavailable compiler-issued LDS capability rather than
  substituting host memory or a second algorithm body;
- canonical Slice 1 Kernel IR has hostile verifier tests, a 93-obligation Verus
  model, dedicated upstream-LLVM lowering, final HSACO inspection, and an
  observational six-case MI300X run with allocation canaries;
- exact K32 Kernel IR carries accumulators across two K16 phases and models the
  reuse barrier before the second LDS overwrite; and
- the Slice 2 proof model covers one through four phases with 196 verified
  obligations and expected-rejection mutations for missing reuse and reset
  accumulators.

The collector cannot yet derive the LDS operations from the attributed Rust
body. Therefore the IR-derived HSACO and hardware result are not evidence of
source correspondence, Worker V2 admission, protected publication, compiler
refinement, or machine-level memory/race freedom. Slice 1 and Slice 2 both
remain partial.

## Slice 1: one LDS tile

The fixed operation is one `16x16x16` BF16-by-BF16 product with FP32 output,
launched as one 64-thread workgroup. It has no M, N, or K tails and no alpha or
beta operands.

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

Until every item is joined by artifact identity, Slice 1 remains partial.

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

## Slice 4: complete edge semantics

Add M, N, and K tails plus `alpha` and `beta`. Loads and stores are predicated
without allowing any lane to skip a required workgroup barrier. Define and
test the BF16-input/FP32-accumulation numerical contract separately from exact
finite-corpus bitwise evidence.

## Slice 5: performance qualification

After correctness and authority are established, add vectorized global loads,
double-buffered LDS, and prefetching as separately admitted profiles. Promotion
requires resource and ISA checks for bank conflicts, VGPR/SGPR pressure,
occupancy, spills, and dynamic stack use, plus reproducible MI300X benchmarks
against an explicitly pinned rocBLAS baseline.

An optimization is never allowed to inherit the safety, functional, or
artifact evidence of a structurally different profile.
