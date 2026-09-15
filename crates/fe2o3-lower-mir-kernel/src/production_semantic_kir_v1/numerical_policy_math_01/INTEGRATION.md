# Policy Math Lowering Checkpoint

## Status

The worker has added only `numerical_policy_math_01.rs`, its eight adapter tests,
and this note. No shared parent hook, schema, source collector, Pliron adapter,
backend, simulator, or analysis file was changed for this lowerer checkpoint.
The child is not included by the parent yet. Rustfmt parsing/checking is local;
compilation and tests remain central-parent owned. These are adapter tests, not
successful source-to-KIR integration evidence.

The parent reports the ten KIR schema tests passed at checkpoint7. Its isolated
checkpoint7 snapshot is `58f4789f`, tree `f0563226`. That checkpoint does not
contain this future lowerer integration.

## Implemented Adapter

`NumericalPolicyMathLoweringV1::new` maps the MIR19 seven type edges, policy,
kernel brand, strict consumer function and full provenance. It checks the three
shared-reference pointee edges, retained constructor field order, ZST referents,
and FP32 scalar type. It does not turn those facts into source custody.

`operation` assembles MathDerive, Bind, or F32 only with exact typed SSA operands:

- MathDerive takes one actual KernelContext value matching the root and brands.
- Bind takes the MathDerive result and policy-issuance result, in that order.
  Both nominal shared-reference identities remain in its signature.
- F32 takes the Bind result followed by one scalar, or three scalars for FMA.
  The bound-reference identity remains in its signature.

Every operation retains LIFETIME_VALIDITY, NUMERICAL_POLICY and TARGET_SUPPORT.
No adapter method emits a FloatOperation, creates a context value, issues a
policy, or certifies source provenance, dominance, borrow validity or numerical
refinement. KIR verification must still check the actual producer graph.

## Source Origin Requirements

1. MathDerive must be anchored to the source-authenticated original
   `KernelContext::math(&self)` occurrence and its actual context reference.
   The current getter is `fe2o3-device/src/context.rs`; it calls the legacy
   argument-free MathContextCurrent terminal through `DeviceMath::current_branded`.
   MIR19's PolicyMathF32 contract does not identify that getter occurrence or
   carry its receiver. The legacy marker, an ambient root, a matching type, and
   an arbitrary same-signature helper are not acceptable replacements.
2. Bind must be anchored to the retained original PolicyMathBind constructor.
   Its aggregate has ordered shared math/policy reference fields and a marker.
   Resolve both actual reference referents to dominating MathDerive and
   NumericalPolicyIssue values with the exact same KernelContext SSA producer.
   Constructor recognition by aggregate type alone is insufficient.
3. Track definition occurrences and storage generations, not just local/type
   IDs. Check ordered dominance, moves, overwrites, deinitialization, storage
   death and mutable/raw/fake borrows. Preserve shared reborrows only while
   both retained referents remain live. Reject ambiguous joins, parameters,
   enum reconstruction, selects and zero-sized constant reauthentication as
   constructor/issuer substitutes. Leave lifetime obligations open.

## Expanded Calls

The checked `SemanticExpandedRootV1` already provides instance records, original
function/block coordinates, expanded local origins, and ParameterTransfer /
ReturnTransfer / FrameStorageLive / FrameStorageDead attribution. A scoped
origin planner must consume those records together with the original admitted
functions and the expansion owner, then key each origin by instance and source
definition occurrence. It must check that the call's original arguments and
destination agree with the recorded frame transfers.

The current `SemanticFunctionLoweringV1` receives the expanded body and only an
optional expansion identity, not the instance/block/local origin tables. The
identity alone cannot identify a getter, constructor or transfer. Existing
execution-capability lowering explicitly rejects expanded calls because source
V1 does not encode checked call-instance coordinates. Do not remove that guard
globally to enable this consumer. First agree canonical occurrence attribution
for policy issuance, MathDerive, Bind and F32, and retain replay correspondence.

## Proposed Hooks, Not Applied

- Include the child beside `numerical_policy_01.rs`; its test module is nested
  in the child. Test filter: `numerical_policy_math_lowering_tests` (eight tests).
- Replace only the NumericalPolicyMath result-type rejection with
  `numerical_policy_math_result_type_v1`, rejecting workgroup/epoch metadata
  instead of silently discarding it.
- Supply the checked source/expansion origin plan to function lowering. Do not
  register a type-only recipe as authority. Introduce typed source/bound SSA
  transport only with extraction and rehydration requiring one actual value.
- Emit MathDerive at the authenticated getter occurrence, Bind at the checked
  aggregate constructor, then F32 at its exact consumer call. Preserve the
  original operation spans and origin correspondence.
- Replace PolicyMathF32's temporary rejection only after the complete producer
  and borrow path is available. A dedicated per-source missing-custody error is
  preferable to falling back to MathContext or bare FP32 operations.

## Wire Revision Evidence

The current partition codec test asserts `[2, 24]` and rejects revisions 0, 1,
3 and 255. The checkpoint5 partition log records that test passing. Tracked HEAD
predates these extensions and accepts only revision1. There is no inspected
evidence of an accepted partition3 encoding to preserve or rename. Existing
partition2/policy2 bytes stay unchanged; policy Math uses closed revision4/tag26.
Operation25 remains reserved for the concurrent borrowed-subgroup work.
