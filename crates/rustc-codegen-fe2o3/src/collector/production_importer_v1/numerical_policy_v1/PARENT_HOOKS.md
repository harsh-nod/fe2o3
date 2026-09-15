# Numerical policy FP32 handoff

The source importer now distinguishes `PolicyMathF32` from historical bare math
and calls `policy_math_terminal_operation_v1`. The resulting MIR V19 consumer
retains the complete type, policy, brand, implementation and root contract.
Root carriage is rechecked after admission. The new source hooks await the
production compiler rebuild and actual AMD callback test; end-to-end consumer
lowering and proof admission remain incomplete.

Central component tests pass: 169 MIR-model tests (including 11 new policy-math
tests), 10 KIR policy-math tests, and 220 kernel-analysis tests. These are not
source-to-GPU results or successful numerical-refinement receipts.

## Exact changed files

Paths relative to `crates/rustc-codegen-fe2o3/src/`:

```text
collector/production_importer_v1/numerical_policy_v1.rs
collector/production_importer_v1/numerical_policy_v1/bind/tests.rs
collector/production_importer_v1/numerical_policy_v1/math.rs
collector/production_importer_v1/numerical_policy_v1/math/contract.rs
collector/production_importer_v1/numerical_policy_v1/math/contract/tests.rs
collector/production_importer_v1/numerical_policy_v1/standalone.rs
collector/production_importer_v1/numerical_policy_v1/PARENT_HOOKS.md
production_ranked_projection_v1/numerical_policy_v1.rs
production_ranked_projection_v1/numerical_policy_v1/legacy_math.rs
production_ranked_projection_v1/numerical_policy_v1/legacy_math/tests.rs
```

## Source parent

1. `production_semantic_terminal_v1.rs` distinguishes policy-bound FP32
   terminals from historical bare `MathF32` expansion. The diagnostic item must
   still be authenticated through `trusted_device_items::classify`; do not use
   a method name or the wrapper's layout as authority. Keep PolicyMathBind and
   PolicyMatrixBind as traversed original Rust constructors.
2. `collector/production_importer_v1.rs` includes the new policy FP expansion
   in `capability_memory_root_for_terminal_v1` and its carriage validation. It
   requires unique caller/root custody, including through helper expansion.
3. `terminal_operation_v1` calls the canonical wrapper around this source
   validation entry point:

   ```rust
   let source = numerical_policy_v1::policy_math_source_contract_v1(
       tcx, instance, function, abi, types, capability_root,
       source_identity, kernel_contexts,
   )?;
   ```

   Encode every `source.types()` edge, `source.policy()`, `kernel_brand()`,
   `function()`, `provenance()`, `source_identity()`, `arguments()`, and
   `numerical_requirements()` in the new canonical operation. The last pair
   specifies StrictIeee and the required operation-specific implementation;
   it does not certify numerical refinement or target support. Do not convert
   `source` to the existing policy-free `SemanticCompilerIntrinsicOperationV1::MathF32`.

## Canonical and lowering parents

- MIR V19 adds the policy-bearing FP consumer with exact type collection,
  source ABI checking, canonical encode/decode and direct-call expansion tests.
  Historical MIR V18 issuance and bare-math bytes remain unchanged.
- KIR now retains MathDerive, Bind and F32 cases, exact reference identities,
  policy and brand. Its verifier checks the dominating producer chain and the
  same root SSA value for math and policy issuance. MIR-to-KIR integration must
  derive Bind from the retained original constructor with both actual reference
  referents; consumer type metadata cannot manufacture either issuer.
- Verify dominating issuance, matching root/policy/brand, constructor inputs,
  shared reborrows, moves, overwrites, joins, and outstanding lifetime obligations
  before legalization. Constructor shape or unused issuance alone grants no
  consumer authority.
- Carry NUMERICAL_POLICY, LIFETIME_VALIDITY and TARGET_SUPPORT obligations and
  operation-specific numerical refinement requirements to final graph and target
  checks. Preserve explicit FMA fusion and separate ordinary scalar rounding.
  Do not clear refinement findings, substitute relaxed modes, or grant artifact
  or launch authority on source validation alone.
- Ranked projection now rejects a legacy MathF32 context whose type graph
  retains an issued policy capability. This protects against an incomplete
  parent hook; it does not replace the SSA custody checks for the new operation.
- Packed BF16 and policy-bound matrix FP consumers still require their own
  exact contracts and legalization. The FP32 validator rejects the packed BF16
  terminal and bare math. Matrix constructors remain traversed and validated.

## Tests for parent integration

`collector::production_importer_v1::numerical_policy_v1` selects the source and
contract tests. The existing opt-in test
`policy_pairing_real_source_amdgpu_retains_references_and_rejects_substituted_types`
now checks the three real constructors, all 13 FP32 consumer signatures, retained
reference edges, substituted policies/brands/target/launch/function/arity,
bare-math rejection, and packed-BF16 rejection. It uses the cached AMD target
metadata and performs no dependency build. The host opt-in remains available.

Use these environment inputs supplied by the parent:

```sh
FE2O3_CORE_TRY_DEVICE_RMETA=/home/harsh/work/fe2o3-47-endtoend-20260910/local-amdgpu-metadata/amdgcn-amd-amdhsa/debug/deps/libfe2o3_device-c1806ab98746e656.rmeta
FE2O3_CORE_TRY_HOST_DEPS=/home/harsh/work/fe2o3-47-endtoend-20260910/local-amdgpu-metadata/debug/deps
FE2O3_CORE_TRY_AMDGPU_CORE=/home/harsh/work/fe2o3-47-endtoend-20260910/local-amdgpu-metadata/amdgcn-amd-amdhsa/debug/deps/libcore-d4cdd9c8b80fe1e2.rmeta
FE2O3_CORE_TRY_AMDGPU_BUILTINS=/home/harsh/work/fe2o3-47-endtoend-20260910/local-amdgpu-metadata/amdgcn-amd-amdhsa/debug/deps/libcompiler_builtins-dfaaf6f113af6683.rmeta
```

`production_ranked_projection_v1::numerical_policy_v1::legacy_math` selects
the projection type-graph guard tests. `standalone.rs` combines those with the
semantic consumer-contract tests using only matching cached MIR/KIR libraries.

Worker verification: the complete AMD source fixture passed direct pinned
rustc analysis with the cached metadata (three target-feature warnings). This
checks the real source API on AMD, not the importer callbacks. Standalone
contract execution is blocked by E0460: cached MIR/KIR libraries require a sha2
crate hash absent from the current dependency directories. Parent owns Cargo,
integrated callback tests, canonical round trips and all47. No Cargo, SSH or
network was invoked by this worker.
