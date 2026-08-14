# Tiled GEMM V1

This directory implements the host-only scaffold for one conservative gfx942
GEMM:

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

`src/kernel_face.rs` binds this scaffold to the existing `DeviceMatrix` and
fragment types, but this public increment does not admit their Rust calls for
GPU lowering. Exact target retention, rustc physical-ABI validation, and
binding the executable register and LDS maps above to generated code remain
pending. LDS data movement, full GEMM loops, output stores, production export
and HSACO generation, protected runtime admission, hardware dispatch,
compiler-to-machine refinement, machine memory-safety proof, and machine
race-freedom proof also remain pending.

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

Five negative fixtures mutate A, B, C, row-major XOR4, and the inner two-bit
permutation. The last mutation is bounded, involutive, and non-row-major but
wrong; it must fail its exact AMD-XOR correspondence theorem. Every mutation
must fail only its intended pinned correspondence theorem.

The fail-closed runner pins both Verus version `0.2026.08.02.b677dd5` and the
exact executable-byte SHA-256
`ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd`:

```text
examples/tiled_gemm_v1/run-verus.sh
```

The positive proof source is pinned by an ordinary Rust test at 34,733 bytes
and SHA-256
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
