//! Explicit source-owned LICM after genuine preheaders, before native emission.
//! Historical owners and the exact P7 witness remain live; no wire authority.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionLicmErrorV1 as LicmAdmissionError, ProductionOwnedLicmContinuationV1 as DirectLicm,
    ProductionOwnedUnitLocalLicmContinuationV1 as ErasedLicm,
};

#[path = "production_pipeline_induction_refinement_native_v1.rs"]
mod induction_refinement_native_v1;
pub(crate) use induction_refinement_native_v1::InductionRefinementNativeStageErrorV1;
#[path = "production_pipeline_cross_block_forwarding_native_v1.rs"]
mod cross_block_forwarding_native_v1;

#[derive(Debug)]
pub(crate) enum LicmNativeStageErrorV1 {
    CrossBlockForwarding(cross_block_forwarding_native_v1::CrossBlockForwardingNativeStageErrorV1),
    Resource(Resource),
    Admission(Box<LicmAdmissionError>),
    Mismatch(&'static str),
}
impl fmt::Display for LicmNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LicmNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            Self::CrossBlockForwarding(error) => Some(error),
            _ => None,
        }
    }
}
fn error(value: LicmNativeStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::LicmNativeStage(value)
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(LicmNativeStageErrorV1::Resource(value))
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    error(LicmNativeStageErrorV1::Mismatch(detail))
}
fn admission(value: LicmAdmissionError) -> ProductionPipelineError {
    error(LicmNativeStageErrorV1::Admission(Box::new(value)))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed source-bearing owner"
)]
enum Licm {
    Direct(DirectLicm),
    Erased(ErasedLicm),
}
impl Licm {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn preheader_input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn promoted_input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().prefix().output(),
            Self::Erased(v) => v.prefix().prefix().output(),
        }
    }
    fn historical_p8(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().prefix().prefix().output(),
            Self::Erased(v) => v.prefix().prefix().prefix().output(),
        }
    }
    fn history(&self) -> Admitted7Ref<'_> {
        match self {
            Self::Direct(v) => Admitted7Ref::Direct(v.prefix().prefix().prefix().prefix()),
            Self::Erased(v) => Admitted7Ref::Erased(v.prefix().prefix().prefix().prefix()),
        }
    }
    fn original(&self) -> Result<&Graph> {
        match self {
            Self::Direct(v) => v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| mismatch("original direct LICM source N")),
            Self::Erased(v) => Ok(v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()),
        }
    }
    fn header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<DirectLicm>(),
            Self::Erased(_) => size_of::<ErasedLicm>(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn continue_once(preheaders: Preheaders, budget: &mut Budget<'_>) -> Result<(Self, usize)> {
        let (owner, storage) = match preheaders {
            Preheaders::Direct(v) => {
                let (owner, storage) = v.continue_licm_v1(budget).map_err(admission)?;
                (Self::Direct(owner), storage)
            }
            Preheaders::Erased(v) => {
                let (owner, storage) = v.continue_licm_v1(budget).map_err(admission)?;
                (Self::Erased(owner), storage)
            }
        };
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        Ok((owner, storage.retained_storage()))
    }
}

/// Added storage returned unreserved, excluding retained source and siblings.
#[derive(Clone, Copy)]
pub(crate) struct LicmNativeStorageV1(usize);
impl LicmNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) struct PreparedLicmNativeOutputV1 {
    owner: Licm,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}
impl PreparedLicmNativeOutputV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.owner.output()
    }
    pub(crate) fn preheader_input(&self) -> &Graph {
        self.owner.preheader_input()
    }
    pub(crate) fn promoted_input(&self) -> &Graph {
        self.owner.promoted_input()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.owner.historical_p8()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.owner.original()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result<()> {
        scoped(self.retained_floor, budget, |budget| {
            self.owner.replay(budget)?;
            let scratch = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1;
            budget.reserve_storage(scratch).map_err(resource)?;
            self.prefix_execution
                .check_history_v1(self.owner.history(), budget)?;
            budget.release_storage(scratch).map_err(resource)?;
            check_native_text(self.output(), self.profile, &self.llvm, budget)
        })
    }
}

fn check_native_text(
    output: &Graph,
    profile: Profile,
    actual: &str,
    budget: &mut Budget<'_>,
) -> Result<()> {
    check_native_text_with_errors_v1(output, profile, actual, budget, resource, || {
        mismatch("exact LICM native LLVM")
    })
}

// The tuple transfers existing owners and receipts only. Its caller retains
// the existing scope and the original point of checked receipt summation.
fn prepare_licm_source_prefix_v1(
    prefix: Prefix6,
    budget: &mut Budget<'_>,
) -> Result<(Licm, Policy7ExecutionWitnessV1, usize, usize, usize, usize)> {
    let (history, prefix_execution, history_added) = prepare_history_v1(prefix, budget)?;
    let (promoted, promoted_added) = Promoted::continue_once(history, budget)?;
    let (preheaders, preheaders_added) = Preheaders::continue_once(promoted, budget)?;
    let (owner, licm_added) = Licm::continue_once(preheaders, budget)?;
    Ok((
        owner,
        prefix_execution,
        history_added,
        promoted_added,
        preheaders_added,
        licm_added,
    ))
}

fn prepare(
    prefix: Prefix6,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> Result<(PreparedLicmNativeOutputV1, LicmNativeStorageV1)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (owner, prefix_execution, history_added, promoted_added, preheaders_added, licm_added) =
            prepare_licm_source_prefix_v1(prefix, budget)?;
        let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
        budget.reserve_storage(native_storage).map_err(resource)?;
        // Active source owner, witness and String receipts already paid these headers.
        let header = size_of::<PreparedLicmNativeOutputV1>()
            .checked_sub(owner.header())
            .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
            .and_then(|n| n.checked_sub(size_of::<String>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(header).map_err(resource)?;
        let retained = history_added
            .checked_add(prefix_execution.retained_storage())
            .and_then(|n| n.checked_add(promoted_added))
            .and_then(|n| n.checked_add(preheaders_added))
            .and_then(|n| n.checked_add(licm_added))
            .and_then(|n| n.checked_add(native_storage))
            .and_then(|n| n.checked_add(header))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedLicmNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(budget)?;
        Ok((value, LicmNativeStorageV1(retained)))
    })
}

/// Actual source/ranked custody and final LICM native text, without wire authority.
pub(crate) struct LicmNativeProductionCompilationV1 {
    native: PreparedLicmNativeOutputV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    /// Explicit unnumbered continuation. Reserve the returned addition before
    /// further use; every exit preserves entry storage and cumulative work.
    pub(crate) fn lower_licm_native_with_budget_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<(LicmNativeProductionCompilationV1, LicmNativeStorageV1)> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let (prefix, ranked_verification, bindings) = self.prepare_native_prefix_v1(budget)?;
            let (native, storage) = prepare(prefix, bindings.rustc_target.profile(), budget)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            let wrapper = size_of::<LicmNativeProductionCompilationV1>()
                .checked_sub(size_of::<PreparedLicmNativeOutputV1>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let value = LicmNativeProductionCompilationV1 {
                native,
                ranked_verification,
                bindings,
                retained_floor: budget.storage(),
            };
            value.verify_equivalence(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or_else(|| resource(Resource::Accounting))?;
            Ok((value, LicmNativeStorageV1(retained)))
        })
    }
}
impl LicmNativeProductionCompilationV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn preheader_input(&self) -> &Graph {
        self.native.preheader_input()
    }
    pub(crate) fn promoted_input(&self) -> &Graph {
        self.native.promoted_input()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.native.historical_p8_output()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.native.llvm_ir()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.native.original()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result<()> {
        scoped(self.retained_floor, budget, |budget| {
            if self
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != self.bindings.rustc_identity_inventory.sha256()
            {
                return Err(ProductionPipelineError::RustcLineageMismatch);
            }
            if self.native.profile != self.bindings.rustc_target.profile()
                || self.ranked_verification.root_count() != self.output().module().kernels.len()
                || !self
                    .ranked_verification
                    .every_functional_verification_is_coherent()
            {
                return Err(mismatch("complete LICM target/ranked custody"));
            }
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_licm_native_v1_tests.rs"]
mod tests;

#[cfg(test)]
mod source_lane_tests {
    use super::*;
    use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
    use crate::production_pipeline::{
        AdmittedSemanticMirStage, CollectedRustStage, ProductionCompilation,
    };
    use std::marker::PhantomData;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    pub(crate) struct LicmSourceObservationV1<'a> {
        owner: &'a LicmNativeProductionCompilationV1,
    }
    impl LicmSourceObservationV1<'_> {
        pub(crate) fn unit_local(&self) -> bool {
            matches!(&self.owner.native.owner, Licm::Erased(_))
        }
        pub(crate) fn original(&self) -> Result<&Graph> {
            self.owner.original()
        }
        pub(crate) fn historical_p8(&self) -> &Graph {
            self.owner.historical_p8_output()
        }
        pub(crate) fn promoted(&self) -> &Graph {
            self.owner.promoted_input()
        }
        pub(crate) fn input(&self) -> &Graph {
            self.owner.preheader_input()
        }
        pub(crate) fn output(&self) -> &Graph {
            self.owner.output()
        }
        pub(crate) fn origins(&self) -> &[fe2o3_kernel_analysis::CanonicalKirLicmOriginV1] {
            match &self.owner.native.owner {
                Licm::Direct(v) => v.operation_origins(),
                Licm::Erased(v) => v.operation_origins(),
            }
        }
        pub(crate) fn preheaders(
            &self,
        ) -> &[fe2o3_lower_mir_kernel::ProductionLoopPreheaderOriginV1] {
            match &self.owner.native.owner {
                Licm::Direct(v) => v.prefix().origins(),
                Licm::Erased(v) => v.prefix().origins(),
            }
        }
        pub(crate) fn incoming(
            &self,
        ) -> &[fe2o3_lower_mir_kernel::ProductionLoopPreheaderIncomingOriginV1] {
            match &self.owner.native.owner {
                Licm::Direct(v) => v.prefix().incoming_origins(),
                Licm::Erased(v) => v.prefix().incoming_origins(),
            }
        }
        pub(crate) fn parameters(
            &self,
        ) -> &[fe2o3_lower_mir_kernel::ProductionLoopPreheaderParameterOriginV1] {
            match &self.owner.native.owner {
                Licm::Direct(v) => v.prefix().parameter_origins(),
                Licm::Erased(v) => v.prefix().parameter_origins(),
            }
        }
        pub(crate) fn kernels(&self) -> &[fe2o3_kernel_ir::FormalMemoryObligations] {
            match &self.owner.native.owner {
                Licm::Direct(v) => v.kernels(),
                Licm::Erased(v) => v.kernels(),
            }
        }
        pub(crate) fn deleted_helpers(&self) -> (usize, usize) {
            match &self.owner.native.owner {
                Licm::Direct(_) => (0, 0),
                Licm::Erased(v) => {
                    let source = v
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .erased_source();
                    (source.deleted_call_count(), source.deleted_function_count())
                }
            }
        }
        pub(crate) fn source_launch(
            &self,
        ) -> &fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1 {
            match &self.owner.native.owner {
                Licm::Direct(v) => v
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .source_semantic_kir()
                    .source_launch_roster()
                    .unwrap(),
                Licm::Erased(v) => v
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .original_source()
                    .source_launch(),
            }
        }
        pub(crate) fn source_semantic(
            &self,
        ) -> &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 {
            match &self.owner.native.owner {
                Licm::Direct(v) => v
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .source_semantic_kir()
                    .semantic()
                    .semantic(),
                Licm::Erased(v) => v
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .original_source()
                    .semantic_ssa()
                    .source_semantic(),
            }
        }
        pub(crate) fn execution_bytes(&self) -> &[u8] {
            self.owner.native.prefix_execution.canonical_bytes()
        }
        pub(crate) fn ranked_identity(&self) -> [u8; 32] {
            *self
                .owner
                .ranked_verification
                .canonical_roster_identity()
                .as_bytes()
        }
        pub(crate) fn source_identity(&self) -> [u8; 32] {
            self.owner.bindings.rustc_identity_inventory.sha256()
        }
        pub(crate) fn preflight_identity(&self) -> [u8; 32] {
            self.owner
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
        }
        pub(crate) fn profile(&self) -> Profile {
            self.owner.bindings.rustc_target.profile()
        }
        pub(crate) fn floor(&self) -> usize {
            self.owner.retained_floor
        }
    }

    fn restored<T>(result: std::thread::Result<T>) -> T {
        match result {
            Ok(value) => value,
            Err(payload) => resume_unwind(payload),
        }
    }

    impl LicmNativeProductionCompilationV1 {
        pub(crate) fn with_source_test_observation_v1<R>(
            &self,
            next: impl for<'a> FnOnce(LicmSourceObservationV1<'a>) -> R,
        ) -> R {
            next(LicmSourceObservationV1 { owner: self })
        }
        pub(crate) fn source_test_bad_llvm_v1(&mut self, budget: &mut Budget<'_>) -> Result<()> {
            let byte = self.native.llvm.as_bytes()[0];
            assert!(byte.is_ascii());
            self.native
                .llvm
                .replace_range(..1, if byte == b'X' { "Y" } else { "X" });
            let result = catch_unwind(AssertUnwindSafe(|| self.verify_equivalence(budget)));
            self.native
                .llvm
                .replace_range(..1, std::str::from_utf8(&[byte]).unwrap());
            let result = restored(result);
            assert!(matches!(
                &result,
                Err(ProductionPipelineError::LicmNativeStage(
                    LicmNativeStageErrorV1::Mismatch("exact LICM native LLVM")
                ))
            ));
            result
        }
        pub(crate) fn source_test_wrong_target_v1(
            &mut self,
            budget: &mut Budget<'_>,
        ) -> Result<()> {
            let original = self.native.profile;
            self.native.profile = match original {
                Profile::Gfx942 => Profile::Gfx950,
                Profile::Gfx950 => Profile::Gfx942,
            };
            let result = catch_unwind(AssertUnwindSafe(|| self.verify_equivalence(budget)));
            self.native.profile = original;
            let result = restored(result);
            assert!(matches!(
                &result,
                Err(ProductionPipelineError::LicmNativeStage(
                    LicmNativeStageErrorV1::Mismatch("complete LICM target/ranked custody")
                ))
            ));
            result
        }
        pub(crate) fn source_test_foreign_ledger_v1<'w>(
            &self,
            budget: &mut Budget<'w>,
            replacement: &mut Budget<'w>,
        ) -> Result<()> {
            let original = budget.work_ledger_identity_v1();
            assert!(original != replacement.work_ledger_identity_v1());
            assert_eq!(budget.storage(), replacement.storage());
            let result = catch_unwind(AssertUnwindSafe(|| {
                scoped(budget.storage(), budget, |budget| {
                    self.verify_equivalence(budget)?;
                    std::mem::swap(budget, replacement);
                    Ok(())
                })
            }));
            if budget.work_ledger_identity_v1() != original {
                std::mem::swap(budget, replacement);
            }
            let result = restored(result);
            assert!(matches!(
                &result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Resource(Resource::Accounting)
                ))
            ));
            result
        }
        pub(crate) fn source_test_missing_floor_v1(&self, budget: &mut Budget<'_>) -> Result<()> {
            let floor = budget.storage();
            let removed = floor.checked_sub(self.retained_floor - 1).unwrap();
            budget.release_storage(removed).unwrap();
            let work = budget.work();
            let result = catch_unwind(AssertUnwindSafe(|| self.verify_equivalence(budget)));
            budget.reserve_storage(removed).unwrap();
            let result = restored(result);
            assert_eq!((budget.storage(), budget.work()), (floor, work));
            assert!(matches!(
                &result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Resource(Resource::Accounting)
                ))
            ));
            result
        }
        pub(crate) fn source_test_consume_failure_v1(
            self,
            budget: &mut Budget<'_>,
            panics: bool,
        ) -> Result<()> {
            let result = scoped(budget.storage(), budget, move |budget| {
                self.verify_equivalence(budget)?;
                drop(self);
                if panics {
                    panic!("source native owner dropped before injected unwind");
                }
                Err(mismatch(
                    "source native owner dropped before injected error",
                ))
            });
            if panics {
                assert!(matches!(
                    &result,
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        P7Error::Panicked
                    ))
                ));
            } else {
                assert!(matches!(
                    &result,
                    Err(ProductionPipelineError::LicmNativeStage(
                        LicmNativeStageErrorV1::Mismatch(
                            "source native owner dropped before injected error"
                        )
                    ))
                ));
            }
            result
        }
    }

    pub(crate) struct SourceNativePairSlotV1 {
        owners: [Option<(LicmNativeProductionCompilationV1, LicmNativeStorageV1)>; 2],
    }

    // Move the complete authenticated import before Pliron creates non-Send owners.
    pub(crate) struct ImportedSourceStageV1 {
        stage: AdmittedSemanticMirStage,
    }

    impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
        pub(crate) fn source_test_import_stage_v1(self) -> Result<ImportedSourceStageV1> {
            fn is_send<T: Send>() {}
            is_send::<ImportedSourceStageV1>();
            Ok(ImportedSourceStageV1 {
                stage: self.import_semantic_mir()?.stage,
            })
        }
    }

    impl ImportedSourceStageV1 {
        pub(crate) fn into_ranked_v1(self) -> Result<RankedVerifiedProductionCompilation> {
            ProductionCompilation {
                stage: self.stage,
                invariant_session: PhantomData,
            }
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .materialize_target_neutral()?
            .verify_general_kernel_checks()
        }
    }

    impl RankedVerifiedProductionCompilation {
        pub(crate) fn source_test_pair_slot_v1() -> SourceNativePairSlotV1 {
            SourceNativePairSlotV1 {
                owners: [None, None],
            }
        }
    }

    impl SourceNativePairSlotV1 {
        pub(crate) fn capture_v1(
            &mut self,
            second: bool,
            source: RankedVerifiedProductionCompilation,
            budget: &mut Budget<'_>,
        ) -> Result<()> {
            let index = usize::from(second);
            assert!(self.owners[index].is_none());
            assert!(!second || self.owners[0].is_some());
            let (owner, receipt) = source.lower_licm_native_with_budget_v1(budget)?;
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(resource)?;
            self.owners[index] = Some((owner, receipt));
            self.with_owner_v1(second, |owner| {
                assert_eq!(owner.retained_storage_floor_v1(), budget.storage())
            });
            Ok(())
        }

        pub(crate) fn with_owner_v1<R>(
            &self,
            second: bool,
            next: impl for<'a> FnOnce(&'a LicmNativeProductionCompilationV1) -> R,
        ) -> R {
            next(&self.owners[usize::from(second)].as_ref().unwrap().0)
        }

        pub(crate) fn with_both_v1<R>(
            &self,
            next: impl for<'a> FnOnce(
                &'a LicmNativeProductionCompilationV1,
                &'a LicmNativeProductionCompilationV1,
            ) -> R,
        ) -> R {
            next(
                &self.owners[0].as_ref().unwrap().0,
                &self.owners[1].as_ref().unwrap().0,
            )
        }

        pub(crate) fn field_extents_v1(&self) -> [[usize; 2]; 2] {
            std::array::from_fn(|index| {
                let owner = &self.owners[index].as_ref().unwrap().0;
                let plan = size_of::<crate::collector::AuthenticatedRustcPreflightPlanV3>()
                    .checked_add(
                        owner
                            .bindings
                            .rustc_preflight_plan
                            .canonical_transcript()
                            .len(),
                    )
                    .unwrap();
                let p7 = size_of::<Policy7ExecutionWitnessV1>()
                    .checked_add(owner.native.prefix_execution.canonical_bytes().len())
                    .unwrap();
                assert_eq!(p7, owner.native.prefix_execution.retained_storage());
                [p7, plan]
            })
        }

        pub(crate) fn with_swapped_v1<R>(
            &mut self,
            plan: bool,
            next: impl for<'a> FnOnce(
                &'a LicmNativeProductionCompilationV1,
                &'a LicmNativeProductionCompilationV1,
            ) -> R,
        ) -> R {
            let [Some((left, _)), Some((right, _))] = &mut self.owners else {
                panic!("two genuine native owners required")
            };
            if plan {
                std::mem::swap(
                    &mut left.bindings.rustc_preflight_plan,
                    &mut right.bindings.rustc_preflight_plan,
                );
            } else {
                std::mem::swap(
                    &mut left.native.prefix_execution,
                    &mut right.native.prefix_execution,
                );
            }
            let result = catch_unwind(AssertUnwindSafe(|| next(left, right)));
            if plan {
                std::mem::swap(
                    &mut left.bindings.rustc_preflight_plan,
                    &mut right.bindings.rustc_preflight_plan,
                );
            } else {
                std::mem::swap(
                    &mut left.native.prefix_execution,
                    &mut right.native.prefix_execution,
                );
            }
            restored(result)
        }

        pub(crate) fn finish_v1(
            mut self,
            first: &mut Budget<'_>,
            second: &mut Budget<'_>,
        ) -> Result<()> {
            fn finish_owner(
                value: &mut Option<(LicmNativeProductionCompilationV1, LicmNativeStorageV1)>,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                if let Some((owner, receipt)) = value.take() {
                    drop(owner);
                    budget
                        .release_storage(receipt.retained_storage())
                        .map_err(resource)?;
                }
                Ok(())
            }
            let first = finish_owner(&mut self.owners[0], first);
            let second = finish_owner(&mut self.owners[1], second);
            first.and(second)
        }
    }

    impl ProductionPipelineError {
        pub(crate) fn source_test_assert_pair_refusal_v1(&self, plan: bool, unequal_extent: bool) {
            if plan {
                assert!(matches!(
                    self,
                    ProductionPipelineError::RustcLineageMismatch
                ));
            } else {
                let expected = if unequal_extent {
                    "Policy7 record extent"
                } else {
                    "Policy7 complete execution transcript"
                };
                assert!(
                    matches!(self, ProductionPipelineError::CheckedOutputPolicy7Stage(P7Error::Execution(detail)) if *detail == expected)
                );
            }
        }

        pub(crate) fn source_test_assert_licm_work_denial_v1(&self, actual: usize, limit: usize) {
            assert!(matches!(self, ProductionPipelineError::LicmNativeStage(
                LicmNativeStageErrorV1::Resource(Resource::Work(error))
            ) if error.actual() == actual && error.limit() == limit));
        }
    }
}
