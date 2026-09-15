# KIR Policy Math Checkpoint

This checkpoint changes kernel-ir only: the execution-capability parent, its
policy-math child/tests, the verifier parent and a new verifier child. No
partition child, MIR, source importer, lowerer, backend, simulator or analysis
is changed. Parent owns Cargo and downstream integration.

## Closed Contract

`ExecutionCapabilityOperationV1::NumericalPolicyMath` has three closed cases:

- `MathDerive { context, binding }`: one existing KernelContext SSA operand,
  source signature `[context] -> binding.math`, one typed Math-source result.
- `Bind { binding }`: two existing SSA operands `[math, policy]`, source
  signature `[math_reference, policy_reference] -> bound`, one typed bound result.
- `F32 { binding, bound_reference, element, function }`: existing bound SSA
  operand followed by one FP32 value (three for FMA), source signature
  `[bound_reference, element...] -> element`, one FP32 result.

`NumericalPolicyMathBindingV1` retains `math_reference`, `math`,
`policy_reference`, `capability`, `bound`, `bound_reference`, `policy`,
`kernel_brand`, and `mode`. All eight identities must be nonzero and distinct.
The consumer's `bound_reference` must exactly equal the binding field; its
FP32 element identity must be distinct from every binding identity. This maps
the seven source/MIR type edges plus the policy and kernel-brand identities.
StrictIeee is the only accepted mode; Abs is outside the closed 13-function set.

The two new roles are `NumericalPolicyMathSource(binding)` and
`NumericalPolicyMathBound(binding)`. Their source types are exactly `math` and
`bound` respectively, with full kernel provenance and no workgroup/epoch fields.
They are logical typed values, never empty aggregates or numerical evidence.

Source/MIR to KIR mapping:

| Source field | KIR field |
| --- | --- |
| `types.bound_reference` | `binding.bound_reference` and `F32.bound_reference` |
| `types.bound` | `binding.bound` |
| `types.math_reference` | `binding.math_reference` |
| `types.math` | `binding.math` |
| `types.policy_reference` | `binding.policy_reference` |
| `types.capability` | `binding.capability` |
| `types.element` | `F32.element` |
| `policy()` | `binding.policy` |
| `kernel_brand()` | `binding.kernel_brand` |
| `provenance()` | outer contract and logical result type provenance |
| `source_identity()` | `contract.source.function` |

Map TypeIds through exact admitted source type identities, never ordinal bytes.
`contract.source.operation` and `.block` require the original checked operation
occurrence/call-instance identity; they cannot be invented from type metadata.
Map `function` exactly and validate its required strict implementation. KIR's
closed function tag determines that requirement; it does not encode or admit
an alternative implementation under the same tag.

## Codec

New contracts and types use payload revision 4. The operation tag is 26,
leaving 25 available to the concurrent subgroup transport work. Math subtags
are 0=Bind, 1=F32, 2=MathDerive. Role tags are 13=MathSource and 14=MathBound.
The eight binding identities are encoded in the order listed above, followed
by strict mode byte 0. MathDerive appends context; F32 appends bound_reference,
element, and function tag 0..12. Revision 4 retains u32 obligation bits.
Unknown versions, subtags, modes, function tags, trailing bytes, truncation,
wrong arity and version-only relabelling reject.

Existing issuance payload 2 and all pre-existing bytes are untouched. At the
start of this checkpoint partition also used revision 2 in the shared parent;
revision 3 was not yet present. The parent's requested partition-3 integration
must preserve its bytes and explicitly update the shared revision dispatch in
coordination with the subgroup worker. This checkpoint does not reinterpret
any partition operation or role as math.

## Custody And Obligations

The verifier requires direct, sole-result, dominating issuers:
KernelContextIssue -> MathDerive; KernelContextIssue -> NumericalPolicyIssue;
both feed Bind; Bind feeds F32. Math and policy must consume the SAME root SSA
ValueId, not merely identically branded roots. Every issuer/consumer must agree
on full provenance. Math/Bind/F32 must agree on the complete binding, including
reference identities, policy and nominal kernel brand. Operand/result types
are checked against exact owned source identities and roles.

Generic calls, selects, function parameters and block parameters cannot issue
these values. A dominating existing origin may be reused across blocks; a phi
is not accepted as a replacement constructor. Identity-preserving source moves
and shared reborrows must be validated by the future MIR-to-KIR integration
before mapping them to that same SSA ValueId. KIR has no source-place lifetime
model, so this checkpoint does NOT certify moves, overwrites or source borrows.
`LIFETIME_VALIDITY` remains open; unsupported custody transformations reject.

Every math case retains `LIFETIME_VALIDITY | NUMERICAL_POLICY | TARGET_SUPPORT`.
Consumers additionally declare `Execution::Numerical { F32, StrictIeee }` on
both function and module. `numerical_requirements()` returns the strict mode
and operation-specific required implementation, never implementation evidence.
The previous `strict_float_operation` policy-erasure helper was removed.

## Parent Hooks

- Pauli's MIR V19 consumer maps its seven IDs and policy/brand field-for-field.
  Keep its checked mode/implementation requirements; do not map to bare MathF32.
- Derive Math only from the authenticated original KernelContext::math source
  origin. Bind only from the checked original pairing constructor and its two
  actual reference referents, not from consumer type metadata alone. Retain
  source operation identities, call-instance custody, move/reborrow validity.
- New operation matches are needed in lowerer/backend/sim and execution analysis;
  new role matches are needed wherever execution roles are exhaustively matched.
  Until implemented, reject the named unsupported variants explicitly. Do not
  erase capability operands to satisfy exhaustiveness.
- Uniformity can conservatively join every actual operand. Numerical refinement,
  target support, source lifetime proof, final graph custody, and FP execution
  remain outstanding downstream obligations.

Ten unit tests cover all 13 functions, full V13 round trips, payload/type gates,
truncation, typed operand order, exact reference/brand/policy edges, provenance,
dominating issuance/constructor, non-issuer rejection, cross-block origin/phi
handling, and required declarations. They are not run by this worker: no Cargo,
SSH or network. Rustfmt and parent parse checks are the local checkpoint only.
