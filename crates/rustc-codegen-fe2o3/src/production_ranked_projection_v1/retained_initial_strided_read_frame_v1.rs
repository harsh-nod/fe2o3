//! Additive typed policy for the retained strided-read stage. No refunds.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1;
type LedgerId = fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
pub(super) fn bytes() -> BResult<usize> {
    const ROWS: usize = 15;
    let rows = [
        // Outer owner and its entry/error/completion carriers.
        size_of::<(RetainedInitialStridedReadV1, ReadPhase, ReadSourceKey,
            Option<ReadSourceKey>, Option<Snapshot>, Option<Backend>, BResult<()>)>(),
        // Producer inputs and physically retained writer destinations.
        size_of::<(&mut RetainedInitialStridedReadV1, &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1, &[Option<ProjectedReadViewAccessV1>],
            &[Option<u32>], &mut RetainedBeforeArgumentWritersV1,
            &mut RootEntryPrefixV1, &mut Prep<'static, 'static>)>(),
        // Source-block scan and the copied source descriptors.
        size_of::<(std::iter::Zip<std::slice::Iter<'static, SemanticBasicBlockV1>,
            std::iter::Copied<std::slice::Iter<'static, Option<ProjectedReadViewAccessV1>>>>,
            &SemanticBasicBlockV1, Option<ProjectedReadViewAccessV1>,
            ProjectedReadViewAccessV1, ProjectedReadViewV1, AllocationContractV1)>(),
        // Lookup-only map replacement: one charged equality for each visit.
        size_of::<(std::iter::Enumerate<std::slice::Iter<'static, ReadViewRow>>,
            &ReadViewRow, u64, usize, Option<usize>, bool, BResult<Option<usize>>)>() ,
        // New view construction and operation clone destinations.
        size_of::<(ReadViewRow, Option<ReadViewRow>, ProjectedViewV1,
            ProductionRankedValueIdV1, u32, usize, ProductionRankedValueV1,
            ProductionRankedValueV1, &mut ReadViewRow, &mut ProductionRankedOperationV1,
            &mut Vec<u64>, &mut Vec<ProductionRankedValueV1>)>(),
        // Attached access and both nested element append calls.
        size_of::<(GuardedRankedAccessV1, Option<GuardedRankedAccessV1>,
            &mut GuardedRankedAccessV1, SemanticSourceProvenanceV1,
            AccessKindAttr, MemorySpaceAttr,
            (ProductionRankedValueV1, ProductionRankedValueV1))>(),
        // Scalar helper and unchanged next-value/type-width helper carriers.
        size_of::<(ProjectedReadValueV1, SemanticLocalIdV1, SemanticTypeIdV1,
            usize, Option<u32>, &mut Option<u32>, u32, Option<usize>,
            u64, Option<u64>, BResult<u32>, BResult<ProductionRankedValueV1>,
            BResult<ProductionRankedValueIdV1>, std::num::TryFromIntError)>(),
        // Nested vector initialization/cloning iterators; no Clone heap call.
        size_of::<(std::iter::Copied<std::slice::Iter<'static, u64>>,
            std::iter::Copied<std::slice::Iter<'static, ProductionRankedValueV1>>,
            &mut Vec<ReadViewRow>, &mut Vec<Option<GuardedRankedAccessV1>>,
            &mut Vec<ProductionRankedOperationV1>,
            &mut Vec<(ProductionRankedValueV1, ProductionRankedValueV1)>,
            std::collections::TryReserveError)>(),
        // Identity/custody checks, saved-error mapping and checked additions.
        size_of::<(Snapshot, Snapshot, Option<Snapshot>, LedgerId,
            usize, usize, usize, Option<usize>, bool, Resource, Backend,
            &Backend, BResult<()>, BResult<usize>)>(),
        // Real-facts bridge variables. Existing source/capability helpers retain
        // their own frame admission; this is the additional lexical carrier.
        size_of::<(&mut PendingWholeRootBeforeArgumentWritersV1<'static>,
            &ProductionPreRankedKirOwnerV1, SemanticFunctionIdV1,
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &mut usize, Source<'static>, NominalCapabilityInputsV1<'static>,
            QueryError, Result<()>, BResult<()>)>(),
        // Retained capability DATA loan, with () as the only callback result.
        size_of::<(&RetainedNominalCapabilityDriverV1, &NominalCapabilityInputsV1<'static>,
            Option<&[Option<ProjectedReadViewAccessV1>]>,
            crate::production_ranked_projection_v1::bf16_nominal_dense_v1::CompletedNominalFifoDriverV1<'static>,
            std::result::Result<(), QueryError>)>(),
        // Sequential canonical-facts/source-preparation callbacks retain their
        // own generic frame; these are the additional paired input carriers.
        size_of::<(&BeforeCapabilitiesV1<'static>, BeforeCapabilitiesV1<'static>,
            &[Option<u64>], &[Option<u32>], &[Option<ProjectedReadViewAccessV1>],
            Option<&[Option<ProjectedReadViewAccessV1>]>, &RetainedBeforeArgumentWritersV1,
            &RootEntryPrefixV1, &RetainedInitialStridedReadV1)>(),
        // Reserve/push policy scalar carriers and checked capacity arithmetic.
        size_of::<(&mut Prep<'static, 'static>, usize, usize, Option<usize>,
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError, BResult<()>, BResult<usize>)>(),
        retained_custody_snapshot_frame_v1(),
        // Fixed policy fold and exact typed array itself.
        size_of::<([usize; ROWS], std::array::IntoIter<usize, ROWS>,
            usize, usize, Option<usize>, BResult<usize>)>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row).ok_or_else(arithmetic)
    })
}
