/// Fresh complete-only memory analysis of the actual checked policy-3 output.
///
/// This report borrows the policy-3 checked owner without copying its executable
/// graph. Each result is freshly derived from that owner's actual output, using
/// the same fixed structural witness as the legacy analysis. Historical source,
/// ranked, and compiler discharges are not imported. The report cannot replace
/// a legacy checked owner or a final source/ranked admission owner.
///
/// This is not runtime-launch authentication or target authorization. Private
/// memory is outside the formal engine's modeled accesses, and memory
/// completeness does not establish absence of traps. The existing formal
/// engine's work, scratch, and obligation payload are not charged to the
/// canonical optimizer ledger or covered by the checked owner's storage receipt.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     CheckedOutputFormalMemoryAnalysisPolicy3V1,
///     analyze_checked_output_formal_memory_policy3_v1,
/// };
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn escape(owner: CheckedNeutralKernelIrOwnerPolicy3V1)
///     -> CheckedOutputFormalMemoryAnalysisPolicy3V1<'static>
/// {
///     analyze_checked_output_formal_memory_policy3_v1(&owner).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::analyze_checked_output_formal_memory_v1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
/// fn legacy(owner: &CheckedNeutralKernelIrOwnerPolicy3V1) {
///     let _ = analyze_checked_output_formal_memory_v1(owner);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::analyze_checked_output_formal_memory_policy3_v1;
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;
/// fn policy3(owner: &CheckedNeutralKernelIrOwnerV1) {
///     let _ = analyze_checked_output_formal_memory_policy3_v1(owner);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     CheckedOutputFormalMemoryAnalysisPolicy3V1, ProductionFormalMemoryOwnerV1,
/// };
/// fn admit(report: CheckedOutputFormalMemoryAnalysisPolicy3V1<'_>)
///     -> ProductionFormalMemoryOwnerV1
/// {
///     report
/// }
/// ```
pub struct CheckedOutputFormalMemoryAnalysisPolicy3V1<'o> {
    checked: &'o fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    kernels: Box<[FormalMemoryObligations]>,
}

impl CheckedOutputFormalMemoryAnalysisPolicy3V1<'_> {
    /// The exact checked policy-3 output borrowed by this analysis.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.checked.owner()
    }

    /// Fresh complete obligations in the actual output module's kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }
}

/// Analyze every actual checked policy-3 output kernel with the fixed witness.
///
/// An empty kernel roster, incomplete extraction, or inter-invocation conflict
/// rejects. No source/ranked discharge or final admission is accepted or
/// constructed. This uses the legacy, non-canonical-ledger-metered formal engine.
pub fn analyze_checked_output_formal_memory_policy3_v1(
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
) -> Result<CheckedOutputFormalMemoryAnalysisPolicy3V1<'_>, ProductionFormalMemoryErrorV1> {
    Ok(CheckedOutputFormalMemoryAnalysisPolicy3V1 {
        checked,
        kernels: derive_complete_output_obligations_v1(checked.owner().module())?,
    })
}

#[cfg(test)]
#[path = "production_checked_output_formal_policy3_v1_tests.rs"]
mod checked_output_formal_policy3_v1_tests;
