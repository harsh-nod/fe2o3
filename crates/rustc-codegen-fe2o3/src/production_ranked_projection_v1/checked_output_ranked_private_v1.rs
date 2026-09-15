// Included privately beneath checked_output_session_v1. All rows are temporary
// candidate definition coordinates, not source evidence or retained graph state.
use super::super::{
    ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedValueV1,
    ProjectedAccessSourceV1,
};
use super::*;

#[derive(Clone, Copy)]
struct Definition {
    value: u32,
    block: u32,
    operation: u32,
}

fn invalid(detail: &'static str) -> ProjectionError {
    ProjectionError::Incomplete(detail)
}

fn definition_key(operation: &ProductionRankedOperationV1) -> Option<u32> {
    match operation {
        ProductionRankedOperationV1::ViewInSpace { result, .. }
        | ProductionRankedOperationV1::IndexConstant { result, .. } => Some(result.get()),
        _ => None,
    }
}

fn find(
    rows: &[Definition],
    value: ProductionRankedValueV1,
    budget: &mut Budget<'_>,
) -> Result<(u32, u32), ProjectionError> {
    budget.charge_work(1).map_err(resource)?;
    let ProductionRankedValueV1::Local(value) = value else {
        return Err(invalid(
            "checked output private ranked operand is not a local definition",
        ));
    };
    let mut start = 0usize;
    let mut end = rows.len();
    loop {
        budget.charge_work(1).map_err(resource)?;
        if start == end {
            return Err(invalid(
                "checked output private ranked definition is absent",
            ));
        }
        // Subtraction, division, addition, indexed row access and scalar compare.
        budget.charge_work(5).map_err(resource)?;
        let middle = start + (end - start) / 2;
        let row = rows[middle];
        match row.value.cmp(&value.get()) {
            std::cmp::Ordering::Equal => return Ok((row.block, row.operation)),
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Less => {
                budget.charge_work(1).map_err(resource)?;
                start = middle + 1;
            }
        }
    }
}

// Four stable byte passes. Histogram and second row buffer are prepaid alongside
// the first buffer; no allocator-backed comparator or per-access graph scan.
fn order(
    rows: &mut Vec<Definition>,
    scratch: &mut Vec<Definition>,
    budget: &mut Budget<'_>,
) -> Result<(), ProjectionError> {
    budget.charge_work(rows.len()).map_err(resource)?;
    scratch.extend_from_slice(rows);
    for shift in [0u32, 8, 16, 24] {
        budget.charge_work(256).map_err(resource)?;
        let mut offsets = [0usize; 256];
        for row in rows.iter() {
            // Row visit, shift, mask, integer conversion, bucket access and add.
            budget.charge_work(6).map_err(resource)?;
            let byte = ((row.value >> shift) & 255) as usize;
            offsets[byte] = offsets[byte]
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
        }
        let mut prefix = 0usize;
        for count in &mut offsets {
            // Cell visit, checked prefix addition, and output offset write.
            budget.charge_work(3).map_err(resource)?;
            let next = prefix
                .checked_add(*count)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            *count = prefix;
            prefix = next;
        }
        for row in rows.iter() {
            // Visit, shift/mask/conversion, bucket read, output write, increment.
            budget.charge_work(7).map_err(resource)?;
            let byte = ((row.value >> shift) & 255) as usize;
            scratch[offsets[byte]] = *row;
            offsets[byte] += 1;
        }
        budget.charge_work(1).map_err(resource)?;
        std::mem::swap(rows, scratch);
    }
    for pair in rows.windows(2) {
        // Adjacent pair visit and exact definition-id comparison.
        budget.charge_work(2).map_err(resource)?;
        if pair[0].value == pair[1].value {
            return Err(invalid(
                "checked output private ranked definitions repeat an id",
            ));
        }
    }
    Ok(())
}

pub(super) fn check(
    occurrences: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    function_name: &str,
    blocks: &[ProductionRankedBlockV1],
    sources: &[ProjectedAccessSourceV1],
    budget: &mut Budget<'_>,
) -> Result<(), ProjectionError> {
    // The existing session owns/reserves the actual N/B/O/index lifetime. This
    // scope adds only candidate-definition scratch on that same live ledger.
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut has_private_source = false;
        for source in sources {
            budget.charge_work(3).map_err(resource)?;
            if source.memory_space == dialect_kernel::MemorySpaceAttr::Private
                && source.private_array_role.is_some()
            {
                has_private_source = true;
                break;
            }
        }
        budget.charge_work(1).map_err(resource)?;
        if !has_private_source {
            return Ok(());
        }
        let mut count = 0usize;
        for block in blocks {
            budget.charge_work(1).map_err(resource)?;
            for operation in block.operations() {
                budget.charge_work(2).map_err(resource)?;
                if definition_key(operation).is_some() {
                    budget.charge_work(1).map_err(resource)?;
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| resource(Resource::Arithmetic))?;
                }
            }
        }
        // Two typed multiplications, two additions and one Layout admission.
        budget.charge_work(5).map_err(resource)?;
        let payload = count
            .checked_mul(std::mem::size_of::<Definition>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let buffers = payload
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let bytes = buffers
            .checked_add(2 * std::mem::size_of::<Vec<Definition>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<[usize; 256]>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        std::alloc::Layout::array::<Definition>(count)
            .map_err(|_| resource(Resource::Arithmetic))?;
        budget.reserve_storage(bytes).map_err(resource)?;
        let mut rows = Vec::<Definition>::new();
        let mut scratch = Vec::<Definition>::new();
        // Two fallible reserve intents, after the entire coexisting payload.
        budget.charge_work(2).map_err(resource)?;
        rows.try_reserve_exact(count)
            .map_err(|_| resource(Resource::Allocation))?;
        scratch
            .try_reserve_exact(count)
            .map_err(|_| resource(Resource::Allocation))?;
        for (block_ordinal, block) in blocks.iter().enumerate() {
            budget.charge_work(1).map_err(resource)?;
            for (operation_ordinal, operation) in block.operations().iter().enumerate() {
                budget.charge_work(2).map_err(resource)?;
                if let Some(value) = definition_key(operation) {
                    // Two coordinate conversions and one already-reserved write.
                    budget.charge_work(3).map_err(resource)?;
                    rows.push(Definition {
                        value,
                        block: u32::try_from(block_ordinal)
                            .map_err(|_| resource(Resource::Arithmetic))?,
                        operation: u32::try_from(operation_ordinal)
                            .map_err(|_| resource(Resource::Arithmetic))?,
                    });
                }
            }
        }
        order(&mut rows, &mut scratch, budget)?;
        for source in sources {
            budget.charge_work(3).map_err(resource)?;
            if source.memory_space != dialect_kernel::MemorySpaceAttr::Private {
                continue;
            }
            let Some(role) = source.private_array_role else {
                continue;
            };
            // Site and statement dispatch, then exact source coordinate casts.
            budget.charge_work(4).map_err(resource)?;
            let site = source.semantic_site.ok_or_else(|| {
                invalid("checked output private ranked access lacks its source site")
            })?;
            let statement = site.statement.ok_or_else(|| {
                invalid("checked output private ranked access is a terminator effect")
            })?;
            let site = fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                block: fe2o3_mir_model::SsaBlockIdV1::new(
                    u32::try_from(site.block).map_err(|_| resource(Resource::Arithmetic))?,
                ),
                statement: u32::try_from(statement).map_err(|_| resource(Resource::Arithmetic))?,
            };
            budget.charge_work(1).map_err(resource)?;
            match occurrences
                .private_array_write(owner, function, site, role, budget)
                .map_err(source_output)?
            {
                ProductionSourceOutputPrivateArrayAccessV1::ProvenUnretained => continue,
                ProductionSourceOutputPrivateArrayAccessV1::Retained {
                    executable: true, ..
                } => {}
                _ => {
                    return Err(invalid(
                        "checked output private ranked write is not executable",
                    ));
                }
            }
            // Two physical lookups, Access classification and index shape.
            budget.charge_work(4).map_err(resource)?;
            let operation = blocks
                .get(source.block)
                .and_then(|block| block.operations().get(source.operation))
                .ok_or_else(|| invalid("checked output private ranked access is absent"))?;
            let ProductionRankedOperationV1::Access { view, indices, .. } = operation else {
                return Err(invalid(
                    "checked output private ranked operation is not an Access",
                ));
            };
            let [index] = indices.as_slice() else {
                return Err(invalid(
                    "checked output private ranked access is not rank one",
                ));
            };
            let view_coordinate = find(&rows, *view, budget)?;
            let index_coordinate = find(&rows, *index, budget)?;
            budget.charge_work(2).map_err(resource)?;
            let access_coordinate = (
                u32::try_from(source.block).map_err(|_| resource(Resource::Arithmetic))?,
                u32::try_from(source.operation).map_err(|_| resource(Resource::Arithmetic))?,
            );
            occurrences
                .check_ranked_private_array_write_allocation_index(
                    owner,
                    function,
                    site,
                    role,
                    function_name,
                    blocks,
                    access_coordinate,
                    view_coordinate,
                    index_coordinate,
                    budget,
                )
                .map_err(source_output)?;
        }
        Ok(())
    }));
    // Closure-local vectors have been dropped on every exit, including unwind.
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(|| resource(Resource::Accounting))?;
    budget.release_storage(release).map_err(resource)?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
mod tests {
    include!("checked_output_ranked_private_index_v1_tests.rs");
}
