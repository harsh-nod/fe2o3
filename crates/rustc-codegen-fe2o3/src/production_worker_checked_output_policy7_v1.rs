//! Closed actual-J worker adapters, not a signed or publication constructor.
use super::*;

pub(crate) fn prepare_checked_output_policy7_worker_handoff(
    admitted: &fe2o3_lower_mir_kernel::ProductionOwnedRedundantStoreContinuationV1,
    catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    llvm_ir: String,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    observed_source_envelope: Option<CompilerFfiEnvelopeV1>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    prepare_checked_output_worker_handoff_v1(
        CheckedOutputOwnerRefV1::Direct7(admitted),
        catalog,
        target,
        llvm_ir,
        typed_roots,
        observed_source_envelope,
        budget,
    )
}

pub(crate) fn prepare_erased_checked_output_policy7_worker_handoff(
    admitted: &fe2o3_lower_mir_kernel::ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    llvm_ir: String,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    observed_source_envelope: Option<CompilerFfiEnvelopeV1>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    prepare_checked_output_worker_handoff_v1(
        CheckedOutputOwnerRefV1::Erased7(admitted),
        catalog,
        target,
        llvm_ir,
        typed_roots,
        observed_source_envelope,
        budget,
    )
}
