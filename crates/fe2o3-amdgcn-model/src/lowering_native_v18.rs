// Actual-owner V18 counterpart. Historical and V12 paths remain independent.
impl ProductionSemanticAnchorKirIdentityV1 {
    /// Inert identity of the actual admitted V18 owner. An identity alone does
    /// not admit a historical Module/identity pair or a replacement executable.
    pub fn from_v18(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18) -> Self {
        Self {
            version: 18,
            sha256: *owner.identity().digest(),
            byte_len: owner.identity().canonical_length(),
        }
    }
}

/// Emits inert target LLVM IR from the actual borrowed V18 owner.
/// No V12 conversion, replacement owner, reencoding or optimizer is run.
/// Typed target selection does not authenticate a target. Unlike the legacy
/// exact-target entry, neutral V18 KIR needs no fabricated capability rows.
/// Every declared capability, including conflicting target claims, is checked.
/// Existing complete-module feature, geometry, target/call-graph,
/// helper-coverage and text limits apply unchanged. This supplies no source,
/// formal, artifact, worker, or runtime launch authority. Target allocations
/// retain the emitter's separate resource policy, not a canonical-work ledger.
pub fn lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_to_llvm_ir_for_target(
        owner.module(),
        LoweringTarget::Gfx942XnackMinusV1,
        None,
        Some(SemanticAnchorInputV1::NativeV18(owner)),
        true,
    )
}

/// Exact gfx950:xnack- counterpart, still inert target LLVM IR.
pub fn lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_to_llvm_ir_for_target(
        owner.module(),
        LoweringTarget::Gfx950XnackMinusV1,
        None,
        Some(SemanticAnchorInputV1::NativeV18(owner)),
        true,
    )
}

#[cfg(test)]
mod native_v18_identity_tests {
    use super::*;
    include!("lowering_native_v18_tests.rs");
}
