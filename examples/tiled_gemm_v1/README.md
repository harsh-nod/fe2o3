# Tiled GEMM V1

This directory implements the checked host contract and production-directed
bounded slices for one conservative gfx942 GEMM:

- row-major `A[M,K]` and `B[K,N]` stored as BF16;
- row-major `C[M,N]` stored as FP32;
- `16x16x16` reduction steps;
- one wave64 workgroup per `16x16` output tile;
- fail-closed admission of the repository's typed, canonical
  `gfx942:xnack-` target declaration;
- no semantic input transposition, bias, scaling, batching, split-K, or tails.

`ShapeV1` and `LaunchGeometryV1` have private fields; checked constructors and
read-only accessors are the only safe interface. For a nonempty output, `M` and
`N` must be multiples of 16. Positive `K` must also be a multiple of 16. An
empty output is a no-dispatch operation requiring no `A`, `B`, or `C` storage,
including when unused dimensions are `u32::MAX`. Nonempty `K=0` is a
no-dispatch host fill with FP32 positive zero. Other tails and unrepresentable
launch geometry are rejected before geometry is produced.

For a dispatched shape, the launch API keeps three related geometries
distinct. `block_counts()` is `[N / 16, M / 16, 1]`, identifying one block per
output tile. `workgroup_dimensions()` is `[64, 1, 1]`, identifying the one
wave64 assigned to each block. `aql_grid_work_items()` is their component-wise
product `[64 * (N / 16), M / 16, 1]`, the global work-item dimensions encoded
in an HSA AQL dispatch packet. These checked host values do not establish that
a kernel was compiled, loaded, or executed.

Planning requires an `AdmittedTargetV1` obtained from the canonical
`fe2o3_amd_target::AmdTargetId`. Generic `gfx942`, XNACK-enabled,
SRAM-ECC-qualified, and other processor declarations fail closed. This token
binds a declaration only: it does not attest installed hardware, executable
metadata, or executable bytes.

The bitwise evidence path first validates exact operand lengths and every BF16
encoding. It admits only `BF16_INPUT_PATTERN_V1`, the finite pinned generator
alphabet; NaNs, infinities, subnormals, negative zero, and every other encoding
fail closed with operand, index, and bit-pattern diagnostics. Validated values
widen exactly to FP32, products and sums are separate FP32 operations, and
accumulation visits increasing `k` from positive zero. Tests pin deterministic
generator bytes and independently calculated output bits.

`tiled_gemm_arithmetic_oracle_v1` remains available for general BF16 arithmetic
experiments, including out-of-corpus rounding, recurrence-order, cancellation,
and signed-zero cases. Its results are not finite-corpus bitwise evidence.
Neither oracle claims undocumented MFMA evaluation order or GPU equivalence.

## Register and LDS layouts

The public host scaffold defines three distinct executable register maps for
gfx942 `V_MFMA_F32_16X16X16_BF16`. For `lane in 0..64` and
`component in 0..4`, they are:

| MFMA operand | Logical matrix coordinate |
| --- | --- |
| A/Src0 | `row = lane % 16`, `depth = 4 * (lane / 16) + component` |
| B/Src1 | `depth = 4 * (lane / 16) + component`, `column = lane % 16` |
| C/Src2 and D/Vdst | `row = 4 * (lane / 16) + component`, `column = lane % 16` |

These maps are pinned to the AMD Matrix Instruction Calculator at commit
[`2ef91896bcdc4d26624f952e5c905c787cd9bc9e`](https://github.com/ROCm/amd_matrix_instruction_calculator/tree/2ef91896bcdc4d26624f952e5c905c787cd9bc9e),
using architecture `cdna3` and instruction
`v_mfma_f32_16x16x16_bf16`. Golden tests reconstruct the calculator's exact
CSV output over all 64 lanes and four components. The independently obtained
SHA-256 pins are:

| Calculator table | Exact CSV SHA-256 |
| --- | --- |
| A | `0b81297df0a554684c8631e9266d9282d911bbf74518fba8e990ac9a3c41355d` |
| B | `b39f7eed0eab2c7b207d79bd63bb57d005638cf2a9f87f250e2bc6dc611be377` |
| C | `87b308afdee4ab2182c640a3a7ed0fb84c5555c7311ec3630d21e80969c944be` |
| D | `dd015ae356fd034cb6f48902bf24d097426ecc3a7d8ac6942b12552bf597d836` |

The logical register maps are not LDS layouts. A separate `RowMajorXor4`
storage map sends a bounded logical `(row, column)` to physical element index

```text
16 * row + (column XOR (4 * (row mod 4))).
```

A staging uses logical `(row, depth)`. B staging is deliberately transposed
and uses logical `(row = column, column = depth)` before XOR4; the semantic B
coordinate remains `B[depth][column]`. Exhaustive tests compare both staging
maps to `fe2o3_device::RowMajorXor4` and check by enumeration that each maps
the 256 logical elements bijectively onto physical indices `0..256`.

`src/kernel_face.rs` binds the host contract to the existing `DeviceMatrix` and
fragment types. `src/kernel.rs` contains the fixed Slice 1 algorithm directly
inside an ordinary `#[kernel(typed, ...)]` function; it is not hidden in a
`macro_rules!` expansion or maintained as a second explanatory body. It uses a
compiler-only typed intrinsic for two distinct LDS tiles. The rustc collector
authenticates the exact attributed root, reviewed lane/LDS/barrier/MFMA/store
MIR sequence and FnAbi, WG64 geometry, and compiler-derived 1,024-byte LDS
resource binding. It consumes a single-use correspondence receipt to select
the canonical Slice 1 Kernel IR. Hostile removed-barrier, shifted-index, and
same-spelling-helper fixtures cannot select that IR.

That source path stops immediately after canonical IR selection. It does not
construct a descriptor, publish to Worker V2, lower through LLVM, create or
load HSACO, or launch. The correspondence is a reviewed exact profile, not a
compiler-refinement proof or protected authority. The separately lowered and
observed Slice 1 artifact below is therefore not yet joined to this source
receipt.

`verus/lds_tiled_slice1_source_refinement.rs` is a bounded, identity-bound
source/model correspondence for this exact profile. Its 96 verified obligations
cover exact source lengths, global bounds, same-epoch LDS initialization,
publish ordering, unique output ownership, and pinned portable-MIR,
correspondence, and canonical-module identities. Four negative fixtures reject
a short input, read at the publish event, colliding output ownership, and
portable-MIR identity drift. It does not establish rustc, LLVM, linking, or
machine refinement and grants no descriptor, load, or launch authority.

`verus/tiled_gemm_host_contract.rs` is an independent mathematical proof of
the public contract on the repository's 64-bit host profile. Its 23 public
proof functions discharge 73 verification obligations covering the exact
A/B/C register formulas; XOR4 physical bounds, inversion, permutation, and
non-aliasing; separate A and transposed-B staging; checked tile origins;
complete global C ownership injectivity for every unequal
`(group_x, group_y, lane, component)` tuple; in-bounds row-major A/B loads for
each `K=16` phase; partitioning of `0..K`; empty-output and `K=0` no-read
decisions; and the stated u32/u64 address bounds.

An ordinary Rust correspondence test parses the exact finite Verus formula
bodies and exhaustively compares them with the executable Rust A/B/C and XOR4
maps. In particular, it parses the nested `if` expression that defines
`xor2_v1`, checks all 16 inputs against the pinned two-bit table, and evaluates
the parsed outer `xor4_lds_col_v1` through that parsed inner expression for all
256 logical LDS coordinates. It does not substitute Rust's XOR operator. This
is source-level correspondence only, not compiler refinement.

Nine negative fixtures cover the five host-map mutations plus wrong Slice 1
LDS epochs/products and wrong Slice 2 reuse/accumulator behavior. Each mutation
must fail only its intended pinned proof obligation.

The fail-closed runner pins both Verus version `0.2026.08.02.b677dd5` and the
exact executable-byte SHA-256
`ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd`:

```text
examples/tiled_gemm_v1/run-verus.sh
```

The original host-contract proof source is pinned by an ordinary Rust test at
34,733 bytes and SHA-256
`fcb0bb8d86430fce8dafcd8a049864111952e49b13e0a68997aa424729db336c`.
This pin detects source changes; it is not authenticated execution and grants
no publication, loading, or launch authority. The proof does not establish
MFMA numerical equivalence, compiler refinement, machine-level memory safety
or race freedom, emitted code identity, HSACO behavior, hardware behavior, or
protected authority.
The dedicated `Tiled GEMM V1 host scaffold` workflow exercises this standalone
manifest independently of the root workspace. Its proof job downloads one
exact Verus release archive, verifies the pinned archive and extracted
executable digests, and fails closed before proof execution if either artifact
is unavailable or differs.

## LDS execution slices

Slice 1 has a canonical bounded Kernel IR for one `16x16x16` tile. Dedicated
lowering produces AMDGPU LLVM IR and final COV6 HSACO using upstream LLVM 22
`llc` and `ld.lld`, without COMGR. Final inspection requires the exact
`gfx942:xnack-` target, WG64/wave64, 1,024 static LDS bytes, zero unexpected
private segment or spills, LDS reads and writes, one converged barrier, one
BF16 MFMA, and no calls or atomics.

The ignored opt-in runtime test at
`crates/fe2o3-hsa-runtime/tests/tiled_gemm_lds_v1_hardware.rs` regenerates that
observational artifact with SHA-pinned upstream LLVM tools. On MI300X it passed
zero, identity, dyadic, deterministic-random, signed-cancellation, and
adversarial finite-BF16 cases: 1,536 output values plus immutable A/B checks
and prefix/suffix canaries around all three allocations. This is IR-derived
hardware evidence, not source correspondence or protected launch authority.

Slice 2 currently has exact K32 Kernel IR with two K16 phases, carried FP32
accumulators, and barriers before reads and before LDS reuse. Its Verus model
covers one through four bounded phases with 196 verified obligations; the
executable event model exhausts phase counts 1, 2, and 4. Dedicated lowering
produces a real SSA loop and an upstream-LLVM final-artifact test pins two
machine barriers, one static loop-body MFMA, 1,024 LDS bytes, and zero spills,
calls, atomics, or COMGR markers. Ordinary multi-phase source collection and
K32 hardware execution remain pending.

Slice 3 has a fixed-K16 grid/stride proof and executable model. It proves
checked grid derivation, workgroup-to-tile injectivity, padded
`lda`/`ldb`/`ldc` bounds, four stores per lane, and disjoint global C ownership
for positive fully tiled M/N shapes. The proof reports 101 verified obligations
and three expected-rejection mutations. The exact
`M=64,N=48,K=16`, `lda=33,ldb=79,ldc=96` representative also has canonical
Kernel IR and dedicated upstream-LLVM lowering. Final inspection pins its
`3x4` grid, workgroup-X/Y reads, WG64/wave64, 1,024 LDS bytes, one barrier, one
BF16 MFMA, zero spills/scratch/calls/atomics, and no COMGR. Grid-aware
attributed source and protected MI300X numerical execution remain pending.

Slice 4 has a bounded 101-obligation Verus model for M/N/K tails, unconditional
barrier participation, exact-real alpha/beta semantics, and output ownership,
with four expected-rejection mutations. Its exact
`M=17,N=19,K=18`, `alpha=2,beta=-1` canonical Kernel IR represents a `2x2`
grid, two predicated K16 phases, BF16 zero-fill tails, reusable XOR4 LDS,
carried FP32 accumulators, unconditional publish/reuse barriers, and predicated
C reads/writes. Dedicated upstream-LLVM lowering and final COV6 inspection pin
workgroup-X/Y, WG64/wave64, 1,024 LDS bytes, predicated global memory, two
barriers, one static loop-body BF16 MFMA, zero spills/private segment, and no
COMGR, calls, scratch, or atomics. Attributed general source and protected
MI300X numerical execution remain pending.

Implementation claims and dependencies are coordinated in fe2o3 issues
[#85](https://github.com/harsh-nod/fe2o3/issues/85) through
[#90](https://github.com/harsh-nod/fe2o3/issues/90). The public tutorial and
evidence update is tracked in
[fe2o3-kernels #1](https://github.com/harsh-nod/fe2o3-kernels/issues/1).

## Observed direct-global tile

The repository also contains an ignored, opt-in HSA harness for an externally
supplied `gfx942:xnack-` COV6 artifact. On 2026-08-14 it accepted and executed
one 6,672-byte artifact with SHA-256
`681077be1108c57d9d887f94afdd0ec3700ed2c86d73e66d2b229d6b418d0c66` on
MI300X. The exact test passed its bitwise 16x16 oracle, A/B/C unchanged-value,
adjacent-canary, synchronous-completion, executable-identity, and terminal
unload checks in 40.92 seconds. The complete command and boundary are recorded
in [`docs/tiled-gemm-v1-mi300x-observation.md`](../../docs/tiled-gemm-v1-mi300x-observation.md).

This is non-authoritative hardware observation for the direct-global one-tile
profile. The artifact was supplied separately, uses no LDS, and is not joined
to the Rust source by authenticated compiler evidence. It establishes neither
a production tiled GEMM nor general shapes, multiple K phases, tails, protected
launch authority, compiler refinement, memory safety, or race freedom.

Run the host checks independently of the root workspace:

```text
cargo fmt --manifest-path examples/tiled_gemm_v1/Cargo.toml \
  --package fe2o3-tiled-gemm-v1 -- --check
cargo test --locked --manifest-path examples/tiled_gemm_v1/Cargo.toml
cargo test --release --locked --manifest-path examples/tiled_gemm_v1/Cargo.toml
cargo clippy --locked --manifest-path examples/tiled_gemm_v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
VERUS=/absolute/path/to/pinned/verus examples/tiled_gemm_v1/run-verus.sh
```
