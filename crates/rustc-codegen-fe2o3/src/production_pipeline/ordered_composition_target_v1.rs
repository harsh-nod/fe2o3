//! Source-owned checked composition continuation. The input is the private live
//! importer owner, never an exported diagnostic or caller LLVM string.
use super::*;
use crate::production_ordered_composition_source_v1::AuthenticatedOrderedCompositionSourceSeedV1;
use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger;
use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1 as Checked;

/// Keeps the original source/launch/SSA owner and all original plus conservative
/// memory conditions alive until explicitly inert extraction.
pub(crate) struct AuthenticatedOrderedCompositionTargetModuleV1<'tcx> {
    checked: Checked,
    emission: dialect_amdgcn::OrderedProgramCompositionCanonicalEmissionV1,
    bindings: AuthenticatedProductionBindings,
    source_seed: AuthenticatedOrderedCompositionSourceSeedV1<'tcx>,
    ledger: Ledger,
}
impl<'tcx> AuthenticatedOrderedCompositionTargetModuleV1<'tcx> {
    pub(crate) fn checked(&self) -> &Checked {
        &self.checked
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.emission.llvm_ir()
    }
    pub(crate) const fn target_name(&self) -> &'static str {
        "gfx942:xnack-"
    }
    /// Test-only inspection over the actual retained owner and its original ledger.
    #[cfg(test)]
    pub(crate) fn observe_transport_with_budget<T>(
        &mut self,
        observe: impl FnOnce(
            &Checked,
            &str,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> T,
    ) -> T {
        self.ledger
            .with_budget(|budget| observe(&self.checked, self.emission.llvm_ir(), budget))
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        Checked,
        fe2o3_compiler_ffi::DeviceTargetV1,
        dialect_amdgcn::OrderedProgramCompositionCanonicalEmissionV1,
        Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
        Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
        AuthenticatedOrderedCompositionSourceSeedV1<'tcx>,
        Ledger,
    ) {
        (
            self.checked,
            self.bindings.rustc_target.device_target(),
            self.emission,
            self.bindings.typed_descriptor_roots,
            self.bindings.transaction.compiler_ffi_envelope,
            self.source_seed,
            self.ledger,
        )
    }
}
impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn lower_ordered_composition_target_v1(
        self,
    ) -> Result<AuthenticatedOrderedCompositionTargetModuleV1<'tcx>, String> {
        // This consuming seam invokes the dedicated actual rustc importer. Its
        // name describes the first supported output, not a bytes-to-owner API.
        let actual = self.lower_ordered_composition_diagnostic_v1()?;
        let (materialized, emission, bindings, source_seed, mut ledger) =
            actual.into_source_parts();
        let [root] = bindings.typed_descriptor_roots.as_slice() else {
            return Err("normal ordered composition requires one authenticated typed root".into());
        };
        let launch = root
            .source_launch()
            .ok_or_else(|| "normal ordered composition source launch missing".to_owned())?;
        if launch.max_grid().x() == u32::MAX {
            return Err(
                "normal ordered composition requires explicit finite source max_grid".into(),
            );
        }
        let retained = materialized.retained_storage().retained_storage();
        let checked = ledger.with_budget(|budget| {
            let checked = Checked::try_check(materialized, budget)
                .map_err(|e| format!("ordered composition mandatory formal/ranked checks: {e}"))?;
            let added = checked
                .retained_storage()
                .checked_sub(retained)
                .ok_or_else(|| "ordered composition checked retention underflow".to_owned())?;
            budget
                .reserve_storage(added)
                .map_err(|e| format!("ordered composition checked retention: {e}"))?;
            // The immutable pre-ranked emission remains the same executable,
            // not the conservative safety projection used by the checker.
            checked
                .verify_equivalence(budget)
                .map_err(|e| format!("ordered composition checked source replay: {e}"))?;
            if checked.executable().identity() != emission.canonical_identity() {
                return Err("ordered composition checked emission identity differs".to_owned());
            }
            Ok::<_, String>(checked)
        })?;
        Ok(AuthenticatedOrderedCompositionTargetModuleV1 {
            checked,
            emission,
            bindings,
            source_seed,
            ledger,
        })
    }
}
