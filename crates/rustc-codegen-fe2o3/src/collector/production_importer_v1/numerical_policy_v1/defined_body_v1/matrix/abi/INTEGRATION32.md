# Source Matrix ABI Checkpoint 32

Mounted owned child and tests only. No schema, shared importer root, preflight,
device, lowerer, backend or simulator edits. No Cargo/compiler/test invocation.

The actual mixed31 projection guards parsed only Format and dropped the real
second accumulator axis Brand. Mixed MFMA expected bare Gfx950Matrix and
two-axis fragments, while source uses PolicyGfx950Matrix<M,R,StrictIeee> and
three-axis fragments. The child accepts exactly the three observed terminals.

## Required Parent Delegation

In `collector/production_importer_v1.rs::terminal_operation_v1`, replace the
three existing guarded arms for Gfx950Fp4AccumulatorIntoValues,
Gfx950Fp8AccumulatorIntoValues, and Gfx950Fp4Fp8MultiplyAccumulate with:

```rust
ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues
| ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues
| ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate =>
    numerical_policy_v1::defined_body_v1::matrix::abi::operation(
        tcx, instance, expansion, abi, types,
    ),
```

No other root/roster/version hook is needed. All three existing expansion tags,
source ownership rules and semantic operation payloads remain unchanged.
The owned matrix attachment/carriage functions already replay this new child;
the standalone-projection replay occurs even when no Bind is present.

## Guarantees And Limits

- Exact reviewed provider and concrete Item signature; normalized safe Rust
  ABI, exact generic arguments, canonical source type/physical ABI equality.
- Projection consumes complete branded Self, exact 16-byte/alignment4 layout,
  actual [f32;4] field, invariant Format/Brand/lifetime and private markers.
  Projection does not issue a Brand; arbitrary exact source Brand is allowed
  only for this field extraction, as in the reviewed Rust method.
- Mixed MFMA retains the actual shared PolicyGfx950Matrix pointee and all
  three full fragment types; exact FP4 A, FP8 B, FP4 accumulator and unchanged
  result type. The shared bound field retains both Matrix and policy refs.
  Existing source pair validation enforces full subgroup/epoch, Width64,
  bounded sealed E/R ancestry and StrictIeee. Register layouts remain exact.
- No scalar operand erasure, Current alias, bare MFMA acceptance, policy or
  target legalization, new authority graph, or replacement of numerical proof.
- Other accumulator zero, homogeneous MFMA, LDS and legacy slice source gates
  remain unchanged. A subsequent actual failure there is a separate frontier.

## Central Test Filters

- `gfx950_matrix_abi_scope_excludes_issuance_and_other_terminals` (ordinary).
- `policy_gfx950_terminal_abis_gfx942` (ignored, actual registered AMD source).
- `policy_gfx950_terminal_abis_gfx950` (ignored, actual registered AMD source).

The new AMD callbacks collect an actual registered kernel, retain its complete
live preflight/type/ABI producers, and invoke all three new ABI constructors.
They verify canonical input/output/context identities and exact source generic,
ownership, role, format, output, borrow and brand substitution negatives.
These are terminal ABI component callbacks, NOT whole-import or execution
tests; unrelated zero/SSA/legalization remains outside their acceptance claim.
Existing full-import and phase tests are unchanged. Parent owns their reruns
and actual mixed32-or-later source frontier validation after delegation.
