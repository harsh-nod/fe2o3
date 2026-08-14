# Row-softmax V1 formal contract

This standalone example specifies one **unmasked, nonempty row of exactly 64
conceptual `f32` values** and a separate 64-element output. It is a source-level
formal model and finite host reference, not a GPU kernel or a production
compiler/runtime slice.

## Contract layers

1. **Real mathematical specification.**
   `verus/row_softmax_v1.rs` defines stable softmax over mathematical reals.
   A maximum bounds every input and equals at least one input. Each weight is
   the abstract real exponential of `input[i] - maximum`; each output satisfies
   `output[i] * sum(weights) == weights[i]`. This is the intended exact-real
   relation; V1 does not claim Verus proves the field/division step that derives
   `sum(output) == 1` from it.
2. **Finite algorithm state.**
   The Verus `denominator_state_v1` predicate models the fixed 64-step
   sequential reduction and proves one-step invariant preservation. A separate
   exact integer surrogate proves that positive output numerators equal the
   weights and sum to their common denominator. The Rust
   `FiniteAlgorithmStateV1` records the maximum, 64 weights, and denominator
   produced by the executable host reference. Neither finite model is claimed
   to refine `f32`.
3. **Mask and empty-row policy.**
   V1 has no mask. All 64 positions participate. An empty row is not
   representable, so there is no all-masked fallback or zero-denominator rule.
4. **Explicitly unproved numeric contract.**
   `exp_real_v1` is uninterpreted. Positivity and correspondence between that
   symbol and each supplied real weight are preconditions, not proved facts.
   No theorem connects Verus `real` to Rust/LLVM/AMDGPU `f32`, rounding,
   subnormals, NaNs, infinities, `f32::exp`, an approximation library, reduction
   order, or error bounds.

## What Verus proves

- all active identity-mapped input, scratch, and output indices are in `0..64`;
- distinct lanes own distinct scratch and output elements;
- every active four-byte access lies inside a checked 256-byte row region;
- separate input/output regions cannot alias for any reader/writer pair;
- distinct output writes and distinct scratch writes do not race;
- the maximum shift is nonpositive under the maximum contract;
- the finite denominator recurrence preserves its prefix-sum invariant;
- positive abstract real weights give a positive real denominator; and
- in the exact finite integer surrogate, output numerators are positive and
  sum to their shared denominator.

The race result is phase-conditional: each lane writes its private scratch slot,
a synchronization boundary makes all 64 scratch writes visible, reduction reads
occur after that boundary, and output writes use the proved injective ownership
map. This proof does not model a concrete barrier instruction, wave execution,
memory ordering, or machine scheduling.

Three negative mutations must be rejected: `lane + 1` indexing, a duplicate
lane-63/lane-0 output owner, and reading lane zero's weight for every output.

## Authentication boundary

`run-verus.sh` fails closed unless both the Verus version and executable bytes
match:

```text
Version: 0.2026.08.02.b677dd5
SHA-256: ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd
```

Ordinary Rust tests additionally pin the proof source digest and check that no
`admit`, `assume`, or external-body shortcut appears. These checks authenticate
the named source and local Verus executable only. They do **not** bind the proof
to generated LLVM IR, ISA, HSACO, loading, launch, or observed GPU execution.
The positive proof pin is 9,409 bytes with SHA-256
`ec87f15d04a3ecb79e974b65923e24eb957a9e5d22714463c916cac6b738d7e0`;
all three negative sources are pinned independently as well.

## Commands

```text
cargo fmt --manifest-path examples/row_softmax_v1/Cargo.toml \
  --package fe2o3-row-softmax-v1 -- --check
cargo test --locked --manifest-path examples/row_softmax_v1/Cargo.toml
cargo test --release --locked --manifest-path examples/row_softmax_v1/Cargo.toml
cargo clippy --locked --manifest-path examples/row_softmax_v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
VERUS=/absolute/path/to/pinned/verus examples/row_softmax_v1/run-verus.sh
```

## Remaining proof obligations

- prove the selected real exponential laws instead of accepting
  `exp_weights_contract_v1`;
- supply and verify the real field/division lemmas needed to derive positivity
  and exact normalization of the real output sequence from the V1 relation;
- define and verify an `f32` exponential approximation with stated ULP/error
  bounds and exceptional-value policy;
- refine finite `f32` max, exp, sum, and divide operations to an error-aware
  specification, including the exact parallel reduction order;
- prove synchronization, memory-order, and race freedom for a concrete kernel;
- bind the Rust kernel body through MIR/LLVM/AMDGPU lowering to emitted HSACO;
- authenticate descriptor/finalizer/runtime admission and hardware results; and
- add masking, all-masked rows, variable row widths, batches, and striding.
