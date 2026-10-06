//! Final LICM target text and descriptor-bound Worker inputs.
//! Shared target/contract engines retain the actual final-native owner, not a prefix.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedLicmOutputHandoffV28 as LicmHandoff,
    ProductionConditionalPredicatedLicmOutputHandoffV90 as PredicatedHandoff,
    ProductionContinuationOccurrenceV90 as Occurrence,
    ProductionMixedPrefixOwnerV29 as PrefixOwner,
};

// Reborrow every retained prefix at the final continuation's lexical lifetime.
// This only shortens borrows: no graph, policy or proof owner is converted.
type MixedHandoff<'view, 'source> = LicmHandoff<'view, 'view, 'view, 'source>;

#[path = "production_worker_mixed_input_v26.rs"]
pub(crate) mod worker_input_v26;

/// Target text cannot outlive the actual source and conditional mixed handoff.
/// The caller must still bind every original ABI field and occurrence through
/// `emit_mixed_contract_v26` to the genuine generated V3 descriptor before a
/// versioned Worker-input admission. No V8/V12 proof holder is constructed here.
pub(crate) type ConditionalMixedTargetLlvmV26<
    'handoff,
    'view,
    'source,
    H = MixedHandoff<'view, 'source>,
> = TargetLlvmV29<'handoff, 'view, 'source, H>;
mixed_target_contract_v29!([<'v, 's, P: PrefixOwner<'v, 's>>] LicmHandoff<'_, '_, 'v, 's, P>);
mixed_target_contract_v29!([] PredicatedHandoff<'_, '_, '_, '_>, emit_predicated_contract_v90,
    fe2o3_kernel_descriptor::mixed_conditional_v86::MAX_MIXED_CONTRACT_BYTES_V86);

impl<'v, 's, R: Occurrence, P: PrefixOwner<'v, 's, R>> target_handoff_sealed::Sealed
    for LicmHandoff<'_, '_, 'v, 's, P, R>
{
}
impl<'v, 's, R: Occurrence, P: PrefixOwner<'v, 's, R>> TargetOutputHandoffV29
    for LicmHandoff<'_, '_, 'v, 's, P, R>
{
    fn check_original(
        &self,
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), SourceError> {
        self.check_original_source(source, budget)
    }

    fn owner(&self, budget: &Budget<'_>) -> Result<&CanonicalOwner, SourceError> {
        self.output(budget)
    }

    fn observe_retained_storage(
        &self,
        required: usize,
        budget: &Budget<'_>,
    ) -> Result<(), SourceError> {
        self.observe_retained_storage_v28(required, budget)
    }

    fn formal(&self, owner: &CanonicalOwner, budget: &mut Budget<'_>) -> Result<(), Error> {
        let output = self.output(budget)?;
        let prefix = self
            .relocation(budget)?
            .prefix(budget)?
            .checked_prefix_v29(budget)?;
        // Original N must be the retained source input to the checked prefix;
        // final O must be the exact freshly completed LICM owner. The prefix output
        // is deliberately not accepted as a substitute final endpoint.
        budget.charge_work(1)?;
        if !std::ptr::eq(owner, output) {
            budget.charge_work(sum(&[
                owner.canonical_bytes().len(),
                prefix.input_audit_bytes().len(),
            ])?)?;
            if owner.canonical_bytes() != prefix.input_audit_bytes() {
                return Err(Error::Unsupported(
                    "mixed LICM target original endpoint changed",
                ));
            }
        }
        let (launches, _) = self.launch_context(budget)?;
        budget.charge_work(sum(&[launches.len(), owner.module().kernels.len(), 1])?)?;
        if launches.len() != owner.module().kernels.len() {
            return Err(Error::Unsupported("mixed target launch census changed"));
        }
        // These are the actual conditions used by source/native admission.
        // Dynamic logical geometry remains conditional on this exact launch;
        // it must not be replaced with an unconditional scalar formal scope.
        for (launch, kernel) in launches.iter().zip(&owner.module().kernels) {
            budget.charge_work(4)?;
            let ExplicitLaunchExtent::Exact { rank, extents } = launch else {
                return Err(Error::Unsupported("mixed target requires exact launch"));
            };
            if *rank != kernel.domain.rank() || extents.iter().any(|extent| *extent == 0) {
                return Err(Error::Unsupported("mixed target launch rank or extent"));
            }
        }
        Ok(())
    }

    fn formal_headers() -> Result<usize, Resource> {
        // No temporary formal analyzer or replacement proof object is built.
        Ok(size_of::<Result<(), Error>>()
            + size_of::<(&CanonicalOwner, &CanonicalOwner, &[ExplicitLaunchExtent])>()
            + fe2o3_lower_mir_kernel::ProductionCheckedMixedPrefixViewV29::inspection_storage_v29(
            )?)
    }
}

/// Lower the exact adopted V18 module while borrowing its full conditional
/// premise owner. This produces inert text only, not Worker/default activation,
/// descriptor authority, runtime premise discharge, or LLVM refinement proof.
pub(crate) fn check_and_lower_mixed_target_llvm_v26<
    'handoff,
    'view,
    'source,
    P: PrefixOwner<'view, 'source, R>,
    R: Occurrence,
>(
    source: &'view Source<'source>,
    handoff: &'handoff LicmHandoff<'view, 'view, 'view, 'source, P, R>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<
    ConditionalMixedTargetLlvmV26<
        'handoff,
        'view,
        'source,
        LicmHandoff<'view, 'view, 'view, 'source, P, R>,
    >,
    Error,
> {
    check_and_lower_target_llvm(source, handoff, target, budget)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::tests::{Mode, genuine_case_for};
    use super::*;

    fn independent_headers() -> (usize, usize) {
        type H<'a, 'b> = MixedHandoff<'a, 'b>;
        type Capture<'a, 'view, 'source, 'work> = (
            &'view Source<'source>,
            &'a H<'view, 'source>,
            TargetProfile,
            &'a mut Budget<'work>,
            &'a std::cell::Cell<usize>,
            usize,
            usize,
        );
        type Outcome = Result<(String, usize), Error>;
        (
            size_of::<ConditionalMixedTargetLlvmV26<'_, '_, '_>>()
                + align_of::<ConditionalMixedTargetLlvmV26<'_, '_, '_>>(),
            size_of::<Capture<'_, '_, '_, '_>>()
                + align_of::<Capture<'_, '_, '_, '_>>()
                + size_of::<AssertUnwindSafe<Capture<'_, '_, '_, '_>>>()
                + size_of::<Outcome>()
                + align_of::<Outcome>()
                + size_of::<std::thread::Result<Outcome>>()
                + size_of::<Result<(), Error>>()
                + size_of::<(&CanonicalOwner, &CanonicalOwner, &[ExplicitLaunchExtent])>()
                + fe2o3_lower_mir_kernel::ProductionCheckedMixedPrefixViewV29::inspection_storage_v29().unwrap()
                + size_of::<Result<String, fe2o3_amdgcn_model::LoweringErrors>>()
                + size_of::<Result<(), Error>>()
                + size_of::<fe2o3_kernel_ir::FormalMemoryObligations>()
                + size_of::<[u64; 3]>()
                + size_of::<std::cell::Cell<usize>>()
                + size_of::<Result<(), SourceError>>()
                + align_of::<Result<(), SourceError>>(),
        )
    }

    #[test]
    fn conditional_mixed_target_frames_have_an_independent_header_oracle() {
        assert_eq!(
            headers_for::<MixedHandoff<'_, '_>>().unwrap(),
            independent_headers()
        );
    }

    #[test]
    fn final_licm_worker_refusal_keeps_its_distinct_native_handoff_boundary() {
        let error = worker_input_v26::MixedWorkerInputErrorV26::Mismatch("final LICM contract");
        let error = crate::production_pipeline::source_owned_v29::Error::from(error);
        assert!(matches!(
            error,
            crate::production_pipeline::source_owned_v29::Error::MixedLicmWorkerInput(
                worker_input_v26::MixedWorkerInputErrorV26::Mismatch("final LICM contract")
            )
        ));
    }

    /// Called only from a real Rust transaction's mixed source callback.
    pub(crate) fn genuine_mixed_case(
        source: &Source<'_>,
        handoff: &MixedHandoff<'_, '_>,
        target: TargetProfile,
        budget: &mut Budget<'_>,
        mode: Mode,
    ) -> Result<(), Error> {
        assert!(!handoff.runtime_requirements_are_discharged());
        assert!(!handoff.grants_artifact_or_launch_authority());
        genuine_case_for(source, handoff, target, budget, mode, independent_headers())
    }
}
