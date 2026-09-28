//! V18 entry points share the legacy traversal, never a legacy graph conversion.
use super::*;
use fe2o3_kernel_ir::{
    ExecutionOperationV15 as Execution, OperationKind as Op, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection,
};

/// Builds complete inert coverage of the original storage-aware graph and layout
/// table. Uses the same bounded, transferred-storage contract as the V1 builder.
/// No initialization, alias, source-layout or transformation proof is produced.
pub fn build_canonical_ranked_candidate_v18<'i, 'g, 'm>(
    inventory: &'i Inventory<'g, StorageOwner>,
    metadata: &'i CanonicalRankedMetadataV18<'g, 'm>,
    budget: &mut Budget<'_>,
) -> Result<(
    CanonicalRankedCandidateV18<'i, 'g, 'm>,
    CanonicalRankedCandidateStorageV1,
)> {
    build::build_candidate(
        inventory,
        metadata,
        inventory.owner().module(),
        operation,
        budget,
    )
}

/// Checks coverage against the original V18 owner, including each layout row,
/// before exposing metered, nonescaping queries. All obligations remain pending.
/// The V1 scope's prepaid-input, first-failure and cleanup contracts apply.
///
/// Query already prepaid inputs without converting their owner:
/// ```no_run
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV18, CanonicalRankedMetadataV18,
///     CanonicalRankedCandidateV18, CanonicalRankedViewErrorV1,
///     with_checked_canonical_ranked_view_v18};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn inspect<'i, 'g, 'm>(inventory: &'i CanonicalKirInventoryV18<'g>,
///     metadata: &'i CanonicalRankedMetadataV18<'g, 'm>,
///     candidate: &'i CanonicalRankedCandidateV18<'i, 'g, 'm>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<usize, CanonicalRankedViewErrorV1> {
///     with_checked_canonical_ranked_view_v18(inventory, metadata, candidate,
///         budget, |view, budget| view.row_count(budget))
/// }
/// ```
///
/// A V18 candidate cannot be passed to a legacy consumer:
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalRankedCandidateV1, CanonicalRankedCandidateV18};
/// fn legacy(_: &CanonicalRankedCandidateV1<'_, '_, '_>) {}
/// fn reject(candidate: &CanonicalRankedCandidateV18<'_, '_, '_>) { legacy(candidate); }
/// ```
///
/// The checked view cannot escape its scope:
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV18, CanonicalRankedMetadataV18,
///     CanonicalRankedCandidateV18, CanonicalRankedViewErrorV1,
///     with_checked_canonical_ranked_view_v18};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'i, 'g, 'm>(inventory: &'i CanonicalKirInventoryV18<'g>,
///     metadata: &'i CanonicalRankedMetadataV18<'g, 'm>,
///     candidate: &'i CanonicalRankedCandidateV18<'i, 'g, 'm>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_checked_canonical_ranked_view_v18(inventory, metadata, candidate,
///         budget, |view, _| Ok::<_, CanonicalRankedViewErrorV1>(view));
/// }
/// ```
pub fn with_checked_canonical_ranked_view_v18<'i, 'g, 'm, 'work, T, E>(
    inventory: &'i Inventory<'g, StorageOwner>,
    metadata: &'i CanonicalRankedMetadataV18<'g, 'm>,
    candidate: &'i CanonicalRankedCandidateV18<'i, 'g, 'm>,
    budget: &mut Budget<'work>,
    run: impl for<'scope> FnOnce(
        &mut CheckedCanonicalRankedViewV18<'scope, 'i, 'g, 'm>,
        &mut Budget<'work>,
    ) -> std::result::Result<T, E>,
) -> std::result::Result<T, E>
where
    E: From<Error>,
{
    check::with_checked_view(
        inventory,
        metadata,
        candidate,
        inventory.owner().module(),
        operation,
        budget,
        run,
    )
}

impl<'g> CheckedCanonicalRankedViewV18<'_, '_, 'g, '_> {
    /// Borrows an original layout row, without copying it or proving source layout
    /// correspondence, initialization, provenance or active-variant validity.
    pub fn storage_layout(
        &mut self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&'g fe2o3_kernel_ir::StorageLayoutV1> {
        self.accounting.charge(budget, 1)?;
        self.candidate
            .inventory
            .owner()
            .module()
            .storage_layouts
            .get(ordinal)
            .ok_or_else(|| {
                self.accounting
                    .fail(Error::InvalidCoordinate(Subject::StorageLayout(ordinal)))
            })
    }
}

pub(super) fn layout_obligations() -> Obligations {
    Obligations::NONE
        .with(Obligation::ReferenceRefinement)
        .with(Obligation::Provenance)
        .with(Obligation::Bounds)
        .with(Obligation::Initialization)
}

fn operation(ordinal: usize, op: &Op) -> Result<(OperationClass, Obligations)> {
    use Obligation as O;
    use OperationClass as C;
    let memory = layout_obligations()
        .with(O::ExactScalarSemantics)
        .with(O::Lifetime)
        .with(O::TrapBehavior);
    let lifecycle = Obligations::NONE
        .with(O::Lifetime)
        .with(O::Ordering)
        .with(O::Contract)
        .with(O::Launch)
        .with(O::ReferenceRefinement);
    let result = match op {
        Op::Storage(storage) => match storage {
            Storage::Project { step, .. } => {
                let obligations = match step {
                    Projection::Field(_)
                    | Projection::ArrayIndex(_)
                    | Projection::VariantForWrite { .. } => memory,
                    Projection::Variant { access, .. } => {
                        ordered_access(memory.with(O::RaceFreedom), access.volatile)
                    }
                };
                (C::StorageProject, obligations)
            }
            Storage::ReadValue { access, .. } => (
                C::StorageRead,
                ordered_access(memory.with(O::RaceFreedom), access.volatile),
            ),
            Storage::ReadDiscriminant { .. } => (
                C::StorageReadDiscriminant,
                memory.with(O::RaceFreedom).with(O::Ordering),
            ),
            Storage::WriteValue { access, .. } => (
                C::StorageWrite,
                ordered_access(memory.with(O::RaceFreedom), access.volatile),
            ),
            Storage::CopyObject {
                source_access,
                destination_access,
                ..
            } => (
                C::StorageCopy,
                ordered_access(
                    memory.with(O::RaceFreedom),
                    source_access.volatile || destination_access.volatile,
                ),
            ),
            Storage::SetDiscriminant { .. } => (
                C::StorageSetDiscriminant,
                memory.with(O::RaceFreedom).with(O::Ordering),
            ),
        },
        Op::Execution(execution) => match execution {
            Execution::ContextIssue => (C::ExecutionContext, lifecycle),
            Execution::WorkgroupDerive { .. } => (C::ExecutionWorkgroup, lifecycle),
            Execution::ScopeEnd { .. } => (C::ExecutionScopeEnd, lifecycle.with(O::Convergence)),
            Execution::MaskedTileLoadU32 { .. } => (
                C::ExecutionTileLoad,
                memory
                    .with(O::RaceFreedom)
                    .with(O::Ordering)
                    .with(O::Launch)
                    .with(O::Contract)
                    .with(O::Convergence)
                    .with(O::Tensor),
            ),
            Execution::TileIntoFragmentU32 { .. } => (
                C::ExecutionTileIntoFragment,
                lifecycle.with(O::Tensor).with(O::ExactScalarSemantics),
            ),
            Execution::FragmentIntoPartsU32 { .. } => (
                C::ExecutionFragmentIntoParts,
                lifecycle.with(O::Tensor).with(O::ExactScalarSemantics),
            ),
        },
        Op::Gfx942OrderedRegion(_) | Op::Gfx942OrderedProgram(_) => (
            if matches!(op, Op::Gfx942OrderedRegion(_)) {
                C::OrderedRegion
            } else {
                C::OrderedProgram
            },
            memory
                .with(O::RaceFreedom)
                .with(O::Ordering)
                .with(O::Convergence)
                .with(O::Target)
                .with(O::Tensor)
                .with(O::Assembly)
                .with(O::Contract),
        ),
        // The legacy classifier is exhaustive and rejects unsupported families;
        // delegating does not grant a default pure or obligation-free category.
        other => return effects::operation(ordinal, other),
    };
    Ok(result)
}

fn ordered_access(obligations: Obligations, volatile: bool) -> Obligations {
    if volatile {
        obligations.with(Obligation::Ordering)
    } else {
        obligations
    }
}

#[cfg(test)]
#[path = "canonical_ranked_view_v18_tests.rs"]
mod tests;
