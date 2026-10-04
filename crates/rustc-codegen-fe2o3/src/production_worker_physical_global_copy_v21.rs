//! Explicit inert preparation only; no native/protected/finalizer/host conversion.
//! Source/replay/ABI charges use the same retained logical ledger. Existing
//! descriptor, FFI, symbol-roster and handoff construction keep their separately
//! bounded allocation domains; this is not whole-compiler heap accounting.
use super::*;
use crate::compiler_descriptor::physical_global_copy_v21::PhysicalGlobalCopyPreparedAbiV21;
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger;
use fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopyCheckedKirOwnerV21 as Checked;

pub(crate) struct PreparedPhysicalGlobalCopyWorkerHandoffV21 {
    prepared: PreparedProductionWorkerHandoff,
    _checked: Checked,
    abi: PhysicalGlobalCopyPreparedAbiV21,
    _typed_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    _ledger: Ledger,
}
impl PreparedPhysicalGlobalCopyWorkerHandoffV21 {
    pub(crate) fn unresolved_abi_conditions(&self) -> &'static str {
        self.abi.unresolved_conditions()
    }
    /// Explicit demotion only: inert protocol bytes/table, not checked native
    /// output or discharged runtime/allocation/launch authority.
    pub(crate) fn into_inert_for_extraction(
        self,
    ) -> Result<(CompilerModuleHandoffV2, CompilerDescriptorSourceV1), ProductionWorkerHandoffError>
    {
        let Self { prepared, .. } = self;
        prepared.into_validated_parts()
    }
}
pub(crate) fn prepare_physical_global_copy_worker_handoff_v21(
    authenticated: crate::production_pipeline::AuthenticatedPhysicalGlobalCopyTargetModuleV21,
) -> Result<PreparedPhysicalGlobalCopyWorkerHandoffV21, ProductionWorkerHandoffError> {
    let (checked, target, emission, typed_roots, observed_source_envelope, abi, mut ledger) =
        authenticated.into_parts();
    let prepared = ledger.with_budget(|budget| {
        let canonical = checked.executable();
        let module = canonical.module();
        if target.to_string() != "gfx942:xnack-" || emission.canonical_identity() != canonical.identity() {
            return Err(ProductionWorkerHandoffError::MissingProductionBindings);
        }
        validate_exact_target_binding(target, module)?;
        let compiler_module =
            crate::kernel_ir_codegen::retain_verified_physical_global_copy_compiler_module_text_v21(canonical, emission)
                .map_err(ProductionWorkerHandoffError::CompilerModule)?;
        let envelope = derive_production_compiler_ffi_envelope(
            target, module, &compiler_module, observed_source_envelope, *canonical.identity().digest(),
        )?;
        validate_exact_target_binding(envelope.target(), module)?;
        validate_envelope_module_roles(&envelope, &compiler_module)?;
        let descriptor = crate::compiler_descriptor::physical_global_copy_v21::construct_physical_global_copy_descriptor_source_v21(
            &envelope, &compiler_module, &typed_roots, &checked, &abi, budget,
        ).map_err(ProductionWorkerHandoffError::CompilerDescriptor)?;
        assemble_production_worker_handoff(target, envelope, compiler_module, descriptor)
    })?;
    // Exact source, combined global/kernarg report and all conditional facts
    // survive until the one explicit inert-extraction demotion above.
    Ok(PreparedPhysicalGlobalCopyWorkerHandoffV21 {
        prepared,
        _checked: checked,
        abi,
        _typed_roots: typed_roots,
        _ledger: ledger,
    })
}
