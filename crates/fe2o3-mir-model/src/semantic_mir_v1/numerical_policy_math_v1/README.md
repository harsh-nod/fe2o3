# Policy FP32 Consumer Handoff

MIR V19 adds `SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract }`,
compiler-intrinsic tag **79**. V18 policy issuance, subgroup partitions, and bare
`MathF32` (tag 30) keep their existing bytes and minimum versions. There is no
policy-bearing conversion to bare math and no policy-binding constructor tag.

Public API (re-exported from `semantic_mir_v1`):

- `SemanticNumericalPolicyMathTypesV1::new([TypeId; 7])`: ordered
  `bound_reference, bound, math_reference, math, policy_reference, capability,
  element`. Named public fields and `all()` preserve the source validator edges.
- `SemanticNumericalPolicyMathContractV1::new(types, policy_identity,
  kernel_brand_identity, function, mode, implementation, provenance,
  source_identity)` returns a structurally bounded inert contract.
- `SemanticNumericalModeV1::StrictIeee` is the only admitted mode. Pass the
  authenticated source mode explicitly; never translate unknown policy to it.
- `SemanticF32MathFunctionV1::required_implementation()` yields
  `IeeeSqrtRoundTiesEvenIgnoreExceptionsV1` for sqrt, `ConstrainedLlvm` for FMA
  and rounding, and `OcmlAbiV1` for transcendentals. Constructor and decoder
  reject a mismatched implementation. Abs is not a policy FP32 source terminal.
- Getters retain every constructor field; `signature()` derives the exact
  shared-wrapper receiver plus one FP32 operand (three for FMA), returning FP32.
  `numerical_requirements()` and `obligations()` are requirements, never proof.

Admission checks exact distinct identities, the three shared-reference edges,
the wrapper's ordered two references plus inhabited ZST marker, exact inhabited
ZST math/policy capabilities, FP32, direct Rust ABI and source ownership, source
function identity and root/kernel-entry provenance. Global type validation
establishes identity uniqueness. Fixed seven-edge checks do not scan the roster.
Claims for the same policy capability must agree on policy/provenance, including
issuance records; consumers must also agree on kernel brand. This consistency
check does NOT establish that issuance exists or dominates a consumer.

Parent / Bernoulli integration requirements:

- Convert the already authenticated source contract field-for-field, retaining
  exact policy and brand identities, provenance and source function identity.
- Keep `PolicyMathBind`/`PolicyMatrixBind` traversed as original defined Rust
  bodies. The consumer receiver still holds both source references.
- KIR must retain issuance and constructor custody in SSA and require numerical
  and target refinement. MIR admission supplies neither SSA dominance nor
  nominal authentication, effect proof, target support or W4 authority.
- Handle the new intrinsic and V19 in downstream exhaustive matches. Do not
  route it through the bare `MathF32` branch. KIR, collector and lowerer hooks
  are outside this change. Direct-call expansion in this crate retains source
  callable/type/root ordinals; it does not reconstruct intrinsic contracts.

Wire record (328 bytes including tag, little-endian integers): tag79; seven u32
type IDs; policy identity32; kernel-brand identity32; function u8 (existing
0..12 order); mode u8 (0=StrictIeee); implementation u8 (0=constrained, 1=OCML,
2=sqrt); provenance (u32 root + six identity32 fields); obligations u32
(`TARGET_SUPPORT | NUMERICAL_POLICY` = 0x10001); source-function identity32.
Unknown tags, missing/extra obligations, mismatched implementations, truncation
and resource-limit excess are rejected, not defaulted.

Tests live in `numerical_policy_math_decode_tests.rs`, included by the existing
canonical decoder test module. Filter `policy_math_` selects 11 tests covering
all 13 functions, signature/graph/identity mutations, exact and mixed record
roundtrips, V18 expected bytes, truncation, resource limits and call expansion.
Parent owns Cargo/integration execution; local checks are syntax/format only.
