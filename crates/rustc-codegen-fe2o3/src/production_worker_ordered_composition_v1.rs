//! Same-owner normal worker preparation; explicit inert extraction is the only
//! demotion. Conditional bounds/initialization/permissions and original formal
//! obligations survive in checked until that demotion, never as launch authority.
use super::*;
use crate::production_ordered_composition_source_v1::AuthenticatedOrderedCompositionSourceSeedV1;
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger;
use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1 as Checked;

pub(crate) struct PreparedOrderedCompositionWorkerHandoffV1<'tcx> {
    prepared: PreparedProductionWorkerHandoff,
    checked: Checked,
    _typed_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    _source_seed: AuthenticatedOrderedCompositionSourceSeedV1<'tcx>,
    _ledger: Ledger,
}
impl PreparedOrderedCompositionWorkerHandoffV1<'_> {
    pub(crate) fn checked(&self) -> &Checked {
        &self.checked
    }
    /// Test-only consuming observation after the ordinary validated handoff.
    /// No live source owner escapes; the unchanged ledger is retained by the
    /// caller until its inert observation has been serialized and consumed.
    #[cfg(test)]
    pub(crate) fn into_inert_transport_observation<T>(
        mut self,
        observe: impl FnOnce(
            &Checked,
            &CompilerModuleHandoffV2,
            &CompilerDescriptorSourceV1,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<T, String>,
    ) -> Result<(T, Ledger), String> {
        let (handoff, descriptor) = self
            .prepared
            .into_validated_parts()
            .map_err(|e| e.to_string())?;
        let result = self
            ._ledger
            .with_budget(|budget| observe(&self.checked, &handoff, &descriptor, budget))?;
        // Source/check/typed-root fields remain alive throughout observe().
        // Charges stay conservative after their drop; there is no refund/reset.
        Ok((result, self._ledger))
    }
    /// Erases live source/check custody. Returned protocol data remains inert.
    pub(crate) fn into_inert_for_extraction(
        self,
    ) -> Result<(CompilerModuleHandoffV2, CompilerDescriptorSourceV1), ProductionWorkerHandoffError>
    {
        self.prepared.into_validated_parts()
    }
}
pub(crate) fn prepare_ordered_composition_worker_handoff_v1<'tcx>(
    authenticated: crate::production_pipeline::ordered_composition_target_v1::AuthenticatedOrderedCompositionTargetModuleV1<'tcx>,
) -> Result<PreparedOrderedCompositionWorkerHandoffV1<'tcx>, ProductionWorkerHandoffError> {
    let (checked, target, emission, typed_roots, observed_source_envelope, source_seed, mut ledger) =
        authenticated.into_parts();
    let prepared = ledger.with_budget(|budget| {
        let canonical = checked.executable();
        let module = canonical.module();
        if target.to_string() != "gfx942:xnack-" || canonical.identity() != emission.canonical_identity() {
            return Err(ProductionWorkerHandoffError::MissingProductionBindings);
        }
        validate_exact_target_binding(target, module)?;
        let compiler_module = crate::kernel_ir_codegen::retain_verified_ordered_composition_compiler_module_text_v1(
            checked.composition(), emission,
        ).map_err(ProductionWorkerHandoffError::CompilerModule)?;
        let envelope = derive_production_compiler_ffi_envelope(target, module, &compiler_module,
            observed_source_envelope, *canonical.identity().digest())?;
        validate_exact_target_binding(envelope.target(), module)?;
        validate_envelope_module_roles(&envelope, &compiler_module)?;
        let descriptor = crate::compiler_descriptor::ordered_composition_v1::construct_ordered_composition_descriptor_source_v1(
            &envelope, &compiler_module, &typed_roots, &checked, budget,
        ).map_err(ProductionWorkerHandoffError::CompilerDescriptor)?;
        assemble_production_worker_handoff(target, envelope, compiler_module, descriptor)
    })?;
    Ok(PreparedOrderedCompositionWorkerHandoffV1 {
        prepared,
        checked,
        _typed_roots: typed_roots,
        _source_seed: source_seed,
        _ledger: ledger,
    })
}
