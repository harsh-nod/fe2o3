//! Source-checked recipe coordinates joined to replayed materialization rows.

use super::*;
use crate::production::conditional_read_occurrences_v1::{ReadExtentV1, ReadOccurrenceV1};
use dialect_kernel::{DYNAMIC_EXTENT, MemorySpaceAttr};

fn failure() -> ProductionSessionErrorV1 {
    ProductionSessionErrorV1::RankedGraphChanged
}

fn limit() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: Phase::PipelineVerification,
        resource: "conditional source/read binding work or storage",
    }
}

fn reserve<T>(
    count: usize,
    work: usize,
    resources: &mut ProductionAnalysisResourceContractV1,
    canonical: Option<&CanonicalAccountV1<'_, '_>>,
) -> Result<Vec<T>, ProductionSessionErrorV1> {
    let phase = Phase::PipelineVerification;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| resource(limit()))?;
    let bound = Bound::checked_phase(phase, work, bytes, 0).map_err(resource)?;
    if let Some(account) = canonical {
        let mut budget = account.borrow_mut();
        budget
            .charge_work(work)
            .map_err(|_| resource(canonical_limit_v1(phase)))?;
        budget
            .reserve_storage(bytes)
            .map_err(|_| resource(canonical_limit_v1(phase)))?;
    }
    resources.admit_retained(phase, bound).map_err(resource)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| resource(limit()))?;
    if rows.capacity() != count {
        return Err(resource(limit()));
    }
    Ok(rows)
}

pub(super) fn retain_source_reads_v1(
    reads: &[RecipeReadBoundV1],
    resources: &mut ProductionAnalysisResourceContractV1,
    canonical: Option<&CanonicalAccountV1<'_, '_>>,
) -> Result<Vec<RecipeReadBoundV1>, ProductionSessionErrorV1> {
    let work = reads
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| resource(limit()))?;
    let mut rows = reserve(reads.len(), work, resources, canonical)?;
    rows.extend_from_slice(reads);
    Ok(rows)
}

pub(super) fn bind_live_reads_v1(
    recipe: &ProductionRankedKernelV1,
    occurrences: &[ReadOccurrenceV1],
    census: crate::production_analysis::ProductionAnalysisInputCensusV1,
    reads: &[RecipeReadBoundV1],
    resources: &mut ProductionAnalysisResourceContractV1,
    canonical: Option<&CanonicalAccountV1<'_, '_>>,
) -> Result<Vec<LiveReadBoundV1>, ProductionSessionErrorV1> {
    let work = census
        .operations
        .checked_add(census.blocks)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| reads.len().checked_add(1).and_then(|r| n.checked_mul(r)))
        .and_then(|n| n.checked_mul(32))
        .ok_or_else(|| resource(limit()))?;
    let mut rows = reserve(reads.len(), work, resources, canonical)?;
    if reads.len() != occurrences.len() || reads.len() > census.operations {
        return Err(failure());
    }
    for occurrence in occurrences {
        let mut matched = None;
        for read in reads {
            if read.block == occurrence.block
                && read.operation == occurrence.recipe_operation
                && matched.replace(read).is_some()
            {
                return Err(failure());
            }
        }
        let read = matched.ok_or_else(failure)?;
        let [coordinate] = occurrence.coordinates() else {
            return Err(failure());
        };
        let ReadExtentV1::Dynamic(extent) = coordinate.extent else {
            return Err(failure());
        };
        if read.view != occurrence.recipe_view || read.index != coordinate.recipe_index {
            return Err(failure());
        }
        require_recipe_extent_v1(recipe, read)?;
        rows.push(LiveReadBoundV1 {
            operation: occurrence.operation,
            view: occurrence.view,
            index: coordinate.index,
            extent,
            domain: read.domain,
        });
    }
    Ok(rows)
}

fn require_recipe_extent_v1(
    recipe: &ProductionRankedKernelV1,
    read: &RecipeReadBoundV1,
) -> Result<(), ProductionSessionErrorV1> {
    use ProductionRankedOperationV1 as Op;
    let mut found = false;
    for op in recipe.blocks().iter().flat_map(|block| block.operations()) {
        match op {
            Op::View {
                result,
                writable,
                shape,
                dynamic_extents,
                ..
            }
            | Op::ViewInSpace {
                result,
                writable,
                shape,
                dynamic_extents,
                ..
            } if ProductionRankedValueV1::Local(*result) == read.view => {
                if found
                    || *writable
                    || shape.as_slice() != [DYNAMIC_EXTENT]
                    || dynamic_extents.as_slice() != [read.extent]
                    || !matches!(read.extent, ProductionRankedValueV1::Argument(_))
                    || matches!(op, Op::ViewInSpace { memory_space, .. }
                        if *memory_space != MemorySpaceAttr::Global)
                {
                    return Err(failure());
                }
                found = true;
            }
            _ => {}
        }
    }
    if !found {
        return Err(failure());
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditional_pipeline_reads_v1_tests.rs"]
mod tests;
