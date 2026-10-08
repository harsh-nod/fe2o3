# Closed exact FP4 matrix simulation

This implements a CPU numerical subdomain of the existing canonical
`ScaledMultiplyAccumulate` operation, not a new opcode, source marker, tensor
layout, codec version, hardware profile or execution authority.

## Exact admitted contract

Only the full `fp4_e2m1_f32_m16n16k128_wave64` profile with the exact
`gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64` tensor layout,
uniform subgroup convergence, 64 active lanes and a 64-bit simulation target
is supported. FP8 and mixed FP4/FP8 remain unsupported. The current KIR has no
scale operands: its existing LLVM lowering fixes both FP4 format selectors
to 4 and the four scale controls to zero. This is the existing identity-scale
operation, not arbitrary E8M0 scaling. In particular, an encoded E8M0 byte of
zero is not being described as the identity.

Each lane supplies eight u32 words for A and B. The low four words contain
32 FP4 nibbles, least-significant nibble first; the upper four words must be
zero. For lane L and element c, A is at [L%16,32*(L/16)+c] and B is at
[32*(L/16)+c,L%16]. Result component d is at [4*(L/16)+d,L%16].
The existing zero-filled-tail contract does not permit partial waves.

The accepted FP4 values are +0 and signed 0.5, 1, 1.5, 2, 3, 4, 6.
Negative zero is a valid FP4 representation but deliberately outside this
simulator subdomain. C accepts +0 and exact quarter-grid f32 values with
absolute value at most 2^20; negative zero, smaller fractions, subnormals,
NaNs and infinities refuse rather than round or coerce.

Let A2=2*A, B2=2*B and C4=4*C. The implementation computes the integer
Q=C4+sum128(A2*B2). Every possible partial sum satisfies
|Q| <= 4,194,304+128*144 = 4,212,736 < 2^23. Products and all intermediate
quarter-grid sums are exactly representable in f32. Consequently this
subdomain needs no claim about hardware accumulation order or approximate
rounding. Bit-only output construction returns the exact result; exact
cancellation produces positive zero. The implementation uses no host float.

## Interpreter and resources

The existing full-wave/site rendezvous remains mandatory. All 64 operand
sets are screened and all 256 results computed before any lane binding.
Input-domain, packing and arithmetic failures therefore cannot publish a
partial matrix result. Existing per-lane completion/event-sink error behavior
is unchanged; this is not transactional rollback after a later sink failure.

A domain refusal reports A/B nibble component 0..31, or upper word 32..35;
accumulator components remain 0..3. It retains no operand payload. Ordinary
memory bounds and initializedness still run before the numerical collective.

Each arrival prepays 40 operand-read units. Resolution prepays 66*N for
existing wave scans plus 268,672 fixed units: 64*76 domain checks,
256*128*8 MAC units, 256*4 result units, 64*5 completion units and 320 scratch
initialization units. All charges precede the corresponding work. Fixed
scratch and construction/move coexistence are added only for reachable FP4
operations. Limits, schemas, ordinary value layouts and the BF16 model are
unchanged. The input payload must fit the existing largest collective
variant; private size tests protect this instead of raising the resident cap.

## Tests and historical scope

The tests include independent dense row-major f64 reference arithmetic
whose inputs/partials are all exact; it does not call the production decoder
or coordinate helpers. It covers all 128 reduction positions, signed and
fractional inputs, extreme accumulators, cancellation, 256 individual outputs
and canaries, two waves, seeded/replay, late-lane domain and packing errors,
memory errors, partial/divergent/mismatched waves, layout/target refusals and
exact/one-short work/storage limits. These are inert canonical KIR graphs,
not a newly observed Rust-source export or native execution.

The existing ordinary gfx950 source route already supports typed FP4/FP8
GEMM and attention. See `examples/gfx950_low_precision/README.md` for the
historical gfx950 four-kernel hardware qualification and exact source,
compiler, HSACO, launch and numeric profile. Its external Clang/LLD/HSA
engineering route is not promoted to the common artifact/host transaction
by this CPU evaluator. Existing BF16 same-owner engineering observations and
FP4 representation tests retain their own scope. M4 still requires the
applicable common-pipeline, emitted layout/resource, target-negative and
currentness evidence; this addition alone does not close M4.

Validation observed on 2026-10-08 UTC with the pinned nightly-2026-04-03
toolchain on mi350-2:

```text
cargo test --locked --offline -p fe2o3-kir-sim --lib matrix_fp4_exact
cargo test --locked --offline -p fe2o3-kir-sim --test matrix_fp4_exact_v1
cargo test --locked --offline -p fe2o3-kir-sim
```

The complete package command passed 569 unit/integration tests and 10
doctests, with no failures or ignored tests. This includes all 14 canonical
FP4 tests, six private FP4 controls, six new transfer/frame-accounting
controls and the unchanged 15-test BF16 integration suite. Earlier focused
runs and their failures are retained separately; only the final complete
suite is the passing qualification. The source/tool/input observations
matched before and after the run. Normal receipt SHA-256:
`ab4af023498eca4a384a5ca107b24a9f237249cff2d3338f9b5277eff3fc21d7`.

The test graph reuses dominating INDEX constants and expressions without
removing any of its 20 typed input loads or four stores. Both two-workgroup
and single-workgroup two-wave cases preserve all 128 invocations. Each
invocation yields once at the matrix operation and once at completion, so
the complete seeded record has exactly 256 decisions; a budget of 255 must
fail rather than truncate. The 16 MiB resident and 8192 memory-access-history
limits remain unchanged. This does not assert complete race-history coverage.

General frame accounting now follows the existing exact stack reservation
and the maximum reachable transfer-operand list length, including repeated
operand positions and every call/return/branch/switch alternative. Retained
inactive frames and transfer buffers remain charged; both full-SSA scratch
allowances and checked size arithmetic are preserved. This corrects excess
reservation, not the resource policy. The tested simulator source and its
local dependency closure match the published implementation; the validation
paragraph is a documentation-only update after that run.
