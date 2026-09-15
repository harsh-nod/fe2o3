//! Join original source operands to the currently live root projection tables.
use super::*;
use crate::collector::{GuardedBf16SourceEventsV1, RootBf16PhysicalInputV1};
use fe2o3_lower_mir_kernel::{ProductionGlobalBf16SourceRowV1, ProductionScopedBf16LaneUseV1};

#[allow(clippy::too_many_arguments)]
pub(super) fn bind_pending_sources(
    source: &mut GuardedBf16SourceEventsV1<'_, '_>,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    entry: Option<&context_entry_v1::Entry<'_>>,
    allocations: &[Option<AllocationContractV1>],
    provenance: &[Option<LocalAllocationProvenanceV1>],
    extent_arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    source.with_ranked_root_inputs(function, types, |row, physical, lane, charge| {
        charge(24).map_err(ProductionRankedProjectionErrorV1::StructuralValidation)?;
        bind_lane_entry(function, entry, lane)?;
        bind_allocation(row, physical, function, types, allocations, provenance,
            extent_arguments, next_argument, views, operations, next_value, ranked_ir)
    })
}

fn bind_lane_entry(
    function: &SemanticFunctionDeclV1,
    entry: Option<&context_entry_v1::Entry<'_>>,
    lane: ProductionScopedBf16LaneUseV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let Some(entry) = entry else {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "BF16 source lane has no authenticated ranked Context entry",
        ));
    };
    if !entry.matches_body(function) || entry.issuer_value() != lane.context_value() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "BF16 source lane changed its original ranked Context issuer",
        ));
    }
    // Subgroup field-zero custody was checked by the source resolver. It is not
    // a numeric ranked value; do not replace it with argument zero or a symbol.
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn bind_allocation(
    row: ProductionGlobalBf16SourceRowV1<'_, '_>,
    physical: &RootBf16PhysicalInputV1<'_>,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    allocations: &[Option<AllocationContractV1>],
    provenance: &[Option<LocalAllocationProvenanceV1>],
    extent_arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let reject = || ProductionRankedProjectionErrorV1::Incomplete(
        "BF16 physical source differs from the exact root allocation or extent",
    );
    if !physical.belongs_to(row)
        || !std::ptr::eq(function, row.view().body())
        || !std::ptr::eq(types, row.owner().source_semantic().types())
        || allocations.len() != function.locals().len()
        || provenance.len() != function.locals().len()
        || function.abi().source_input_types().get(physical.root_argument() as usize)
            != Some(&physical.physical_type())
        || function.abi().source_argument_ownership().get(physical.root_argument() as usize)
            != Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
        || row.contract().memory() != SemanticCapabilityMemoryContractV1::global_read_only()
    {
        return Err(reject());
    }
    let local = physical.physical_local() as usize;
    let root = physical.root_local() as usize;
    let allocation = allocations.get(local).copied().flatten().ok_or_else(reject)?;
    let expected = Some(LocalAllocationProvenanceV1::Argument(physical.root_argument()));
    if allocations.get(root).copied().flatten() != Some(allocation)
        || provenance.get(local).copied().flatten() != expected
        || provenance.get(root).copied().flatten() != expected
        || allocation.allocation_origin != u64::from(physical.root_argument()) + 1
        || allocation.writable || allocation.singleton_object || allocation.noalias_class != 1
        || type_width(types, row.contract().types().element)? != 16
    {
        return Err(reject());
    }
    let bound = ProjectedGlobalViewV1 {
        view: row.contract().types().global,
        physical: physical.physical_type(),
        element: row.contract().types().element,
        contract: row.contract().memory(),
        provenance: row.contract().provenance(),
        allocation,
        borrow: Some(SemanticBorrowKindV1::Shared),
    };
    let extent = project_allocation_extent_argument_v1(
        physical.root_argument() as usize, extent_arguments, next_argument,
    )?;
    project_global_ranked_view_v1(types, bound, extent, views, operations, next_value, ranked_ir)?;
    Ok(())
}
