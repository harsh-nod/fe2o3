//! Typed frame accounting for retained complete-CFG V1 analysis.
use super::*;
use crate::semantic_mir_v1::SemanticControlFlowEdgeV1;

pub(super) const RETAINED_FRAME_ROWS: usize = 33;
pub(super) fn retained_frame_rows<M: SemanticU32InductionBoundSnapshotMeterV1>()
-> [usize; RETAINED_FRAME_ROWS] {
    [
        size_of::<RetainedSemanticU32InductionV1<'static, M::Error>>(), // owner
        size_of::<(
            Payload,
            Phase,
            Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
            Option<Saved<M::Error>>,
            usize,
        )>(), // constructor
        size_of::<(
            SemanticCfgV1,
            SemanticInventoryV1,
            Vec<Reachability>,
            Vec<SemanticU32InductionNoOverflowCertificateV1>,
            Option<SemanticU32InductionNoOverflowReportV1>,
        )>(), // payload constructor
        size_of::<(
            &mut RetainedSemanticU32InductionV1<'static, M::Error>,
            &AdmittedInertSemanticMirV1,
            SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            &mut M,
            bool,
        )>(), // entry arguments
        size_of::<(
            Adapter<'_, M>,
            WorkLease<'static, 'static>,
            &mut WorkBudgetV1<'static>,
            &mut usize,
            &mut Option<Saved<M::Error>>,
        )>(), // adapter and guard
        size_of::<(
            &mut Payload,
            &mut Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
            &mut WorkBudgetV1<'static>,
            &AdmittedInertSemanticMirV1,
            SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            usize,
        )>(), // entry split borrows/capture
        size_of::<(
            Option<&SemanticFunctionDeclV1>,
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            InertSemanticMirSha256V1,
            Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
        )>(), // source loan/result
        size_of::<(
            AnalysisResult<()>,
            AnalysisResult<()>,
            Result<(), SemanticU32InductionRetainedFailureV1>,
            Result<(), SemanticU32InductionRetainedFailureV1>,
            SemanticU32InductionRetainedFailureV1,
            Saved<M::Error>,
            M::Error,
            Option<Saved<M::Error>>,
        )>(), // entry outcomes/errors
        size_of::<(
            &mut Adapter<'_, M>,
            usize,
            bool,
            Result<(), M::Error>,
            AnalysisResult<()>,
            M::Error,
            &mut M,
            &mut Option<Saved<M::Error>>,
        )>(), // meter forwarding
        size_of::<(&mut WorkLease<'static, 'static>, &mut usize, usize)>(), // work guard drop
        size_of::<(
            &RetainedSemanticU32InductionV1<'static, M::Error>,
            &AdmittedInertSemanticMirV1,
            SemanticFunctionIdV1,
            Option<(&AdmittedInertSemanticMirV1, SemanticFunctionIdV1)>,
            Result<&SemanticU32InductionNoOverflowReportV1, SemanticU32InductionRetainedFailureV1>,
            Option<&Saved<M::Error>>,
            Option<&SemanticU32InductionNoOverflowReportV1>,
            (&AdmittedInertSemanticMirV1, SemanticFunctionIdV1),
            bool,
            usize,
        )>(), // completed/error/work getters
        size_of::<(
            &mut Payload,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            InertSemanticMirSha256V1,
            SemanticFunctionIdV1,
            SemanticU32InductionAnalysisLimitsV1,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
        )>(), // payload entry
        size_of::<(
            &SemanticFunctionDeclV1,
            &mut SemanticCfgV1,
            &mut Vec<Reachability>,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut Vec<Vec<usize>>,
            &mut Vec<Vec<usize>>,
        )>(), // graph invocation
        size_of::<(
            &mut Vec<Vec<usize>>,
            &mut Vec<Vec<usize>>,
            &mut WorkBudgetV1<'static>,
            usize,
            usize,
        )>(), // edge closure captures
        size_of::<(
            SemanticControlFlowEdgeV1,
            SemanticControlFlowEdgeV1,
            SemanticBlockIdV1,
            SemanticBlockIdV1,
            u32,
            usize,
            AnalysisResult<()>,
        )>(), // edge by-value vertices
        size_of::<(
            &SemanticCfgV1,
            &[bool],
            &Vec<bool>,
            std::slice::Iter<'static, bool>,
            &bool,
            usize,
            usize,
            &SemanticBasicBlockV1,
            bool,
        )>(), // graph/read traversal aliases
        size_of::<(
            &SemanticFunctionDeclV1,
            &SemanticCfgV1,
            &mut SemanticInventoryV1,
            &mut WorkBudgetV1<'static>,
            &mut Vec<DefinitionSummaryV1>,
            &mut Vec<bool>,
            &mut Vec<bool>,
            &mut Vec<usize>,
            &mut Vec<CandidateSiteV1>,
            AnalysisResult<()>,
        )>(), // inventory invocation and fields
        size_of::<(
            &mut [DefinitionSummaryV1],
            &mut [bool],
            &mut [bool],
            &mut [usize],
            &[CandidateSiteV1],
            &Vec<CandidateSiteV1>,
        )>(), // inventory coerced slices
        size_of::<(
            &CandidateProofContextV1<'static>,
            CandidateSiteV1,
            BoundResolverV1<'static>,
            &mut WorkBudgetV1<'static>,
            &mut Vec<Reachability>,
            AnalysisResult<Option<ProvedCandidateV1>>,
            AnalysisResult<Option<ProvedCandidateV1>>,
            ProvedCandidateV1,
            SemanticU32InductionNoOverflowCertificateV1,
        )>(), // candidate retained bridge
        size_of::<(
            CandidateProofContextV1<'static>,
            std::slice::Iter<'static, CandidateSiteV1>,
            &CandidateSiteV1,
            usize,
            usize,
            &SemanticInventoryV1,
            &SemanticCfgV1,
            &mut Vec<SemanticU32InductionNoOverflowCertificateV1>,
        )>(), // candidate iteration
        size_of::<(
            &SemanticCfgV1,
            &mut Vec<Reachability>,
            usize,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<bool>,
            AnalysisResult<&[bool]>,
            &[bool],
            bool,
        )>(), // dominance retained bridge
        size_of::<(
            &SemanticCfgV1,
            Option<usize>,
            &mut Vec<Reachability>,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<&[bool]>,
            Reachability,
            &mut Reachability,
            &mut Vec<bool>,
            &mut Vec<usize>,
        )>(), // reachability entry/capture
        size_of::<(
            Option<usize>,
            usize,
            std::slice::Iter<'static, usize>,
            &usize,
            &Vec<usize>,
            &[usize],
            bool,
            AnalysisResult<()>,
        )>(), // reachability walk
        size_of::<(
            &mut Vec<DefinitionSummaryV1>,
            usize,
            DefinitionSummaryV1,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut [DefinitionSummaryV1],
        )>(), // filled definition instantiation
        size_of::<(
            &mut Vec<bool>,
            usize,
            bool,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut [bool],
        )>(), // filled bool instantiation
        size_of::<(
            &mut Vec<usize>,
            usize,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            &mut [usize],
        )>(), // filled usize instantiation
        size_of::<(
            &mut Vec<Vec<usize>>,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            Vec<usize>,
            &mut [Vec<usize>],
        )>(), // nested usize instantiation
        size_of::<(
            &mut Vec<Reachability>,
            usize,
            &mut WorkBudgetV1<'static>,
            AnalysisResult<()>,
            Reachability,
            Option<&mut Reachability>,
            &mut Reachability,
        )>(), // retained reach header allocation
        size_of::<(
            &Vec<SemanticU32InductionNoOverflowCertificateV1>,
            &mut WorkBudgetV1<'static>,
            usize,
            usize,
            Option<usize>,
            AnalysisResult<()>,
            bool,
        )>(), // retained final admission
        size_of::<(
            Vec<SemanticU32InductionNoOverflowCertificateV1>,
            Box<[SemanticU32InductionNoOverflowCertificateV1]>,
            SemanticU32InductionNoOverflowReportV1,
            Option<SemanticU32InductionNoOverflowReportV1>,
            &mut Vec<SemanticU32InductionNoOverflowCertificateV1>,
            usize,
            SemanticFunctionIdentityV1,
        )>(), // report ownership commit
        size_of::<(usize, usize, Option<usize>, Option<usize>, &usize)>(), // frame arithmetic/results
        size_of::<(
            [usize; RETAINED_FRAME_ROWS],
            [usize; RETAINED_FRAME_ROWS],
            std::array::IntoIter<usize, RETAINED_FRAME_ROWS>,
            Option<usize>,
            usize,
            usize,
        )>(), // retained frame array/result/fold
        size_of::<(
            &mut WorkBudgetV1<'static>,
            &mut Vec<Reachability>,
            usize,
            usize,
            usize,
            usize,
            Option<usize>,
            Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
            AnalysisResult<()>,
            bool,
        )>(), // new Reachability strict-reserve instantiation
    ]
}
pub(super) fn retained_frame<M: SemanticU32InductionBoundSnapshotMeterV1>() -> Option<usize> {
    // The unchanged strict donor frame remains separate, never residual funding.
    let original = strict_resources::frame_storage_v1::<M>()?;
    retained_frame_rows::<M>()
        .into_iter()
        .try_fold(original, usize::checked_add)
}
