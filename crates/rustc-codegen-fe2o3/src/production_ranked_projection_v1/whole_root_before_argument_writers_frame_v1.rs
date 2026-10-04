//! Explicit logical reached-carrier envelope for the new whole-root glue.
//! Existing components retain their independent frame/reservation policies.
use super::*;
use std::mem::size_of;
type Facts = CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>;
type Whole = PendingWholeRootBeforeArgumentWritersV1<'static>;
type Borrow = scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'static>;
type Authenticated = crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1<'static>;
type Site =
    crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::NominalCallerSiteV1<
        'static,
    >;
const ROWS: usize = 33;
pub(super) fn frame() -> BResult<usize> {
    let rows = [
        // Physical owner, all fixed/owned headers, and initial transfers.
        size_of::<Whole>(),
        size_of::<(WholePhase, Option<&ProductionPreRankedKirOwnerV1>,
            Option<SemanticFunctionIdV1>, Option<Snapshot>, Option<Backend>,
            ProductionSemanticSharedReadsPreparationV1<'static>, RetainedScalarSingletonV1,
            RetainedScalarBorrowsV1<'static>, RetainedLazyScopePrefixV1,
            RetainedSemanticU32InductionV1<'static, Resource>, Option<ActualSelectedInputsV1<'static>>,
            RetainedConstantLocalsV1, RootEntryPrefixV1, Option<&'static str>,
            RetainedProjectedViewPrefixV1<'static>, String, RetainedSliceScopePrefixV1,
            PendingBeforeCapabilitiesV1, RetainedNominalCapabilityDriverV1,
            [usize; 3], RetainedBeforeArgumentWritersV1)>(),
        // Sealed entry/source enrollment and one-shot result transfer.
        size_of::<(&mut Whole, &ProductionPreRankedKirOwnerV1,
            &CheckedBf16NominalCallV1<'static>, &ActualRetainedRankedInputsV1<'static>,
            &mut Facts, &mut usize, bool, BResult<()>, Result<()>, Backend,
            QueryError, Option<QueryError>, Option<&Backend>, &Backend, fn(&Backend)->QueryError)>(),
        size_of::<(SemanticFunctionIdV1, BResult<SemanticFunctionIdV1>, &ProductionPreRankedKirOwnerV1,
            &fe2o3_pliron::ProductionSemanticSsaOwnerV1, &AdmittedInertSemanticMirV1,
            &[SemanticFunctionDeclV1], Option<&SemanticFunctionDeclV1>,
            &SemanticFunctionDeclV1, BResult<&SemanticFunctionDeclV1>,
            &[SemanticTypeDeclV1], &[SemanticCallableDeclV1],
            &[fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1],
            &[fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1],
            fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1,
            SemanticFunctionRoleV1, usize, u32, bool)>(),
        // Snapshot/counter and original Shared backend work/ledger prefix.
        size_of::<(Snapshot, Option<Snapshot>, BResult<Snapshot>, &mut Facts,
            &mut usize, usize, usize, Option<usize>, BResult<usize>,
            Ledger, BResult<Ledger>, &mut ProductionSemanticSharedReadsPreparationV1<'static>,
            &ProductionSemanticSharedReadsPreparationV1<'static>,
            &fe2o3_pliron::ProductionSemanticSsaOwnerV1, SemanticFunctionIdV1)>(),
        size_of::<(Option<&ProductionSemanticSharedReadsV1<'static>>,
            &ProductionSemanticSharedReadsV1<'static>,
            BResult<&ProductionSemanticSharedReadsV1<'static>>, Source<'static>,
            &Source<'static>, SourceIdentity)>(),
        // Source-preparation captures are held in the outer owner, never returned
        // as authority; the existing helper also charges its concrete F/R frames.
        size_of::<(&mut RetainedScalarSingletonV1, &mut RetainedScalarBorrowsV1<'static>,
            &AdmittedInertSemanticMirV1, &SemanticFunctionDeclV1,
            &mut Prep<'static, 'static>, BResult<()>)>(),
        size_of::<(&mut RetainedLazyScopePrefixV1, &mut dyn ProjectedAssertionFactsV1,
            BResult<()>)>(),
        // Model adapter/source binding and preserved first owning failure.
        size_of::<(InductionMeter<'static, 'static, 'static>, &mut InductionMeter<'static, 'static, 'static>,
            &mut RetainedSemanticU32InductionV1<'static, Resource>,
            &RetainedSemanticU32InductionV1<'static, Resource>,
            &AdmittedInertSemanticMirV1, SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            std::result::Result<(), fe2o3_mir_model::SemanticU32InductionRetainedFailureV1>,
            fe2o3_mir_model::SemanticU32InductionRetainedFailureV1,
            std::result::Result<&SemanticU32InductionNoOverflowReportV1,
                fe2o3_mir_model::SemanticU32InductionRetainedFailureV1>,
            BResult<&SemanticU32InductionNoOverflowReportV1>)>(),
        size_of::<(Option<&SemanticU32InductionMeteredErrorV1<Resource>>,
            &SemanticU32InductionMeteredErrorV1<Resource>, SemanticU32InductionMeteredErrorV1<Resource>,
            &fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1,
            fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1,
            &Resource, Resource, &mut Prep<'static, 'static>, usize,
            BResult<()>, std::result::Result<(), Resource>, Backend)>(),
        // Actual selection, constants and the original root prefix owners.
        size_of::<(ActualSelectedInputsV1<'static>, Option<ActualSelectedInputsV1<'static>>,
            BResult<ActualSelectedInputsV1<'static>>, &ActualSelectedInputsV1<'static>,
            Option<&ActualSelectedInputsV1<'static>>, BResult<&ActualSelectedInputsV1<'static>>,
            ProductionSourceLaunchRootV1, &ProductionRankedRootInputV1,
            &[crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1],
            &mut RetainedConstantLocalsV1, &mut RootEntryPrefixV1,
            &mut RetainedProjectedViewPrefixV1<'static>, BResult<()>)>(),
        // Census loans include real Some(empty Shared); no None stand-in.
        size_of::<(&RetainedScalarSingletonV1, &RetainedScalarBorrowsV1<'static>,
            &[u8], BResult<&[u8]>, Option<&Borrow>, BResult<Option<&Borrow>>,
            (&[u8], Option<&Borrow>), BResult<(&[u8], Option<&Borrow>)>)>(),
        // The concrete lexical F reference stays inside the HRTB unit callback.
        size_of::<(ProjectedViewPrefixLoanV1<'static, 'static, Facts>,
            BResult<ProjectedViewPrefixLoanV1<'static, 'static, Facts>>,
            &mut ProjectedViewPrefixLoanV1<'static, 'static, Facts>,
            &mut Facts, usize, usize, Option<usize>, BResult<()>)>(),
        // Complete concrete callback captures; HRTB alone is not ownership proof.
        size_of::<(&mut RetainedSliceScopePrefixV1, &mut PendingBeforeCapabilitiesV1,
            &RetainedConstantLocalsV1, &mut RetainedNominalCapabilityDriverV1,
            &mut [usize; 3], &mut RetainedBeforeArgumentWritersV1, &Source<'static>,
            &ProductionPreRankedKirOwnerV1, &CheckedBf16NominalCallV1<'static>,
            &mut usize, &SemanticFunctionDeclV1, &mut Facts, BResult<()>)>(),
        // Earlier genuine DATA loans remain tied to physical earlier owners.
        size_of::<(BeforeCapabilitiesV1<'static>, BResult<BeforeCapabilitiesV1<'static>>,
            &[Option<u64>], BResult<&[Option<u64>]>,
            (&[Option<u64>], BeforeCapabilitiesV1<'static>),
            BResult<(&[Option<u64>], BeforeCapabilitiesV1<'static>)>,
            &BeforeCapabilitiesV1<'static>, &SemanticEnumPayloadDominanceV1,
            &[Option<AllocationContractV1>], NominalCapabilityInputsV1<'static>,
            &NominalCapabilityInputsV1<'static>)>(),
        // Fixed actual query visitor, not an externally supplied callback.
        size_of::<(&Site, &mut dyn NominalCapabilityConsumerV1, &mut usize,
            &mut RetainedNominalCapabilityDriverV1, &NominalCapabilityInputsV1<'static>,
            &mut NominalCapabilityVisitV1<'static>, NominalCapabilityPassV1,
            &Authenticated, &mut Budget<'static>, &mut [usize; 3], usize,
            &ProductionPreRankedKirOwnerV1, &CheckedBf16NominalCallV1<'static>,
            SemanticFunctionIdV1, Result<()>, BResult<()>)>(),
        size_of::<(crate::production_ranked_projection_v1::bf16_nominal_dense_v1::CompletedNominalFifoDriverV1<'static>,
            Result<crate::production_ranked_projection_v1::bf16_nominal_dense_v1::CompletedNominalFifoDriverV1<'static>>,
            [u8; 4], &crate::production_ranked_projection_v1::bf16_nominal_call_projection_v1::CheckedNominalCallProjectionV1<'static>,
            &fe2o3_lower_mir_kernel::Bf16CallInstanceEmissionViewV1<'static>,
            &CheckedBf16NominalCallV1<'static>, &SemanticDirectCallV1,
            &Site, SemanticFunctionIdV1, Option<usize>, bool)>(),
        // Completion/source/counter checks. The snapshot never grants readiness.
        size_of::<(&Whole, &ProductionPreRankedKirOwnerV1, SemanticFunctionIdV1,
            &mut Facts, &mut usize, Snapshot, usize, Option<usize>, Ledger,
            Option<Snapshot>, bool, BResult<()>)>(),
        size_of::<(&RetainedProjectedViewPrefixV1<'static>, &RetainedSliceScopePrefixV1,
            &RetainedBeforeArgumentWritersV1, &RetainedLazyScopePrefixV1,
            &AdmittedInertSemanticMirV1, &SemanticFunctionDeclV1,
            &mut Prep<'static, 'static>, &[SemanticTypeDeclV1],
            fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1, usize)>(),
        // Original final pre-writer destination constructors and partial states.
        size_of::<(RetainedBeforeArgumentWritersV1, bool, bool, usize,
            Vec<(usize, usize)>, Vec<Option<u32>>, Vec<Option<u32>>, usize)>(),
        size_of::<(&mut RetainedBeforeArgumentWritersV1, &RetainedBeforeArgumentWritersV1,
            usize, &mut Prep<'static, 'static>, &mut Vec<Option<u32>>,
            &Vec<Option<u32>>, &Vec<(usize, usize)>, Option<u32>,
            usize, usize, Option<usize>, BResult<()>, bool)>(),
        size_of::<(std::slice::Iter<'static, Option<u32>>, &Option<u32>,
            &mut dyn FnMut(&Option<u32>) -> bool,
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError, BResult<()>)>(),
        // Error maps, pre-map/mapped results and option predicate captures.
        size_of::<(Backend, &Backend, QueryError, Option<Backend>, &mut Option<Backend>,
            CanonicalAssertionErrorV1, Resource, &Resource,
            BResult<()>, Result<()>, BResult<usize>, Option<usize>,
            &mut dyn FnMut(&ProductionPreRankedKirOwnerV1) -> bool)>(),
        size_of::<(&mut dyn FnMut() -> BResult<()>,
            &mut dyn FnMut(&mut Prep<'static, 'static>) -> BResult<()>,
            &mut dyn FnMut(&mut Facts) -> BResult<()>)>(),
        // Caller pair and source pointer wrappers reached before frame admission.
        size_of::<(&mut Whole, &mut usize, Option<&ProductionPreRankedKirOwnerV1>,
            &ProductionPreRankedKirOwnerV1, &ActualRetainedRankedInputsV1<'static>,
            &CheckedBf16NominalCallV1<'static>, SemanticFunctionIdV1,
            &mut Facts, bool, Option<Snapshot>, BResult<Snapshot>)>(),
        // Pure typed policy fold and checked arithmetic.
        size_of::<([usize; ROWS], std::array::IntoIter<usize, ROWS>,
            usize, usize, Option<usize>, Resource, BResult<usize>)>(),
        // Existing source-chain policy; no old component entry/refund is invoked.
        allocation_frame::<(), ()>()?,
        // Read-only Original Prep/counter carrier closure.
        retained_custody_snapshot_frame_v1(),
        canonical_assertion_facts_v1::retained_whole_root_snapshot_frame_v1()?,
        // Physically distinct lazy and slice envelope policies, admitted here.
        retained_lazy_prefix_frame_v1(),
        retained_slice_prefix_frame_v1()?,
        // Concrete underlying checked-source/reference and frame receivers.
        size_of::<(&Facts, &mut Facts, &mut usize, &mut Resource,
            &ProductionSemanticSharedReadsV1<'static>, &Borrow, &[u8])>(),
        size_of::<(&mut String, &String, Option<&'static str>, &Option<&'static str>,
            &[Option<ProjectedViewV1>], &RootEntryPrefixV1, &ActualSelectedInputsV1<'static>)>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row).ok_or_else(arithmetic)
    })
}
