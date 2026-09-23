//! Actual-L worker text/descriptor preparation, not protected publication.
use super::*;

pub(crate) fn prepare_source_local_order_worker_handoff_v1(
    admitted: &fe2o3_lower_mir_kernel::ProductionOwnedSourceLocalOrderContinuationV1,
    catalog: &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    llvm_ir: String,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    observed_source_envelope: Option<CompilerFfiEnvelopeV1>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    if target.to_string() != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942.device_target()
    {
        return Err(ProductionWorkerHandoffError::MissingProductionBindings);
    }
    prepare_checked_output_worker_handoff_v1(
        CheckedOutputOwnerRefV1::SourceLocalOrder(admitted),
        catalog,
        target,
        llvm_ir,
        typed_roots,
        observed_source_envelope,
        budget,
    )
}
